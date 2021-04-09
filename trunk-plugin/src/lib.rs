pub use trunk_derive::{trunk_extern, trunk_plugin};

pub use crate::{
    args::Args,
    error::Error,
    output::Output,
    permissions::Permissions,
};

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub mod args;
pub mod error;
pub mod output;
pub mod permissions;

#[doc(hidden)]
pub mod export {
    pub use core;

    pub use serde_cbor;

    pub use crate::{
        args::Args,
        error::Error,
        output::Output,
        permissions::Permissions,
    };
}
