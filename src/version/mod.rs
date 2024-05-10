mod enforce;

#[cfg(feature = "update_check")]
mod enabled;
#[cfg(feature = "update_check")]
pub use enabled::update_check;

#[cfg(not(feature = "update_check"))]
mod disabled;
#[cfg(not(feature = "update_check"))]
pub use disabled::update_check;

pub(crate) use enforce::enforce_version_with;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
#[cfg(feature = "update_check")]
const NAME: &str = env!("CARGO_PKG_NAME");
