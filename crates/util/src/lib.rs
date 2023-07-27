mod archive;
mod error;
mod executable;
mod fs;
mod manifest;
mod strip_prefix;

pub use archive::Archive;
pub use error::{Error, ErrorExt, ErrorReason, Result, ResultExt};
pub use executable::{is_executable, Executable};
pub use fs::{copy_dir_recursive, path_exists, remove_dir_all};
pub use manifest::{CargoMetadata, Features};
pub use strip_prefix::strip_prefix;
