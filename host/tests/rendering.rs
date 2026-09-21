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

    fn tree(editor_text: &str, selected: bool, markdown_text: &str) -> protocol::Node {
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
                 "text": markdown_text}
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
            .send_blocking(protocol::HostEvent::tree(
                tree(
                    "(defn greet [name]\n  (str \"Hello, \" name))",
                    true,
                    "---\ntitle: Pixel fixture\n---\n**hard break**  \n`inline λ`",
                ),
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
            .send_blocking(protocol::HostEvent::tree(
                tree(
                    "(defn greet [name]\n  (str \"Welcome, \" name \"!\"))",
                    false,
                    "---\ntitle: Pixel fixture\n---\n**hard break**  \n`inline λ`",
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

        event_tx
            .send_blocking(protocol::HostEvent::tree(
                tree(
                    "(defn greet [name]\n  (str \"Welcome, \" name \"!\"))",
                    false,
                    "---\ntitle: Pixel fixture\n---\n**soft break**\n`replacement λ`",
                ),
                None,
                vec![],
            ))
            .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        let markdown_updated = cx.capture_screenshot(handle.into()).unwrap();
        if updated == markdown_updated {
            save_failure("markdown-equal-block-before", &updated);
            save_failure("markdown-equal-block-after", &markdown_updated);
            panic!("equal-block Markdown replacement produced identical pixels");
        }

        // Real image bytes through RootView and Kit Avatar: a Resource variant
        // assertion alone cannot prove that the production avatar loads/paints.
        let image_path = std::env::temp_dir().join(format!(
            "clj-gpui-avatar-{} café #1.png",
            std::process::id()
        ));
        image::RgbaImage::from_pixel(32, 32, image::Rgba([230, 40, 60, 255]))
            .save(&image_path)
            .expect("write local image fixture");
        let file_url = gpui_kit::http_client::Url::from_file_path(&image_path).unwrap();
        let children = [
            ("local-path", "avatar", image_path.to_str().unwrap()),
            ("local-url", "avatar", file_url.as_str()),
            (
                "attachment-path",
                "attachment-media",
                image_path.to_str().unwrap(),
            ),
            ("attachment-url", "attachment-media", file_url.as_str()),
        ]
        .map(|(id, kind, src)| {
            json!({
                "type": "vstack", "id": id, "width": 64, "height": 64,
                "children": [{"type": kind, "src": src, "width": 64, "height": 64}]
            })
        });
        event_tx
            .send_blocking(protocol::HostEvent::tree(
                serde_json::from_value(json!({
                    "type": "window", "chrome": "app", "theme": "light",
                    "padding": 20, "gap": 12, "children": children
                }))
                .unwrap(),
                None,
                vec![],
            ))
            .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        let avatars = cx.capture_screenshot(handle.into()).unwrap();
        let scale = cx
            .update_window(handle.into(), |_, window, _| window.scale_factor())
            .unwrap();
        // HeadlessAppContext captures actual pixels, without TestAppContext's
        // semantic observer. Sample the center of each fixed 64px image slot.
        for row in 0..4 {
            let center_x = 20. + 32.;
            let center_y = 20. + row as f32 * (64. + 12.) + 32.;
            let pixel = avatars.get_pixel((center_x * scale) as u32, (center_y * scale) as u32);
            if pixel.0[..3] != [230, 40, 60] {
                save_failure("avatar-local-sources", &avatars);
                panic!("local image row {row} did not paint fixture pixels: {pixel:?}");
            }
        }
        std::fs::remove_file(image_path).unwrap();

        // Arbitrary slots must paint production widgets, not a placeholder or
        // a static label approximation. Both the affix and the collapsed-body
        // editor go through the deferred renderer.
        let parity_tree = |text: &str| {
            serde_json::from_value(json!({
            "type":"window", "chrome":"app", "theme":"light", "padding":20, "gap":12,
            "children":[
                {"type":"form", "children":[{"type":"field", "text":"Project", "children":[
                    {"type":"input-group", "width":400,"children":[
                        {"type":"input", "id":"parity-input", "text":"Native composition", "prefix":{"type":"hstack","width":20,"height":20,"bg":"#e6283c"}},
                        {"type":"input-group-addon","children":[{"type":"input-group-text","text":".clj"}]}
                    ]}
                ]}]},
                {"type":"collapsible", "open":true, "children":[{"type":"label","text":"Editor in a content slot"}],
                 "content":{"type":"editor","id":"parity-editor","language":"clojure","height":150,"text":text}},
                {"type":"empty", "children":[
                    {"type":"empty-header", "children":[{"type":"empty-title","text":"Native Empty composition"}]},
                    {"type":"empty-content", "children":[{"type":"button","text":"Continue","variant":"primary"}]}
                ]}
            ]
        })).unwrap()
        };
        let mut parity_images = Vec::new();
        for source in ["(def answer 42)", "(def answer 2048)"] {
            event_tx
                .send_blocking(protocol::HostEvent::tree(parity_tree(source), None, vec![]))
                .unwrap();
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
                .unwrap();
            parity_images.push(cx.capture_screenshot(handle.into()).unwrap());
        }
        if parity_images[0] == parity_images[1] {
            save_failure("parity-slot-editor", &parity_images[1]);
            panic!("editing a nested content-slot editor produced identical pixels");
        }
        assert!(
            parity_images[1]
                .pixels()
                .filter(|pixel| pixel.0[..3] == [230, 40, 60])
                .count()
                >= 100,
            "widget prefix did not paint"
        );
        parity_images[1]
            .save("/private/tmp/clj-gpui-parity.png")
            .unwrap();

        // The default pie radius must come from its actual layout, including
        // a flex-filled viewport taller than chart_viewport's 180px fallback.
        // Explicit and partial per-slice radii must still win.
        for (name, outer, items, samples) in [
            (
                "pie-layout-radius",
                None,
                json!([{"label": "A", "value": 1, "color": "#e6283c"}]),
                vec![(160., [230, 40, 60]), (0., [255, 255, 255])],
            ),
            (
                "pie-explicit-radius",
                Some(80.),
                json!([{"label": "A", "value": 1, "color": "#e6283c"}]),
                vec![(60., [230, 40, 60]), (160., [255, 255, 255])],
            ),
            (
                "pie-slice-radius",
                Some(80.),
                json!([
                    {"label": "A", "value": 1, "color": "#e6283c", "outer-radius": 130},
                    {"label": "B", "value": 1, "color": "#2864e6"}
                ]),
                vec![
                    (100., [230, 40, 60]),
                    (-60., [40, 100, 230]),
                    (-100., [255, 255, 255]),
                ],
            ),
        ] {
            let mut chart = json!({
                "type": "chart", "id": "pie", "variant": "pie", "flex": 1,
                "inner-radius": 40, "items": items
            });
            if let Some(radius) = outer {
                chart["outer-radius"] = json!(radius);
            }
            event_tx
                .send_blocking(protocol::HostEvent::tree(
                    serde_json::from_value(json!({
                        "type": "window", "chrome": "app", "theme": "light",
                        "padding": 0, "gap": 0, "bg": "#ffffff", "children": [chart]
                    }))
                    .unwrap(),
                    None,
                    vec![],
                ))
                .unwrap();
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
                .unwrap();
            let pie = cx.capture_screenshot(handle.into()).unwrap();
            for (offset, expected) in samples {
                let pixel = pie.get_pixel(((310. + offset) * scale) as u32, (250. * scale) as u32);
                if pixel.0[..3] != expected {
                    save_failure(name, &pie);
                    panic!("{name} at x offset {offset}: expected {expected:?}, got {pixel:?}");
                }
            }
        }

        println!("rendering: 6 passed (production RootView, Metal)");
    }
}
