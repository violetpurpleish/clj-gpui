mod action_bridge;
mod bridge;
mod catalog;
mod chat;
mod extra;
mod mapping;
mod overlay;
mod preview;
mod protocol;
mod renderer;
#[cfg(test)]
mod renderer_integration_tests;
mod rows;
mod syntax;

use anyhow::Result;
use gpui_kit::application;
use gpui_kit::assets as gpui_kit_assets;
use std::sync::Arc;

fn main() -> Result<()> {
    if let Some(request) = preview::parse_capture_args(std::env::args()) {
        preview::run_helper(request);
        return Ok(());
    }

    let protocol_test = std::env::args().any(|a| a == "--protocol-test");
    if protocol_test {
        return bridge::protocol_test();
    }

    let host = bridge::start()?;
    let nrepl_port = host.nrepl_port;
    let cmd_tx = host.cmd_tx.clone();
    let event_rx = host.event_rx.clone();

    // Kit Avatar paints the image slot whenever `:src` is set; without an
    // HTTP client remote URLs stay empty circles (NullHttpClient). Same
    // requirement as Kit Storybook / the text_max_lines example.
    let http_client = reqwest_client::ReqwestClient::user_agent("clj-gpui/host")?;

    application()
        // 0.6.1 exposes the shared complete Lucide catalog. Clojure icon
        // names may use any member, while Component's own defaults keep
        // resolving from the same source.
        .with_assets(gpui_kit_assets::AllAssets)
        .with_http_client(Arc::new(http_client))
        .run(move |cx| {
            gpui_kit::init(cx);
            syntax::init(cx);
            renderer::open_window(nrepl_port, cmd_tx, event_rx, cx);
        });

    drop(host);
    Ok(())
}
