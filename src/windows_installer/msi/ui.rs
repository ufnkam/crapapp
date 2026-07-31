//! Control rows for the authored installer UI.

use msi::Value;

use crate::windows_installer::{Eula, InstallerConfig};

use super::{
    builder::{
        ADD_TO_PATH_PROPERTY, BACK_BUTTON_X, CANCEL_BUTTON_X, NEXT_BUTTON_X, checkbox_control,
        control_row, eula_property, eula_rtf, footer_button, heading_control, license_dialog_id,
        line_control, text_control, title_control,
    },
    identity,
};

pub(super) fn control_rows(
    plan: &InstallerConfig,
    eulas: &[Eula],
    path_updates: &[String],
) -> Vec<Vec<Value>> {
    let display_name = identity::display_name(plan);
    let welcome_title = format!("Welcome to the {display_name} Setup Wizard");
    let welcome_body = format!(
        "\nThis wizard will install {} {} on your computer.",
        display_name, plan.app_version,
    );
    let install_dir_title = format!("Choose where to install {display_name}");
    let ready_title = format!("Ready to install {display_name}");
    let exit_title = format!("Completed the Setup Wizard for {display_name}");

    let mut rows = vec![
        title_control("WelcomeDlg", "Title", 24, 24, 472, 40, &welcome_title),
        text_control("WelcomeDlg", "Body", 24, 82, 472, 80, &welcome_body),
        line_control("WelcomeDlg"),
        footer_button("WelcomeDlg", "Cancel", CANCEL_BUTTON_X, "Cancel"),
        footer_button("WelcomeDlg", "Next", NEXT_BUTTON_X, "Next"),
        title_control(
            "InstallDirDlg",
            "Title",
            24,
            24,
            472,
            40,
            &install_dir_title,
        ),
        text_control(
            "InstallDirDlg",
            "Body",
            24,
            72,
            472,
            32,
            "Choose the directory to where application will be installed.",
        ),
        heading_control(
            "InstallDirDlg",
            "PathLabel",
            44,
            122,
            432,
            18,
            "Installation folder",
        ),
        super::builder::edit_control(
            "InstallDirDlg",
            "PathEdit",
            44,
            150,
            344,
            18,
            "INSTALLFOLDER",
        ),
        super::builder::button_control("InstallDirDlg", "Browse", 396, 148, 80, 22, "Browse..."),
        checkbox_control(
            "InstallDirDlg",
            "AddToPath",
            44,
            198,
            432,
            20,
            ADD_TO_PATH_PROPERTY,
            "Add to PATH",
            !path_updates.is_empty(),
        ),
        line_control("InstallDirDlg"),
        footer_button("InstallDirDlg", "Back", BACK_BUTTON_X, "Back"),
        footer_button("InstallDirDlg", "Cancel", CANCEL_BUTTON_X, "Cancel"),
        footer_button("InstallDirDlg", "Next", NEXT_BUTTON_X, "Next"),
        title_control("VerifyReadyDlg", "Title", 24, 24, 472, 40, &ready_title),
        text_control(
            "VerifyReadyDlg",
            "Body",
            24,
            82,
            472,
            60,
            "Click Install to begin installation.",
        ),
        line_control("VerifyReadyDlg"),
        footer_button("VerifyReadyDlg", "Back", BACK_BUTTON_X, "Back"),
        footer_button("VerifyReadyDlg", "Cancel", CANCEL_BUTTON_X, "Cancel"),
        footer_button("VerifyReadyDlg", "Install", NEXT_BUTTON_X, "Install"),
        title_control(
            "ProgressDlg",
            "Title",
            24,
            24,
            472,
            40,
            &format!("Installing {display_name}"),
        ),
        text_control(
            "ProgressDlg",
            "Version",
            24,
            78,
            472,
            22,
            &format!("Version {}", plan.app_version),
        ),
        text_control(
            "ProgressDlg",
            "ActionText",
            24,
            122,
            472,
            24,
            "Preparing installation...",
        ),
        control_row(
            "ProgressDlg",
            "Progress",
            "ProgressBar",
            24,
            158,
            472,
            18,
            1,
            None,
            None,
        ),
        line_control("ProgressDlg"),
        footer_button("ProgressDlg", "Cancel", NEXT_BUTTON_X, "Cancel"),
        title_control("ExitDlg", "Title", 24, 24, 472, 40, &exit_title),
        text_control(
            "ExitDlg",
            "Body",
            24,
            82,
            472,
            60,
            "Installation completed successfully.",
        ),
        line_control("ExitDlg"),
        footer_button("ExitDlg", "Finish", NEXT_BUTTON_X, "Finish"),
    ];

    for (index, eula) in eulas.iter().enumerate() {
        let dialog = license_dialog_id(index);
        rows.extend([
            title_control(
                &dialog,
                "Title",
                24,
                20,
                472,
                34,
                &format!("License agreement {} of {}", index + 1, eulas.len()),
            ),
            heading_control(&dialog, "Name", 24, 58, 472, 20, &eula.name),
            control_row(
                &dialog,
                "LicenseText",
                "ScrollableText",
                24,
                86,
                472,
                174,
                7,
                None,
                Some(&eula_rtf(&eula.text)),
            ),
            line_control(&dialog),
            footer_button(&dialog, "Back", BACK_BUTTON_X, "Back"),
            footer_button(&dialog, "Cancel", CANCEL_BUTTON_X, "Cancel"),
            footer_button(&dialog, "Next", NEXT_BUTTON_X, "Next"),
            checkbox_control(
                &dialog,
                "Accept",
                24,
                272,
                472,
                22,
                &eula_property(index),
                &format!("I accept {}", eula.name),
                true,
            ),
        ]);
    }

    rows
}
