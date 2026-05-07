pub mod audio;
pub mod download;
pub mod upload;
pub mod video;
pub mod network_runtime;

use std::sync::OnceLock;
use tokio::runtime::Runtime;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum StreamType {
    Audio,
    Video,
}

pub fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().expect("Runtime creation failed. Loser"))
}