//! Headless interaction coverage for the production `RootView` renderer.
//!
//! The fixture speaks the same `HostEvent`/`Cmd` channel contract as the
//! socket bridge. It deliberately renders `Root`, so menus, dialogs,
//! notifications, and other overlay layers use their production owner.

use crate::protocol::{Cmd, HostEvent, Node};
use crate::{renderer::RootView, syntax};
use gpui_kit::component::Root;
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{AppContext as _, TestAppContext, px, size};
use serde_json::json;
use std::sync::mpsc;
use std::time::Duration;

fn fixture_tree(value: &str, button_label: &str, status: &str) -> Node {
    serde_json::from_value(json!({
        "type": "window",
        "chrome": "app",
        "padding": 16,
        "gap": 12,
        "children": [
            {
                "type": "input",
                "id": "name",
                "text": value,
                "accessibility-label": "Name",
                "on-change": "name-change",
                "width": 260
            },
            {
                "type": "button",
                "id": "save",
                "text": button_label,
                "on-click": "save-click"
            },
            {
                "type": "label",
                "id": "status",
                "text": status
            }
        ]
    }))
    .expect("valid renderer fixture")
}

fn drain(receiver: &mpsc::Receiver<Cmd>) -> Vec<Cmd> {
    receiver.try_iter().collect()
}

fn controls_tree(checked: bool, switched: bool, reverse: bool) -> Node {
    let mut children = vec![
        json!({"type": "checkbox", "id": "agree", "text": "Agree",
               "checked": checked, "on-click": "agree-click"}),
        json!({"type": "switch", "id": "notify", "text": "Notify",
               "checked": switched, "on-change": "notify-change"}),
        json!({"type": "button", "id": "locked", "text": "Locked",
               "disabled": true, "on-click": "locked-click"}),
    ];
    if reverse {
        children.reverse();
    }
    serde_json::from_value(json!({
        "type": "window", "chrome": "app", "padding": 16, "gap": 12,
        "children": children
    }))
    .unwrap()
}

#[gpui_kit::test]
async fn production_renderer_round_trips_unicode_and_returned_tree(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
    });

    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(520.), px(300.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    assert!(matches!(cmd_rx.recv().unwrap(), Cmd::Render));

    event_tx
        .send(HostEvent::Tree(
            fixture_tree("", "Save", "Ready"),
            None,
            vec![],
        ))
        .await
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("name").is_some() && window.try_find("save").is_some()
    })
    .await;

    cx.update_window(handle.into(), |_, window, cx| {
        window.click("name", cx);
        window.input("Ada λ🦀", cx);
        let input = window.find("name");
        assert_eq!(input.value(), Some("Ada λ🦀"));
        assert!(input.bounds().size.width > px(0.));
        window.click("save", cx);
        assert!(window.find("save").bounds().size.width > px(0.));
    })
    .unwrap();

    let emitted = drain(&cmd_rx);
    assert!(emitted.iter().any(|cmd| matches!(cmd,
        Cmd::Callback { id, value: Some(value), .. }
        if id == "name-change" && value == &json!("Ada λ🦀")
    )));
    let save_seq = emitted.iter().find_map(|cmd| match cmd {
        Cmd::Callback { id, seq, .. } if id == "save-click" => *seq,
        _ => None,
    });
    assert_eq!(save_seq, Some(1));

    event_tx
        .send(HostEvent::Tree(
            fixture_tree("Ada λ🦀", "Saved", "Clojure accepted Unicode"),
            save_seq,
            vec![],
        ))
        .await
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window
            .try_find("save")
            .is_some_and(|snapshot| snapshot.label() == Some("Saved"))
    })
    .await;

    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let input = window.find("name");
        let button = window.find("save");
        assert_eq!(input.value(), Some("Ada λ🦀"));
        assert_eq!(button.label(), Some("Saved"));
        assert!(input.bounds().bottom() <= button.bounds().top());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn production_renderer_controls_are_controlled_reorderable_and_disabled(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(480.), px(280.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    assert!(matches!(cmd_rx.recv().unwrap(), Cmd::Render));
    event_tx
        .send(HostEvent::Tree(
            controls_tree(false, false, false),
            None,
            vec![],
        ))
        .await
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("notify").is_some()
    })
    .await;

    cx.update_window(handle.into(), |_, window, cx| {
        window.click("agree", cx);
        window.click("notify", cx);
        window.click("locked", cx);
    })
    .unwrap();
    let emitted = drain(&cmd_rx);
    assert!(emitted.iter().any(|cmd| matches!(cmd,
        Cmd::Callback { id, value: None, .. } if id == "agree-click"
    )));
    assert!(emitted.iter().any(|cmd| matches!(cmd,
        Cmd::Callback { id, value: Some(value), .. }
        if id == "notify-change" && value == &json!(true)
    )));
    assert!(!emitted.iter().any(|cmd| matches!(cmd,
        Cmd::Callback { id, .. } if id == "locked-click"
    )));

    event_tx
        .send(HostEvent::Tree(
            controls_tree(true, true, true),
            None,
            vec![],
        ))
        .await
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window
            .try_find("agree")
            .is_some_and(|s| s.checked() == Some(true))
            && window
                .try_find("notify")
                .is_some_and(|s| s.checked() == Some(true))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("locked").bounds().bottom() <= window.find("notify").bounds().top());
        assert!(window.find("notify").bounds().bottom() <= window.find("agree").bounds().top());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn production_select_is_accessible_and_commits_once(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(480.), px(280.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    assert!(matches!(cmd_rx.recv().unwrap(), Cmd::Render));
    let select_tree = |value: &str| {
        serde_json::from_value(json!({
            "type": "window", "chrome": "app", "padding": 16,
            "children": [{
                "type": "select", "id": "language", "value": value,
                "accessibility-label": "Language", "on-change": "language-change",
                "options": [
                    {"id": "clj", "label": "Clojure"},
                    {"id": "rs", "label": "Rust"}
                ]
            }]
        }))
        .unwrap()
    };
    event_tx
        .send(HostEvent::Tree(select_tree("clj"), None, vec![]))
        .await
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("language").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        let select = window.find("language");
        assert_eq!(select.label(), Some("Language"));
        assert_eq!(select.value(), Some("Clojure"));
        assert_eq!(select.expanded(), Some(false));
        window.within("language").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_millis(200), |window, _| {
        window.find("language").expanded() == Some(false)
    })
    .await;
    let callbacks: Vec<_> = drain(&cmd_rx)
        .into_iter()
        .filter(|cmd| matches!(cmd, Cmd::Callback { id, .. } if id == "language-change"))
        .collect();
    assert_eq!(callbacks.len(), 1);
    assert!(matches!(&callbacks[0],
        Cmd::Callback { value: Some(value), .. } if value == &json!("rs")
    ));

    event_tx
        .send(HostEvent::Tree(select_tree("rs"), None, vec![]))
        .await
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.find("language").value() == Some("Rust")
    })
    .await;
}
