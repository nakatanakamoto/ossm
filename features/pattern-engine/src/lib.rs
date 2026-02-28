#![no_std]

mod input;
mod pattern;
pub mod patterns;
mod util;

pub use input::{PatternInput, SharedPatternInput};
pub use pattern::{Pattern, PatternCtx};
pub use util::scale;
