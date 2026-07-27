/// Runs work with stable terminal output.
///
/// Do not redraw this line: cargo commands can take minutes and many terminals
/// render carriage-return spinners as distracting flicker.
pub fn run<T, E, F>(message: &str, work: F) -> Result<T, E>
where
    F: FnOnce() -> Result<T, E>,
{
    eprintln!("{message}...");
    let result = work();
    let status = if result.is_ok() { "done" } else { "failed" };
    eprintln!("{message} {status}");
    result
}
