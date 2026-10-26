//! Core of MobaLedLib Studio: layout model, `.MLL_pgf` import/export, macro catalog,
//! LED numbering and generation of the `LEDs_AutoProg.h` header for the MobaLedLib.

pub mod catalog;
pub mod codegen;
pub mod leds;
pub mod model;
pub mod pgf;
pub mod vba;

pub use catalog::Catalog;
pub use model::{Layout, Protocol, Row, Sheet};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Format(String),
    #[error("{0}")]
    Invalid(String),
}
