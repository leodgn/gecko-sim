//! Web build: runs the UI in the page's canvas.
//!
//! The browser has no threads, no command line and no file system, so this
//! side can't reuse `native`:
//! - `start` (here) is the entry point: it attaches the app to the canvas.
//!   It needs the browser, so it only exists in the wasm build.
//! - `app` holds the logic (loading a dropped program, running the CPU
//!   each frame). It doesn't need the browser, so it's also compiled for
//!   native tests.

mod app;

#[cfg(target_arch = "wasm32")]
use eframe::wasm_bindgen::JsCast as _;

/// Starts the app on the page's `<canvas id="the_canvas_id">` (see
/// `index.html`).
///
/// `WebRunner::start` is async (it waits for the browser's rendering
/// context) and a browser has no async runtime to block on, so it's handed
/// to the browser's event loop with `spawn_local`, and this returns right
/// away.
#[cfg(target_arch = "wasm32")]
pub fn start() {
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("no element with id the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id is not a canvas");

        eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|_cc| Ok(Box::new(app::WebApp::new(crate::assembler::web::assemble)))),
            )
            .await
            .expect("failed to start eframe");
    });
}
