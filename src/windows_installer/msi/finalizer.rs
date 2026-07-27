use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{Context, anyhow};

fn demangle_stream_name(name: &str) -> String {
    const BASE64: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz._";
    let mut output = String::new();

    for character in name.chars() {
        let value = character as u32;
        if (0x3800..0x4800).contains(&value) {
            let encoded = value - 0x3800;
            output.push(BASE64[(encoded & 0x3f) as usize] as char);
            output.push(BASE64[((encoded >> 6) & 0x3f) as usize] as char);
        } else if (0x4800..0x4840).contains(&value) {
            let encoded = value - 0x4800;
            output.push(BASE64[(encoded & 0x3f) as usize] as char);
        } else if value == 0x4840 {
            output.push('\0');
        } else {
            output.push(character);
        }
    }

    output
}

fn resort_stream(data: &[u8], widths: &[usize], keys: &[usize]) -> Vec<u8> {
    let row_width = widths.iter().sum::<usize>();
    if row_width == 0 {
        return data.to_vec();
    }

    let rows = data.len() / row_width;
    if rows <= 1 {
        return data.to_vec();
    }

    let mut column_offsets = vec![0; widths.len()];
    let mut offset = 0;
    for (column, width) in widths.iter().enumerate() {
        column_offsets[column] = offset;
        offset += rows * width;
    }

    let read = |row: usize, column: usize| -> u64 {
        let base = column_offsets[column] + row * widths[column];
        let mut value = 0;
        for byte in 0..widths[column] {
            value |= (data[base + byte] as u64) << (8 * byte);
        }
        value
    };

    let mut order = (0..rows).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        for key in keys {
            match read(*left, *key).cmp(&read(*right, *key)) {
                std::cmp::Ordering::Equal => {}
                order => return order,
            }
        }
        left.cmp(right)
    });

    let mut output = vec![0; data.len()];
    for (column, width) in widths.iter().enumerate() {
        for (new_row, old_row) in order.iter().enumerate() {
            let source = column_offsets[column] + old_row * width;
            let destination = column_offsets[column] + new_row * width;
            output[destination..destination + width].copy_from_slice(&data[source..source + width]);
        }
    }

    output
}

pub(super) fn finalize(msi_path: &Path) -> anyhow::Result<()> {
    let mut compound = cfb::open_rw(msi_path)
        .with_context(|| format!("failed to open MSI compound file {}", msi_path.display()))?;
    let names = compound
        .read_storage("/")?
        .map(|entry| entry.name().to_owned())
        .collect::<Vec<_>>();

    let read_stream =
        |compound: &mut cfb::CompoundFile<std::fs::File>, raw: &str| -> anyhow::Result<Vec<u8>> {
            let mut stream = compound
                .open_stream(format!("/{raw}"))
                .with_context(|| format!("failed to open MSI stream {raw:?}"))?;
            let mut bytes = Vec::new();
            stream
                .read_to_end(&mut bytes)
                .with_context(|| format!("failed to read MSI stream {raw:?}"))?;
            Ok(bytes)
        };
    let write_stream = |compound: &mut cfb::CompoundFile<std::fs::File>,
                        raw: &str,
                        bytes: &[u8]|
     -> anyhow::Result<()> {
        let mut stream = compound
            .open_stream(format!("/{raw}"))
            .with_context(|| format!("failed to open MSI stream {raw:?} for writing"))?;
        stream
            .write_all(bytes)
            .with_context(|| format!("failed to write MSI stream {raw:?}"))?;
        Ok(())
    };
    let find_raw = |suffix: &str| -> Option<String> {
        names
            .iter()
            .find(|name| demangle_stream_name(name).ends_with(suffix))
            .cloned()
    };

    let pool_raw = find_raw("_StringPool").ok_or_else(|| anyhow!("MSI has no _StringPool"))?;
    let pool = read_stream(&mut compound, &pool_raw)?;
    let string_width = if pool.len() >= 4 && (pool[3] & 0x80) != 0 {
        3
    } else {
        2
    };

    let data_raw = find_raw("_StringData").ok_or_else(|| anyhow!("MSI has no _StringData"))?;
    let string_data = read_stream(&mut compound, &data_raw)?;
    let mut strings = vec![String::new()];
    let mut data_offset = 0;
    let mut pool_offset = 4;
    while pool_offset + 4 <= pool.len() {
        let length = u16::from_le_bytes([pool[pool_offset], pool[pool_offset + 1]]) as usize;
        let end = (data_offset + length).min(string_data.len());
        strings.push(String::from_utf8_lossy(&string_data[data_offset..end]).into_owned());
        data_offset += length;
        pool_offset += 4;
    }

    let string_id = |text: &str| -> Option<u64> {
        strings
            .iter()
            .position(|string| string == text)
            .map(|index| index as u64)
    };

    let system_tables = [
        ("_Tables", vec![string_width], vec![0]),
        (
            "_Columns",
            vec![string_width, 2, string_width, 2],
            vec![0, 1],
        ),
        (
            "_Validation",
            vec![
                string_width,
                string_width,
                string_width,
                4,
                4,
                string_width,
                2,
                string_width,
                string_width,
                string_width,
            ],
            vec![0, 1],
        ),
    ];

    for (name, widths, keys) in system_tables {
        if let Some(raw) = find_raw(name) {
            let bytes = read_stream(&mut compound, &raw)?;
            let sorted = resort_stream(&bytes, &widths, &keys);
            write_stream(&mut compound, &raw, &sorted)?;
        }
    }

    let columns_raw = find_raw("_Columns").ok_or_else(|| anyhow!("MSI has no _Columns"))?;
    let columns = read_stream(&mut compound, &columns_raw)?;
    let column_row_width = string_width + 2 + string_width + 2;
    let column_rows = columns.len() / column_row_width;

    let read_column = |array_offset: usize, row: usize, width: usize| -> u64 {
        let base = array_offset + row * width;
        let mut value = 0;
        for byte in 0..width {
            value |= (columns[base + byte] as u64) << (8 * byte);
        }
        value
    };

    let table_offset = 0;
    let number_offset = column_rows * string_width;
    let type_offset = number_offset + column_rows * 2 + column_rows * string_width;
    let width_of = |column_type: u64| -> usize {
        let normalized = (column_type ^ 0x8000) & 0xffff;
        if (normalized & 0x0800) != 0 {
            string_width
        } else if (normalized & 0xff) == 4 {
            4
        } else {
            2
        }
    };
    let is_key = |column_type: u64| -> bool { ((column_type ^ 0x8000) & 0x2000) != 0 };

    let mut table_columns = BTreeMap::<u64, Vec<(u64, u64)>>::new();
    for row in 0..column_rows {
        let table_id = read_column(table_offset, row, string_width);
        let number = read_column(number_offset, row, 2) ^ 0x8000;
        let column_type = read_column(type_offset, row, 2);
        table_columns
            .entry(table_id)
            .or_default()
            .push((number, column_type));
    }
    for columns in table_columns.values_mut() {
        columns.sort_by_key(|(number, _)| *number);
    }

    for raw in &names {
        let demangled = demangle_stream_name(raw);
        let Some(table_name) = demangled.strip_prefix('\0') else {
            continue;
        };
        if table_name.starts_with('_') {
            continue;
        }

        let Some(table_id) = string_id(table_name) else {
            continue;
        };
        let Some(columns) = table_columns.get(&table_id) else {
            continue;
        };

        let widths = columns
            .iter()
            .map(|(_, column_type)| width_of(*column_type))
            .collect::<Vec<_>>();
        let keys = columns
            .iter()
            .enumerate()
            .filter(|(_, (_, column_type))| is_key(*column_type))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if keys.is_empty() {
            continue;
        }

        let bytes = read_stream(&mut compound, raw)?;
        let sorted = resort_stream(&bytes, &widths, &keys);
        write_stream(&mut compound, raw, &sorted)?;
    }

    compound
        .flush()
        .with_context(|| format!("failed to flush finalized MSI {}", msi_path.display()))?;
    Ok(())
}
