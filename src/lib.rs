//! GTO Studio: postflop solver engine shared by the native server (`src/main.rs`)
//! and the browser build (`web/`, WebAssembly).

pub mod engine;
pub mod hands;

pub use postflop_solver;
