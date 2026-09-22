//! Drive GPUI's real sink/receiver/subscription path with a deterministic
//! platform text system. Installed fonts cannot make these fixtures flaky.
use crate::overlay::{CallbackQueue, QueuedAction};
use crate::protocol::{Cmd, HostEvent, Node};
use crate::renderer::RootView;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{self as gpui, AppContext as _};
use serde_json::json;
use std::borrow::Cow;
use std::sync::{Arc, Mutex, mpsc};

#[derive(Default)]
struct ReportingTextSystem {
    sink: Mutex<Option<Arc<dyn gpui::MissingGlyphSink>>>,
    registrations: Mutex<usize>,
}

impl ReportingTextSystem {
    fn enabled(&self) -> bool {
        self.sink.lock().unwrap().is_some()
    }

    fn report(&self, glyphs: Vec<gpui::MissingGlyph>) {
        let sink = self.sink.lock().unwrap().clone();
        if let Some(sink) = sink {
            sink.report(glyphs);
        }
    }
}

impl gpui::PlatformTextSystem for ReportingTextSystem {
    fn set_missing_glyph_sink(&self, sink: Option<Arc<dyn gpui::MissingGlyphSink>>) {
        if sink.is_some() {
            *self.registrations.lock().unwrap() += 1;
        }
        *self.sink.lock().unwrap() = sink;
    }

    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> anyhow::Result<()> {
        gpui::NoopTextSystem.add_fonts(fonts)
    }
    fn all_font_names(&self) -> Vec<String> {
        gpui::NoopTextSystem.all_font_names()
    }
    fn font_id(&self, descriptor: &gpui::Font) -> anyhow::Result<gpui::FontId> {
        gpui::NoopTextSystem.font_id(descriptor)
    }
    fn font_metrics(&self, id: gpui::FontId) -> gpui::FontMetrics {
        gpui::NoopTextSystem.font_metrics(id)
    }
    fn typographic_bounds(
        &self,
        font: gpui::FontId,
        glyph: gpui::GlyphId,
    ) -> anyhow::Result<gpui::Bounds<f32>> {
        gpui::NoopTextSystem.typographic_bounds(font, glyph)
    }
    fn advance(&self, font: gpui::FontId, glyph: gpui::GlyphId) -> anyhow::Result<gpui::Size<f32>> {
        gpui::NoopTextSystem.advance(font, glyph)
    }
    fn glyph_for_char(&self, font: gpui::FontId, ch: char) -> Option<gpui::GlyphId> {
        gpui::NoopTextSystem.glyph_for_char(font, ch)
    }
    fn glyph_raster_bounds(
        &self,
        params: &gpui::RenderGlyphParams,
    ) -> anyhow::Result<gpui::Bounds<gpui::DevicePixels>> {
        gpui::NoopTextSystem.glyph_raster_bounds(params)
    }
    fn rasterize_glyph(
        &self,
        params: &gpui::RenderGlyphParams,
        bounds: gpui::Bounds<gpui::DevicePixels>,
    ) -> anyhow::Result<(gpui::Size<gpui::DevicePixels>, Vec<u8>)> {
        gpui::NoopTextSystem.rasterize_glyph(params, bounds)
    }
    fn layout_line(
        &self,
        text: &str,
        size: gpui::Pixels,
        runs: &[gpui::FontRun],
    ) -> gpui::LineLayout {
        gpui::NoopTextSystem.layout_line(text, size, runs)
    }
    fn recommended_rendering_mode(
        &self,
        font: gpui::FontId,
        size: gpui::Pixels,
    ) -> gpui::TextRenderingMode {
        gpui::NoopTextSystem.recommended_rendering_mode(font, size)
    }
}

fn glyph(text: &str, class: gpui::FallbackFontClass) -> gpui::MissingGlyph {
    gpui::MissingGlyph::new(text.to_string().into(), class)
}

fn tree(callback: Option<&str>) -> Node {
    serde_json::from_value(json!({
        "type": "window", "chrome": "app", "on-missing-glyphs": callback,
        "children": [{"type": "label", "text": "Font diagnostics"}]
    }))
    .unwrap()
}

fn callback(receiver: &mpsc::Receiver<Cmd>, id: &str, value: serde_json::Value) -> u64 {
    let commands: Vec<_> = receiver.try_iter().collect();
    assert_eq!(commands.len(), 1, "{commands:?}");
    let Cmd::Callback {
        id: actual,
        value: payload,
        seq,
    } = &commands[0]
    else {
        panic!("expected a callback: {commands:?}");
    };
    assert_eq!(actual, id);
    assert_eq!(payload.as_ref(), Some(&value));
    seq.expect("diagnostics participate in callback sequencing")
}

#[test]
fn missing_glyph_subscription_tracks_live_handler_and_window_lifetime() {
    use gpui::FallbackFontClass::{Monospace, Proportional};
    let text_system = Arc::new(ReportingTextSystem::default());
    let cx = &mut gpui::TestAppContext::build_with_text_system(
        gpui::TestDispatcher::new(0),
        None,
        text_system.clone(),
    );
    cx.update(gpui::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(gpui::size(gpui::px(600.), gpui::px(400.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    cx.run_until_parked();
    let _ = cmd_rx.try_iter().collect::<Vec<_>>();
    let install = |node, seq, cx: &mut gpui::TestAppContext| {
        event_tx
            .send_blocking(HostEvent::tree(node, seq, vec![]))
            .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
    };

    // Omitted/nil, nested and non-window callbacks do not enable collection.
    let mut nested = tree(None);
    nested.children[0].on_missing_glyphs = Some("nested".into());
    install(nested, None, cx);
    assert!(!text_system.enabled());
    let mut not_window = tree(Some("wrong-root"));
    not_window.kind = "vstack".into();
    install(not_window, None, cx);
    install(tree(Some("")), None, cx);
    text_system.report(vec![glyph("ignored", Proportional)]);
    cx.run_until_parked();
    assert!(!text_system.enabled());
    assert!(cmd_rx.try_recv().is_err());

    install(tree(Some("old")), None, cx);
    assert!(text_system.enabled());
    let emoji = glyph("👩‍💻", Proportional);
    text_system.report(vec![emoji.clone(), emoji.clone(), glyph("界", Monospace)]);
    cx.run_until_parked();
    let seq = callback(
        &cmd_rx,
        "old",
        json!([
            {"grapheme": "👩‍💻", "font-class": "proportional"},
            {"grapheme": "界", "font-class": "monospace"}
        ]),
    );
    install(tree(Some("fresh")), Some(seq), cx);
    assert_eq!(*text_system.registrations.lock().unwrap(), 1);
    text_system.report(vec![emoji.clone()]);
    cx.run_until_parked();
    assert!(
        cmd_rx.try_recv().is_err(),
        "rerenders must not reset GPUI deduplication"
    );

    event_tx.send_blocking(HostEvent::RenderRequested).unwrap();
    cx.run_until_parked();
    assert!(matches!(cmd_rx.try_recv(), Ok(Cmd::Render)));
    text_system.report(vec![glyph("e\u{301}", Proportional)]);
    cx.run_until_parked();
    assert!(
        cmd_rx.try_recv().is_err(),
        "wait for the new callback registry"
    );
    install(tree(Some("newest")), None, cx);
    let seq = callback(
        &cmd_rx,
        "newest",
        json!([
            {"grapheme": "e\u{301}", "font-class": "proportional"}
        ]),
    );
    text_system.report(vec![glyph("discard-pending", Monospace)]);
    cx.run_until_parked();
    assert!(
        cmd_rx.try_recv().is_err(),
        "wait for in-flight callback acknowledgement"
    );
    install(tree(None), Some(seq), cx);
    assert!(!text_system.enabled());
    assert!(
        cmd_rx.try_recv().is_err(),
        "removing the handler discards queued reports"
    );

    install(tree(Some("re-enabled")), None, cx);
    text_system.report(vec![emoji.clone()]);
    cx.run_until_parked();
    let seq = callback(
        &cmd_rx,
        "re-enabled",
        json!([
            {"grapheme": "👩‍💻", "font-class": "proportional"}
        ]),
    );
    install(tree(Some("re-enabled")), Some(seq), cx);
    event_tx
        .send_blocking(HostEvent::Error("callback failed".into()))
        .unwrap();
    cx.run_until_parked();
    assert!(
        !text_system.enabled(),
        "an error view must not keep calling a failed handler"
    );
    install(tree(Some("recovered")), None, cx);
    text_system.report(vec![emoji]);
    cx.run_until_parked();
    callback(
        &cmd_rx,
        "recovered",
        json!([
            {"grapheme": "👩‍💻", "font-class": "proportional"}
        ]),
    );
    cx.update_window(handle.into(), |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    assert!(
        !text_system.enabled(),
        "closing the window must drop the subscription"
    );
}

#[test]
fn missing_glyph_batches_are_bounded_and_preserve_ui_actions() {
    let reports = |range: std::ops::Range<usize>| {
        QueuedAction::MissingGlyphs(
            range
                .map(|n| {
                    glyph(
                        &format!("cluster-{n}"),
                        gpui::FallbackFontClass::Proportional,
                    )
                })
                .collect(),
        )
    };
    let mut queue = CallbackQueue::default();
    queue.render_requested();
    queue.push(reports(0..600));
    queue.push(QueuedAction::ButtonClick {
        key: "button".into(),
    });
    queue.push(reports(500..1500));
    let mut current = tree(Some("current"));
    current.children.push(
        serde_json::from_value(json!({
            "type": "button", "id": "button", "on-click": "clicked"
        }))
        .unwrap(),
    );
    assert!(queue.next(&current).is_none());
    queue.tree_installed(None);
    let calls = queue.next(&current).unwrap();
    assert_eq!(calls[0].id, "current");
    let value = calls[0].value.as_ref().unwrap().as_array().unwrap();
    assert_eq!(value.len(), 1024);
    assert_eq!(value[1023]["grapheme"], "cluster-1023");
    assert_eq!(queue.next(&current).unwrap()[0].id, "clicked");
    assert!(queue.next(&current).is_none());

    queue.push(reports(0..1500));
    queue.push(QueuedAction::ButtonClick {
        key: "button".into(),
    });
    queue.clear_missing_glyphs();
    assert_eq!(queue.next(&current).unwrap()[0].id, "clicked");
    assert!(queue.next(&current).is_none());
}
