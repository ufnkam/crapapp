//! Embedded MSI action that opens Windows' Common Item Dialog.

#[cfg(windows)]
mod windows_action {
    use windows::{
        Win32::{
            Foundation::ERROR_CANCELLED,
            System::{
                ApplicationInstallationAndServicing::{
                    MSIHANDLE, MsiSetPropertyW,
                },
                Com::{
                    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance,
                    CoInitializeEx, CoTaskMemFree, CoUninitialize,
                },
            },
            UI::Shell::{
                FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog,
                IFileOpenDialog, SIGDN_FILESYSPATH,
            },
        },
        core::{HRESULT, PCWSTR},
    };

    const ERROR_INSTALL_FAILURE: u32 = 1603;

    /// Opens the normal Windows folder picker and writes its selection to MSI.
    /// Cancelling leaves `INSTALLFOLDER` unchanged and continues the installer.
    #[unsafe(no_mangle)]
    pub unsafe extern "system" fn PickInstallFolder(session: MSIHANDLE) -> u32 {
        let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok() };
        let result = unsafe { show_folder_picker(session) };
        if initialized {
            unsafe { CoUninitialize() };
        }

        match result {
            Ok(()) => 0,
            Err(error) if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) => 0,
            Err(_) => ERROR_INSTALL_FAILURE,
        }
    }

    unsafe fn show_folder_picker(session: MSIHANDLE) -> windows::core::Result<()> {
        let dialog: IFileOpenDialog =
            unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)? };
        let options = unsafe { dialog.GetOptions()? };
        unsafe {
            dialog
                .SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)?;
            dialog.SetTitle(windows::core::w!("Select installation folder"))?;
            dialog.Show(None)?;
        }

        let item = unsafe { dialog.GetResult()? };
        let path = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH)? };
        let property_result =
            unsafe { MsiSetPropertyW(session, windows::core::w!("INSTALLFOLDER"), PCWSTR(path.0)) };
        unsafe { CoTaskMemFree(Some(path.0.cast())) };

        if property_result == 0 {
            Ok(())
        } else {
            Err(HRESULT::from_win32(property_result).into())
        }
    }
}

#[cfg(windows)]
pub use windows_action::PickInstallFolder;

// Keeps the auxiliary crate checkable from non-Windows development hosts. The
// bundler only builds and embeds it for a Windows target, so this symbol is
// never shipped in a generated MSI.
#[cfg(not(windows))]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn PickInstallFolder(_session: u32) -> u32 {
    1603
}
