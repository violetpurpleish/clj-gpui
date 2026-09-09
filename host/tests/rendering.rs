//! Explicit macOS Metal checks for the production clj-gpui renderer.
//!
//! Run with `cargo test --manifest-path host/Cargo.toml --test rendering`.
//! Other platforms report the target as unsupported; they do not substitute
//! semantic snapshots for pixels.

#![allow(dead_code, unused_imports)]

fn main() {
    #[cfg(target_os = "macos")]
    macos::run();
    #[cfg(not(target_os = "macos"))]
    println!("rendering: skipped; GPUI has no offscreen renderer on this platform");
}

#[cfg(target_os = "macos")]
#[path = "../src/action_bridge.rs"]
mod action_bridge;
#[cfg(target_os = "macos")]
#[path = "../src/catalog.rs"]
mod catalog;
#[cfg(target_os = "macos")]
#[path = "../src/chat.rs"]
mod chat;
#[cfg(target_os = "macos")]
#[path = "../src/extra.rs"]
mod extra;
#[cfg(target_os = "macos")]
#[path = "../src/mapping.rs"]
mod mapping;
#[cfg(target_os = "macos")]
#[path = "../src/overlay.rs"]
mod overlay;
#[cfg(target_os = "macos")]
#[path = "../src/preview.rs"]
mod preview;
#[cfg(target_os = "macos")]
#[path = "../src/protocol.rs"]
mod protocol;
#[cfg(target_os = "macos")]
#[path = "../src/renderer.rs"]
mod renderer;
#[cfg(target_os = "macos")]
#[path = "../src/rows.rs"]
mod rows;
#[cfg(target_os = "macos")]
#[path = "../src/syntax.rs"]
mod syntax;

#[cfg(target_os = "macos")]
mod macos {
    use super::{protocol, renderer, syntax};
    use gpui_kit::component::Root;
    use gpui_kit::{
        AppContext as _, HeadlessAppContext, assets::AllAssets, px, size, test::TestWindowExt,
    };
    use serde_json::json;
    use std::sync::{Arc, mpsc};

    fn tree(editor_text: &str, selected: bool) -> protocol::Node {
        serde_json::from_value(json!({
            "type": "window",
            "chrome": "app",
            "theme": "light",
            "padding": 20,
            "gap": 14,
            "children": [
                {"type": "label", "text": "Clojure λ — glyph edges", "font-size": 22},
                {"type": "button", "id": "ghost", "text": "Selected ghost",
                 "variant": "ghost", "selected": selected},
                {"type": "editor", "id": "source", "text": editor_text,
                 "language": "clojure", "height": 120, "bordered": true},
                {"type": "markdown", "id": "notes", "height": 140, "frontmatter": true,
                 "text": "---\ntitle: Pixel fixture\n---\n**hard break**  \n`inline λ`"}
            ]
        }))
        .expect("valid rendering fixture")
    }

    fn save_failure(name: &str, image: &image::RgbaImage) {
        let path = format!("/private/tmp/clj-gpui-rendering-{name}.png");
        image.save(&path).expect("save Metal failure image");
        eprintln!("saved failure image: {path}");
    }

    pub fn run() {
        let mut cx = HeadlessAppContext::with_platform(
            gpui_kit::platform::current_platform(true).text_system(),
            Arc::new(AllAssets),
            gpui_kit::platform::current_headless_renderer,
        );
        cx.update(|cx| {
            gpui_kit::init(cx);
            syntax::init(cx);
        });

        let (cmd_tx, _cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = async_channel::unbounded();
        let handle = cx
            .open_window(size(px(620.), px(500.)), |window, cx| {
                let view = cx.new(|cx| renderer::RootView::new(7331, cmd_tx, event_rx, window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("open offscreen production window");

        event_tx
            .send_blocking(protocol::HostEvent::Tree(
                tree("(defn greet [name]\n  (str \"Hello, \" name))", true),
                None,
                vec![],
            ))
            .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        let first = cx
            .capture_screenshot(handle.into())
            .expect("Metal renderer must be available for the selected macOS target");
        let repeated = cx.capture_screenshot(handle.into()).unwrap();
        if first != repeated {
            save_failure("unstable-first", &first);
            save_failure("unstable-repeated", &repeated);
            panic!("fixed production fixture did not render deterministically");
        }

        event_tx
            .send_blocking(protocol::HostEvent::Tree(
                tree(
                    "(defn greet [name]\n  (str \"Welcome, \" name \"!\"))",
                    false,
                ),
                None,
                vec![],
            ))
            .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        let updated = cx.capture_screenshot(handle.into()).unwrap();
        if first == updated {
            save_failure("unchanged-before", &first);
            save_failure("unchanged-after", &updated);
            panic!("editor text and selected ghost-button changes produced identical pixels");
        }

        println!("rendering: 2 passed (production RootView, Metal)");
    }
}
