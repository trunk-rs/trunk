pub use trunk_derive::{trunk_extern, trunk_plugin};

pub use crate::{args::Args, error::Error, output::Output};

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub mod args;
pub mod error;
pub mod output;

#[doc(hidden)]
pub mod export {
    pub use crate::{args::Args, error::Error, output::Output};
    pub use core;
    pub use serde_cbor;
}
