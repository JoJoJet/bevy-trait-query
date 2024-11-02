mod core;
mod impls;

pub use impls::*;

pub use core::{change_detection::ChangeDetectionFetch, fetch::OneTraitFetch};
pub(crate) use core::{change_detection::ChangeDetectionStorage, fetch::FetchStorage};
