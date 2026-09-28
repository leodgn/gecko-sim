//! Turns `.s` source files into binary images the emulator can load.

#[cfg(not(target_arch = "wasm32"))]
pub mod native;
