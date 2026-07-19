pub mod macos;
pub mod windows;

pub use macos::{MacosPkgConfig, MacosPlatformManifest};
pub use windows::WindowsPlatformManifest;
