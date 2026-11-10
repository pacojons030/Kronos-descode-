#![allow(
    clippy::field_reassign_with_default,
    clippy::useless_vec,
    clippy::manual_range_contains,
    clippy::vec_init_then_push,
    clippy::const_is_empty,
    clippy::unnecessary_literal_unwrap,
    clippy::collapsible_if
)]

#[path = "../src/core/mod.rs"]
pub mod core;

pub mod common;
pub mod e2e;
