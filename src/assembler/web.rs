//! Web `.s` support: the Rust side of `web/assemble.js`, which runs GNU
//! binutils compiled to WebAssembly in the browser (same pipeline as
//! `assembler::native`).

use std::sync::mpsc::{Receiver, channel};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/web/assemble.js")]
extern "C" {
    /// `assemble(fileName, source, linkerScript)` from web/assemble.js: a
    /// Promise of the binary (`Uint8Array`), rejected with binutils'
    /// messages (`catch` turns the rejection into `Err`).
    #[wasm_bindgen(catch, js_name = assemble)]
    async fn js_assemble(
        file_name: &str,
        source: &str,
        linker_script: &str,
    ) -> Result<JsValue, JsValue>;
}

/// The course linker script, embedded at compile time.
const LINKER_SCRIPT: &str = include_str!("../../assets/mmio.ld");

/// Starts assembling `source` (the text of `file_name`) in the background,
/// and returns right away. The binary image, or binutils' messages
/// verbatim, arrive in the returned receiver when done. See
/// `web::app::Assembler`.
pub fn assemble(file_name: &str, source: &str) -> Receiver<Result<Vec<u8>, String>> {
    let (sender, receiver) = channel();
    let file_name = file_name.to_owned();
    let source = source.to_owned();

    wasm_bindgen_futures::spawn_local(async move {
        let result = match js_assemble(&file_name, &source, LINKER_SCRIPT).await {
            Ok(bytes) => Ok(js_sys::Uint8Array::new(&bytes).to_vec()),
            Err(e) => Err(e
                .as_string()
                .unwrap_or_else(|| format!("could not run the assembler: {e:?}"))),
        };
        let _ = sender.send(result);
    });

    receiver
}
