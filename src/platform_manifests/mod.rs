pub mod linux;
pub mod macos;
pub mod windows;

pub use linux::LinuxPlatformManifest;
pub use macos::{MacosPkgConfig, MacosPlatformManifest};
pub use windows::WindowsPlatformManifest;
