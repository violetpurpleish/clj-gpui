//! Headless interaction coverage for the production `RootView` renderer.
//!
//! The fixture speaks the same `HostEvent`/`Cmd` channel contract as the
//! socket bridge. It deliberately renders `Root`, so menus, dialogs,
//! notifications, and other overlay layers use their production owner.

use crate::protocol::{Cmd, HostEvent, Node};
use crate::{renderer::RootView, syntax};
use gpui_kit::component::Root;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::Position;
use gpui_kit::component::slider::SliderValue;
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{
    AppContext as _, Entity, EntityInputHandler as _, ScrollDelta, TestAppContext, WindowHandle,
    point, px, size,
};
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

fn callback_pairs(commands: &[Cmd]) -> Vec<(String, Option<serde_json::Value>)> {
    commands
        .iter()
        .flat_map(|cmd| match cmd {
            Cmd::Callback { id, value, .. } => vec![(id.clone(), value.clone())],
            Cmd::CallbackBatch { callbacks, .. } => callbacks
                .iter()
                .map(|call| (call.id.clone(), call.value.clone()))
                .collect(),
            _ => Vec::new(),
        })
        .collect()
}

fn callback_sequence(commands: &[Cmd]) -> Option<u64> {
    commands.iter().find_map(|cmd| match cmd {
        Cmd::Callback { seq, .. } | Cmd::CallbackBatch { seq, .. } => *seq,
        _ => None,
    })
}

fn production_view(handle: WindowHandle<Root>, cx: &mut TestAppContext) -> Entity<RootView> {
    let root = handle.root(cx).expect("production Root entity");
    root.read_with(cx, |root, _| {
        root.view()
            .clone()
            .downcast::<RootView>()
            .expect("RootView child")
    })
}

fn settle_root(handle: WindowHandle<Root>, cx: &mut TestAppContext) {
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(500));
    for _ in 0..3 {
        cx.update_window(handle.into(), |_, window, cx| {
            window.simulate_next_frame(cx);
            window.render_frame(cx);
        })
        .expect("test window remains open");
        cx.run_until_parked();
    }
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

#[gpui_kit::test]
async fn production_slider_keeps_identity_and_controlled_value_across_tree_changes(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(520.), px(260.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    assert!(matches!(cmd_rx.recv().unwrap(), Cmd::Render));
    let slider_tree = |value: f32, extra: bool| {
        let mut children = vec![json!({
            "type": "slider", "id": "volume", "value": value,
            "min": 0, "max": 100, "step": 1, "height": 28,
            "on-change": "volume-change", "on-release": "volume-release"
        })];
        if extra {
            children.insert(0, json!({"type": "label", "text": "Unrelated rerender"}));
        }
        serde_json::from_value(json!({
            "type": "window", "chrome": "app", "padding": 16, "gap": 12,
            "children": children
        }))
        .unwrap()
    };

    event_tx
        .send(HostEvent::Tree(slider_tree(20., false), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let view = production_view(handle, cx);
    let first_state = view.read_with(cx, |view, _| view.test_slider_state("volume").unwrap());
    assert_eq!(
        view.read_with(cx, |view, cx| view.test_slider_value("volume", cx)),
        Some(SliderValue::Single(20.))
    );

    event_tx
        .send(HostEvent::Tree(slider_tree(65., true), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        view.read_with(cx, |view, cx| view.test_slider_value("volume", cx)),
        Some(SliderValue::Single(65.))
    );
    assert_eq!(
        first_state.entity_id(),
        view.read_with(cx, |view, _| {
            view.test_slider_state("volume").unwrap().entity_id()
        }),
        "an unrelated sibling must not recreate controlled slider state"
    );

    cx.update_window(handle.into(), |_, window, cx| {
        let mut slider = window.within("volume-track");
        let track = slider.find("slider-bar-container");
        slider.click_at(
            "slider-bar-container",
            point(
                track.bounds().size.width * 0.8,
                track.bounds().size.height / 2.,
            ),
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().timer(Duration::from_millis(20)).await;
    let emitted = drain(&cmd_rx);
    let emitted_pairs = callback_pairs(&emitted);
    assert!(
        emitted_pairs
            .iter()
            .any(|(id, value)| id == "volume-change" && value.is_some())
    );
    assert!(
        emitted_pairs
            .iter()
            .any(|(id, value)| id == "volume-release" && value.is_some())
    );

    let without_slider: Node = serde_json::from_value(json!({
        "type": "window", "chrome": "app", "children": [
            {"type": "label", "id": "absent-marker", "text": "Temporarily absent"}
        ]
    }))
    .unwrap();
    event_tx
        .send(HostEvent::Tree(without_slider, None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        view.read_with(cx, |view, _| view.test_tree_child_ids()),
        vec![Some("absent-marker".into())]
    );
    assert_eq!(
        first_state.entity_id(),
        view.read_with(cx, |view, _| {
            view.test_slider_state("volume").unwrap().entity_id()
        }),
        "slider state intentionally survives temporary unmounts so cached native bounds stay valid"
    );

    event_tx
        .send(HostEvent::Tree(slider_tree(35., false), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        view.read_with(cx, |view, cx| view.test_slider_value("volume", cx)),
        Some(SliderValue::Single(35.))
    );
    assert_eq!(
        first_state.entity_id(),
        view.read_with(cx, |view, _| {
            view.test_slider_state("volume").unwrap().entity_id()
        }),
        "remounting the same explicit id must recover the retained slider state"
    );
}

fn editor_tree(language: &str, text: &str) -> Node {
    serde_json::from_value(json!({
        "type": "window", "chrome": "app", "padding": 16,
        "children": [{
            "type": "editor", "id": "source", "language": language,
            "text": text, "auto-close": true, "smart-indent": true,
            "on-change": "source-change", "height": 220
        }]
    }))
    .unwrap()
}

#[gpui_kit::test]
async fn production_editor_retains_crlf_search_and_identity_across_language_changes(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
    });
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(620.), px(360.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    assert!(matches!(cmd_rx.recv().unwrap(), Cmd::Render));
    let original = "(defn greet []\r\n  \"Ada\")\r\n; greet";

    event_tx
        .send(HostEvent::Tree(
            editor_tree("clojure", original),
            None,
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    let view = production_view(handle, cx);
    let editor = view.read_with(cx, |view, _| view.test_editor_state("source").unwrap());
    let editor_id = editor.entity_id();
    editor.update(cx, |editor, cx| {
        assert_eq!(editor.language_name().as_ref(), "clojure");
        assert_eq!(editor.value().as_ref(), original);
        let revision = editor.search_activation_revision();
        editor.open_search(false, cx);
        assert_eq!(editor.search_activation_revision(), revision + 1);
        editor.set_search_query("greet", false, cx);
        assert_eq!(
            editor.search_session().matcher.matched_ranges().as_ref(),
            &[6..11, 28..33]
        );
        assert_eq!(editor.next_search_match(cx), Some(28..33));
        let active_match = editor.search_session().matcher.current_match_index();
        editor.open_search(false, cx);
        assert_eq!(editor.search_activation_revision(), revision + 2);
        assert_eq!(
            editor.search_session().matcher.current_match_index(),
            active_match
        );
        editor.close_search(cx);
        assert!(!editor.search_session().open);
    });

    cx.update_window(handle.into(), |_, window, cx| {
        editor.update(cx, |editor, cx| {
            editor.set_cursor_position(
                Position {
                    line: 2,
                    character: 7,
                },
                window,
                cx,
            );
            editor.replace_text_in_range(None, "(", window, cx);
        });
    })
    .unwrap();
    let edited = editor.read_with(cx, |editor, _| editor.value().to_string());
    assert!(
        edited.ends_with("()"),
        "editor value after auto-close: {edited:?}"
    );

    event_tx
        .send(HostEvent::Tree(editor_tree("rust", original), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let retained = view.read_with(cx, |view, _| view.test_editor_state("source").unwrap());
    assert_eq!(retained.entity_id(), editor_id);
    retained.read_with(cx, |editor, _| {
        assert_eq!(editor.language_name().as_ref(), "rust");
        assert!(editor.value().ends_with("()"));
        assert!(editor.value().contains("\r\n"));
    });
}

fn collection_tree(shrunk: bool) -> Node {
    let list_items = if shrunk {
        json!([{"id": "alpha", "label": "Alpha"}])
    } else {
        json!([
            {"id": "alpha", "label": "Alpha"},
            {"id": "beta", "label": "Beta"}
        ])
    };
    let table_rows = if shrunk {
        json!([{"id": "grace", "cells": ["Grace", "COBOL"]}])
    } else {
        json!([
            {"id": "ada", "cells": ["Ada", "Clojure"]},
            {"id": "grace", "cells": ["Grace", "COBOL"]}
        ])
    };
    serde_json::from_value(json!({
        "type": "window", "chrome": "app", "padding": 12, "gap": 8,
        "children": [
            {"type": "list", "id": "tasks", "items": list_items,
             "value": if shrunk { serde_json::Value::Null } else { json!("beta") },
             "height": 100, "on-change": "task-change"},
            {"type": "data-table", "id": "people",
             "options": [{"id": "name", "label": "Name"},
                         {"id": "language", "label": "Language"}],
             "items": table_rows,
             "value": if shrunk { serde_json::Value::Null } else { json!("ada") },
             "height": 120, "on-change": "person-change"},
            {"type": "tree", "id": "files", "value": "lib",
             "items": [{"id": "src", "label": "src", "expanded": !shrunk,
                        "items": [{"id": "lib", "label": "lib.rs"}]}],
             "height": 100, "on-change": "file-change"},
            {"type": "resizable", "id": "split", "orientation": "horizontal",
             "height": 100,
             "children": [{"type": "label", "text": "Left"},
                          {"type": "label", "text": "Right"}]},
            {"type": "dock", "id": "workspace", "height": 140,
             "items": [{"id": "files-panel", "side": "left", "label": "Files",
                        "content": {"type": "label", "text": "Dock content"}}]}
        ]
    }))
    .unwrap()
}

#[gpui_kit::test]
async fn production_collections_shrink_selection_and_retain_layout_state(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(720.), px(760.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    assert!(matches!(cmd_rx.recv().unwrap(), Cmd::Render));
    event_tx
        .send(HostEvent::Tree(collection_tree(false), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let view = production_view(handle, cx);
    let initial_list = view.read_with(cx, |view, cx| view.test_list_state("tasks", cx).unwrap());
    assert_eq!(
        (initial_list.0.as_slice(), initial_list.1),
        (
            ["alpha".to_string(), "beta".to_string()].as_slice(),
            Some(1)
        )
    );
    let list_id = initial_list.2;
    let (table_rows, table_selection, table_id) =
        view.read_with(cx, |view, cx| view.test_table_state("people", cx).unwrap());
    assert_eq!(table_rows, vec!["ada", "grace"]);
    assert_eq!(table_selection, Some(0));
    let (tree_selection, tree_id) =
        view.read_with(cx, |view, cx| view.test_tree_state("files", cx).unwrap());
    assert_eq!(tree_selection.as_deref(), Some("lib"));
    let layout_ids = view.read_with(cx, |view, _| {
        view.test_layout_state_ids("split", "workspace")
    });
    assert!(layout_ids.0.is_some() && layout_ids.1.is_some());
    let panel_sizes = view.read_with(cx, |view, cx| {
        view.test_resizable_sizes("split", cx).unwrap()
    });
    assert_eq!(panel_sizes.len(), 2);
    assert!(panel_sizes.iter().all(|size| *size > 0.));

    event_tx
        .send(HostEvent::Tree(collection_tree(true), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let list = view.read_with(cx, |view, cx| view.test_list_state("tasks", cx).unwrap());
    assert_eq!(list.0, vec!["alpha"]);
    assert_eq!(list.1, None);
    assert_eq!(list.2, list_id);
    let (table_rows, table_selection, retained_table_id) =
        view.read_with(cx, |view, cx| view.test_table_state("people", cx).unwrap());
    assert_eq!(table_rows, vec!["grace"]);
    assert_eq!(table_selection, None);
    assert_eq!(retained_table_id, table_id);
    let (tree_selection, retained_tree_id) =
        view.read_with(cx, |view, cx| view.test_tree_state("files", cx).unwrap());
    assert_eq!(tree_selection.as_deref(), Some("lib"));
    assert_eq!(retained_tree_id, tree_id);
    assert_eq!(
        view.read_with(cx, |view, _| view
            .test_layout_state_ids("split", "workspace")),
        layout_ids
    );
}

#[gpui_kit::test]
async fn production_table_only_queries_painted_virtual_rows(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(640.), px(360.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    assert!(matches!(cmd_rx.recv().unwrap(), Cmd::Render));
    let rows: Vec<_> = (0..200)
        .map(|row| json!({"id": format!("row-{row}"), "cells": [format!("Row {row}")]}))
        .collect();
    let tree: Node = serde_json::from_value(json!({
        "type": "window", "chrome": "app", "padding": 16,
        "children": [{
            "type": "data-table", "id": "records", "height": 280,
            "options": [{"id": "name", "label": "Name", "width": 300}],
            "items": rows
        }]
    }))
    .unwrap();
    event_tx
        .send(HostEvent::Tree(tree, None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);

    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find(("row", 150usize)).is_none());
        window.click(("row", 1usize), cx);
        assert_eq!(window.find(("row", 1usize)).selected(), Some(true));
        window.press("down", cx);
        assert_eq!(window.find(("row", 2usize)).selected(), Some(true));
        let viewport = window.find("table").bounds();
        for _ in 0..30 {
            window.press("down", cx);
        }
        let selected = window.find(("row", 32usize));
        assert_eq!(selected.selected(), Some(true));
        assert!(selected.bounds().top() >= viewport.top());
        assert!(selected.bounds().bottom() <= viewport.bottom());
        assert!(window.try_find(("row", 1usize)).is_none());
        window.scroll("table", ScrollDelta::Pixels(point(px(0.), px(2000.))), cx);
        assert!(window.find(("row", 1usize)).visible());
        assert!(window.try_find(("row", 32usize)).is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn production_calendar_days_are_accessible_and_selectable(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(640.), px(600.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    assert!(matches!(cmd_rx.recv().unwrap(), Cmd::Render));
    let tree: Node = serde_json::from_value(json!({
        "type": "window", "chrome": "app", "padding": 16,
        "children": [{
            "type": "date-picker", "id": "release-date", "value": "2026-09-15",
            "on-change": "date-change", "cleanable": true
        }]
    }))
    .unwrap();
    event_tx
        .send(HostEvent::Tree(tree, None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let view = production_view(handle, cx);
    let picker_id = view.read_with(cx, |view, _| {
        view.test_date_state_id("release-date").unwrap()
    });

    cx.update_window(handle.into(), |_, window, cx| {
        let picker = ("date-picker", picker_id);
        assert_eq!(window.find(picker).expanded(), Some(false));
        window.click(picker, cx);
        assert_eq!(window.find(picker).expanded(), Some(true));
        let day = window.find("calendar-2026-09-16-0-2");
        assert_eq!(day.label(), Some("2026-09-16"));
        assert!(day.bounds().size.width > px(0.));
        window.click("calendar-2026-09-16-0-2", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let callbacks = drain(&cmd_rx);
    assert!(
        callback_pairs(&callbacks)
            .iter()
            .any(|(id, value)| id == "date-change" && value.as_ref() == Some(&json!("2026-09-16")))
    );
}

fn dialog_tree(open: bool, generation: &str) -> Node {
    serde_json::from_value(json!({
        "type": "window", "chrome": "app", "padding": 16, "children": [
            {"type": "input", "id": "before-dialog", "text": "focus target"},
            {
                "type": "alert-dialog", "id": "confirm", "open": open,
                "variant": "confirm", "title": format!("Confirm {generation}"),
                "close-button": true,
                "on-ok": format!("ok-{generation}"),
                "on-cancel": format!("cancel-{generation}"),
                "on-close": format!("close-{generation}"),
                "children": [{"type": "label", "text": "Continue?"}]
            }
        ]
    }))
    .unwrap()
}

#[gpui_kit::test]
async fn production_dialog_uses_live_callbacks_closes_reopens_and_restores_focus(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(560.), px(340.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    assert!(matches!(cmd_rx.recv().unwrap(), Cmd::Render));

    event_tx
        .send(HostEvent::Tree(dialog_tree(false, "1"), None, vec![]))
        .await
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("before-dialog").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("before-dialog", cx);
        assert_eq!(window.find("before-dialog").focused(), Some(true));
    })
    .unwrap();

    event_tx
        .send(HostEvent::Tree(dialog_tree(true, "1"), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let view = production_view(handle, cx);
    assert_eq!(
        view.read_with(cx, |view, _| view.test_dialog_state()),
        (1, vec!["confirm".into()], false)
    );
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, cx| {
        window.has_active_dialog(cx)
    })
    .await;
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("dialog").is_some() && window.try_find("ok").is_some()
    })
    .await;
    event_tx
        .send(HostEvent::Tree(dialog_tree(true, "2"), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert!(
        cx.update_window(handle.into(), |_, window, _| window
            .try_find("ok")
            .is_some())
            .unwrap()
    );
    cx.update_window(handle.into(), |_, window, cx| window.click("ok", cx))
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("dialog").is_none()
    })
    .await;
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("before-dialog").focused(), Some(true));
    })
    .unwrap();
    let ok_commands = drain(&cmd_rx);
    let ok_sequence = callback_sequence(&ok_commands);
    let ids: Vec<_> = callback_pairs(&ok_commands)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids, vec!["ok-2", "close-2"]);

    event_tx
        .send(HostEvent::Tree(
            dialog_tree(false, "2"),
            ok_sequence,
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    event_tx
        .send(HostEvent::Tree(dialog_tree(true, "3"), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("dialog").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| window.press("escape", cx))
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("dialog").is_none()
    })
    .await;
    let cancel_commands = drain(&cmd_rx);
    let cancel_sequence = callback_sequence(&cancel_commands);
    let ids: Vec<_> = callback_pairs(&cancel_commands)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids, vec!["cancel-3", "close-3"]);

    event_tx
        .send(HostEvent::Tree(
            dialog_tree(false, "3"),
            cancel_sequence,
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    event_tx
        .send(HostEvent::Tree(dialog_tree(true, "4"), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("close").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| window.click("close", cx))
        .unwrap();
    let close_commands = drain(&cmd_rx);
    let ids: Vec<_> = callback_pairs(&close_commands)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids, vec!["cancel-4", "close-4"]);
}

#[gpui_kit::test]
async fn production_sheet_mounts_after_deferred_open_and_closes_from_escape(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(620.), px(420.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    assert!(matches!(cmd_rx.recv().unwrap(), Cmd::Render));
    let tree: Node = serde_json::from_value(json!({
        "type": "window", "chrome": "app", "children": [{
            "type": "sheet", "id": "inspector", "open": true,
            "title": "Inspector", "placement": "right",
            "on-close": "sheet-close", "on-open-change": "sheet-open-change",
            "children": [{"type": "label", "text": "Sheet body"}]
        }]
    }))
    .unwrap();
    event_tx
        .send(HostEvent::Tree(tree, None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, cx| {
        window.has_active_sheet(cx) && window.try_find("sheet-content").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| window.press("escape", cx))
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, cx| {
        !window.has_active_sheet(cx)
    })
    .await;
    let commands = drain(&cmd_rx);
    assert_eq!(
        callback_pairs(&commands),
        vec![
            ("sheet-close".into(), None),
            ("sheet-open-change".into(), Some(json!(false)))
        ]
    );
}

#[gpui_kit::test]
async fn production_nested_menu_emits_leaf_then_parent_callback(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(480.), px(280.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    assert!(matches!(cmd_rx.recv().unwrap(), Cmd::Render));
    let tree: Node = serde_json::from_value(json!({
        "type": "window", "chrome": "app", "padding": 16, "children": [{
            "type": "dropdown-menu", "id": "actions", "on-change": "menu-change",
            "trigger": {"type": "button", "text": "Actions"},
            "items": [{
                "id": "file", "label": "File", "items": [{
                    "id": "export", "label": "Export", "on-click": "export-click"
                }]
            }]
        }]
    }))
    .unwrap();
    event_tx
        .send(HostEvent::Tree(tree, None, vec![]))
        .await
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("actions-trigger").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("actions-trigger", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("popup-menu").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.within("popup-menu").hover(0usize, cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.try_find("submenu").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        window
            .within("submenu")
            .within("popup-menu")
            .click(0usize, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().timer(Duration::from_millis(20)).await;
    let commands = drain(&cmd_rx);
    let callbacks = callback_pairs(&commands);
    assert_eq!(
        callbacks,
        vec![
            ("export-click".into(), None),
            ("menu-change".into(), Some(json!(["file", "export"])))
        ]
    );
}
