//! Deterministic default-monospace fallback coverage for the production renderer.
//!
//! This is a separate test executable because GPUI Kit caches the resolved
//! default monospace family once per process. `NoopTextSystem` deliberately
//! omits the platform monospace default and its named alternates while still
//! providing deterministic shaping, so the fixture must choose GPUI's virtual
//! `.SystemUIFont` before laying out the editor and Markdown code block.

#![allow(dead_code, unused_imports)]

#[path = "../src/action_bridge.rs"]
mod action_bridge;
#[path = "../src/catalog.rs"]
mod catalog;
#[path = "../src/chat.rs"]
mod chat;
#[path = "../src/extra.rs"]
mod extra;
#[path = "../src/mapping.rs"]
mod mapping;
#[path = "../src/overlay.rs"]
mod overlay;
#[path = "../src/preview.rs"]
mod preview;
#[path = "../src/protocol.rs"]
mod protocol;
#[path = "../src/renderer.rs"]
mod renderer;
#[path = "../src/rows.rs"]
mod rows;
#[path = "../src/syntax.rs"]
mod syntax;

use gpui_kit::component::{Root, theme::Theme};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, NoopTextSystem, TestApp, base::TypographyTokens};
use serde_json::json;
use std::sync::{Arc, mpsc};

fn main() {
    let mut app = TestApp::with_text_system(Arc::new(NoopTextSystem::new()));
    let installed = app.text_system().all_font_names();
    let configured_default = TypographyTokens::default().mono;
    assert!(
        !installed
            .iter()
            .any(|name| name == configured_default.as_ref()),
        "the fixture must omit configured default {configured_default:?}: {installed:?}"
    );
    app.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
        assert_eq!(
            Theme::global(cx).mono_font_family.as_ref(),
            ".SystemUIFont",
            "an unavailable platform default must resolve to GPUI's virtual system family"
        );
    });

    let tree: protocol::Node = serde_json::from_value(json!({
        "type": "window",
        "chrome": "app",
        "padding": 16,
        "gap": 12,
        "children": [
            {
                "type": "editor",
                "id": "source",
                "language": "clojure",
                "height": 120,
                "text": "(println :fallback)"
            },
            {
                "type": "markdown",
                "id": "notes",
                "height": 120,
                "text": "```clojure\n(map inc [1 2 3])\n```"
            }
        ]
    }))
    .expect("valid production font fixture");

    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let mut window = app.open_window(|window, cx| {
        let view = cx.new(|cx| renderer::RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    assert!(matches!(cmd_rx.recv().unwrap(), protocol::Cmd::Render));
    event_tx
        .send_blocking(protocol::HostEvent::Tree(tree, None, vec![]))
        .unwrap();
    app.run_until_parked();

    let handle = window.handle();
    app.update(|cx| {
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let markdown = window.find("notes");
            assert!(markdown.visible());
            assert!(markdown.bounds().size.width > gpui_kit::px(0.));
            assert!(markdown.bounds().size.height > gpui_kit::px(0.));
        })
        .expect("production fallback window remains open");
    });
    window.draw();

    println!("mono-font-fallback: passed (production RootView, missing default monospace)");
}
