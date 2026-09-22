//! Headless interaction coverage for the production `RootView` renderer.
//!
//! The fixture speaks the same `HostEvent`/`Cmd` channel contract as the
//! socket bridge. It deliberately renders `Root`, so menus, dialogs,
//! notifications, and other overlay layers use their production owner.

use crate::protocol::{Cmd, HostEvent, Node};
use crate::{renderer::RootView, syntax};
use gpui_kit::base::TextSelection;
use gpui_kit::component::Root;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::Position;
use gpui_kit::component::slider::SliderValue;
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{
    AppContext as _, Axis, Entity, EntityInputHandler as _, InputEvent as _, Modifiers,
    MouseButton, ScrollDelta, TestAppContext, VisualTestContext, WindowHandle, point, px, size,
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
    paint_root(handle, cx);
}

fn paint_root(handle: WindowHandle<Root>, cx: &mut TestAppContext) {
    cx.run_until_parked();
    for _ in 0..3 {
        cx.update_window(handle.into(), |_, window, cx| {
            window.simulate_next_frame(cx);
            window.render_frame(cx);
        })
        .expect("test window remains open");
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
async fn masked_labels_render_secondary_and_highlights_after_tree_updates(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (cmd_tx, _cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(640.), px(360.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    // Exercise the real Kit Label instead of duplicating its highlight math.
    // The old upstream implementation panicked on these masked byte ranges.
    for masked in [true, false, true] {
        event_tx
            .send(HostEvent::tree(
                serde_json::from_value(json!({
                    "type": "window", "chrome": "app", "gap": 12,
                    "children": [
                        {"type": "label", "text": "Hello", "secondary": "World", "masked": masked},
                        {"type": "label", "text": "Hello", "highlights": "ell", "masked": masked},
                        {"type": "label", "text": "é🙂", "secondary": "世界",
                         "highlights": "🙂 世", "masked": masked},
                        {"type": "label", "text": "café",
                         "secondary": "世界", "highlights": "ca", "highlights-match": "prefix",
                         "masked": masked},
                        {"type": "button", "id": "after-labels", "text": "Continue"}
                    ]
                }))
                .unwrap(),
                None,
                vec![],
            ))
            .await
            .unwrap();
        settle_root(handle, cx);
        cx.update_window(handle.into(), |_, window, _| {
            // Labels have no Kit observation record; the following button
            // confirms the frame completed and the label rows took space.
            let button = window.find("after-labels");
            assert!(button.visible());
            assert!(button.bounds().origin.y > px(48.));
        })
        .unwrap();
    }
}

#[gpui_kit::test]
async fn title_bar_renders_styled_children_and_keeps_title_updates(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
    });
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(640.), px(360.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let mut tree: Node = serde_json::from_value(json!({
        "type": "window", "title": "Toolbar", "chrome": "app",
        "children": [
            {"type": "title-bar", "height": 56, "bg": "#112233", "children": [
                {"type": "hstack", "flex": 1, "justify": "between", "children": [
                    {"type": "label", "text": "App"},
                    {"type": "button", "id": "refresh", "text": "Refresh", "on-click": "refresh-old"}
                ]}
            ]},
            {"type": "button", "id": "content", "text": "Content"}
        ]
    })).unwrap();
    event_tx
        .send(HostEvent::tree(tree.clone(), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    // Read the headless platform's recorded set_window_title call, rather than
    // Window::window_title (a macOS-only getter with an empty test default).
    assert_eq!(
        VisualTestContext::from_window(handle.into(), cx)
            .window_title()
            .as_deref(),
        Some("Toolbar")
    );
    drain(&cmd_rx);
    cx.update_window(handle.into(), |_, window, cx| {
        let button = window.find("refresh").bounds();
        let content = window.find("content").bounds();
        assert!(button.origin.y >= px(0.));
        assert!(button.bottom() <= px(56.));
        assert_eq!(content.origin.y, px(56.));
        window.click("refresh", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let commands = drain(&cmd_rx);
    let callback_seq = callback_sequence(&commands);
    assert_eq!(
        callback_pairs(&commands),
        vec![("refresh-old".into(), None)]
    );

    tree.title = Some("Updated toolbar".into());
    tree.children[0].children[0].children[1].on_click = Some("refresh-new".into());
    event_tx
        .send(HostEvent::tree(tree.clone(), callback_seq, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        VisualTestContext::from_window(handle.into(), cx)
            .window_title()
            .as_deref(),
        Some("Updated toolbar")
    );
    drain(&cmd_rx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("refresh", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let commands = drain(&cmd_rx);
    let callback_seq = callback_sequence(&commands);
    assert_eq!(
        callback_pairs(&commands),
        vec![("refresh-new".into(), None)]
    );

    tree.title = None;
    event_tx
        .send(HostEvent::tree(tree, callback_seq, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        VisualTestContext::from_window(handle.into(), cx)
            .window_title()
            .as_deref(),
        Some("clj-gpui")
    );
}

fn episode_list_tree(count: usize, generation: &str, query: &str) -> Node {
    let mut rows = vec![json!({"type": "vstack", "id": "controls", "padding": 16,
        "children": [{"type": "input", "id": "episode-search", "text": query,
                      "on-change": format!("search-{generation}")}]
    })];
    rows.extend((0..count).map(|i| json!({
        "type": "vstack", "id": format!("episode-{i}"), "padding": 18, "gap": 10,
        "children": [
            {"type": "label", "text": format!("Episode {i}: {}", "A longer title that wraps. ".repeat(1 + i % 4)), "font-size": 17},
            {"type": "label", "text": "An episode description with two lines of notes.", "font-size": 13},
            {"type": "button", "id": format!("play-{i}"), "on-click": format!("play-{generation}-{i}"),
             "children": [
                 {"type": "icon", "icon": "play", "size": 14},
                 {"type": "progress", "value": 25, "width": 42, "accessibility-label": "25% played"},
                 {"type": "label", "text": "45m"}
             ]}
        ]
    })));
    serde_json::from_value(json!({"type": "window", "children": [{
        "type": "virtual-scroll", "id": "episodes", "width": 600, "height": 420,
        "row-height": 160,
        "scroll-to-item": 0, "children": rows
    }]}))
    .unwrap()
}

#[gpui_kit::test]
async fn custom_titlebar_keeps_host_and_render_errors_below_window_controls(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (cmd_tx, _cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(640.), px(420.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let normal: Node = serde_json::from_value(json!({
        "type": "window", "chrome": "app", "children": [
            {"type": "title-bar", "height": 56, "traffic-light-position": [10, 19]},
            {"type": "button", "id": "content", "text": "Content"}
        ]
    }))
    .unwrap();
    event_tx
        .send(HostEvent::tree(normal.clone(), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    event_tx
        .send(HostEvent::Error("A callback failed".into()))
        .await
        .unwrap();
    settle_root(handle, cx);
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(window.find("error-titlebar").bounds().size.height, px(56.));
        assert!(window.find("clojure-error").bounds().origin.y >= px(56.));
    })
    .unwrap();

    // Clojure's render exception fallback is a plain vstack, not HostEvent::Error.
    let error: Node = serde_json::from_value(json!({
        "type": "vstack", "padding": 12, "children": [
            {"type": "button", "id": "render-error", "text": "Clojure error"}
        ]
    }))
    .unwrap();
    event_tx
        .send(HostEvent::tree(error, None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(window.find("error-titlebar").bounds().size.height, px(56.));
        assert!(window.find("render-error").bounds().origin.y >= px(56.));
    })
    .unwrap();
    event_tx
        .send(HostEvent::tree(normal, None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(window.find("content").bounds().origin.y, px(56.));
        assert!(window.try_find("clojure-error").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn episode_scroller_virtualizes_rich_controls_and_retains_search(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
    });
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(640.), px(480.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    event_tx
        .send(HostEvent::tree(
            episode_list_tree(2000, "old", ""),
            None,
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    let view = production_view(handle, cx);
    let input_id = view.read_with(cx, |view, _| view.test_input_state_id("episode-search"));
    assert!(input_id.is_some());
    assert!(
        view.read_with(cx, |view, _| view.test_virtual_scroll_counts("episodes").3) > 200_000.,
        "the scrollbar must account for all 2,000 rows immediately"
    );
    let renders = view.read_with(cx, |view, _| view.test_virtual_scroll_counts("episodes").1);
    assert!(
        renders < 200,
        "initial layout rendered {renders} rows out of 2,000"
    );
    drain(&cmd_rx);

    cx.update_window(handle.into(), |_, window, cx| {
        // Compound play controls must keep their icon, progress, and duration.
        assert!(window.find("play-0").bounds().size.width > px(80.));
        window.click("episode-search", cx);
        window.input("Ada", cx);
        assert_eq!(window.find("episode-search").value(), Some("Ada"));
    })
    .unwrap();
    cx.run_until_parked();
    let search = drain(&cmd_rx);
    assert!(callback_pairs(&search).contains(&("search-old".into(), Some(json!("Ada")))));

    let started = std::time::Instant::now();
    for _ in 0..12 {
        cx.update_window(handle.into(), |_, window, cx| {
            let position = point(px(300.), px(250.));
            window.dispatch_event(
                gpui_kit::MouseMoveEvent {
                    position,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            window.dispatch_event(
                gpui_kit::ScrollWheelEvent {
                    position,
                    delta: ScrollDelta::Pixels(point(px(0.), px(-80.))),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
        })
        .unwrap();
        paint_root(handle, cx);
    }
    let rendered =
        view.read_with(cx, |view, _| view.test_virtual_scroll_counts("episodes").1) - renders;
    eprintln!(
        "2,000 episodes, 12 scroll events: {:?}; {rendered} row renders",
        started.elapsed()
    );
    assert!(
        rendered < 1000,
        "scrolling must only realize nearby rows: {rendered}"
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.test_virtual_scroll_counts("episodes").0),
        1
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.test_input_state_id("episode-search")),
        input_id
    );

    cx.update_window(handle.into(), |_, window, _| {
        assert!(
            window.try_find("episode-search").is_none(),
            "the search row must actually scroll out of view"
        );
    })
    .unwrap();

    // A fast gesture must traverse the corresponding distance, not skip all
    // unmeasured rows to the final episodes because their height was zero.
    cx.update_window(handle.into(), |_, window, cx| {
        window.scroll(
            "episodes",
            ScrollDelta::Pixels(point(px(0.), px(-1600.))),
            cx,
        );
    })
    .unwrap();
    paint_root(handle, cx);
    let top = view.read_with(cx, |view, _| view.test_virtual_scroll_counts("episodes").2);
    assert!(
        (8..50).contains(&top),
        "fast scroll landed at row {top} of 2,000"
    );

    // Playback-style snapshots update progress without moving the viewport.
    let mut playback = episode_list_tree(2000, "playing", "Ada");
    playback.children[0].width = Some(420.);
    playback.children[0].children[top].children[2].children[1].value = Some(json!(40));
    event_tx
        .send(HostEvent::tree(playback, None, vec![]))
        .await
        .unwrap();
    paint_root(handle, cx);
    assert_eq!(
        view.read_with(cx, |view, _| view.test_virtual_scroll_counts("episodes").2),
        top
    );
    assert!(
        view.read_with(cx, |view, _| view.test_virtual_scroll_counts("episodes").3) > 200_000.,
        "resizing must not discard offscreen height estimates"
    );

    // A filtered replacement resets the row list and returns to the top.
    event_tx
        .send(HostEvent::tree(
            episode_list_tree(40, "current", "Ada"),
            callback_sequence(&search),
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        view.read_with(cx, |view, _| view.test_input_state_id("episode-search")),
        input_id
    );
    drain(&cmd_rx);
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.find("episode-search").value(), Some("Ada"));
        window.click("play-0", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let commands = drain(&cmd_rx);
    assert_eq!(
        callback_pairs(&commands),
        vec![("play-current-0".into(), None)]
    );
    let sequence = callback_sequence(&commands).expect("scroller buttons use the callback queue");

    // A second click must wait for the first response and resolve its new ID.
    cx.update_window(handle.into(), |_, window, cx| window.click("play-0", cx))
        .unwrap();
    cx.run_until_parked();
    assert!(callback_pairs(&drain(&cmd_rx)).is_empty());
    event_tx
        .send(HostEvent::tree(
            episode_list_tree(40, "fresh", "Ada"),
            Some(sequence),
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        callback_pairs(&drain(&cmd_rx)),
        vec![("play-fresh-0".into(), None)]
    );

    // Removing the scroller must release its retained native search state.
    event_tx
        .send(HostEvent::tree(
            serde_json::from_value(json!({"type": "window"})).unwrap(),
            None,
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert!(
        view.read_with(cx, |view, _| view.test_input_state_id("episode-search"))
            .is_none()
    );
}

#[gpui_kit::test]
async fn transcript_words_wrap_and_click_inside_virtual_scroller(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
    });
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(300.), px(260.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let tree = |callback: &str| {
        serde_json::from_value(json!({
            "type": "window", "children": [{
                "type": "message-scroller", "id": "transcript", "width": 220, "height": 180,
                "scroll-to-item": "passage", "jump-button": false,
                "children": [{"type": "hstack", "id": "passage", "flex-wrap": "wrap", "gap": 6,
                    "children": [
                        {"type": "label", "text": "Trust", "width": 100, "on-click": "first"},
                        {"type": "label", "text": "your", "width": 100, "on-click": "second"},
                        {"type": "label", "text": "partner", "width": 100, "on-click": callback}
                    ]}]
            }]
        }))
        .unwrap()
    };
    for callback in ["seek-old", "seek-current"] {
        event_tx
            .send(HostEvent::tree(tree(callback), None, vec![]))
            .await
            .unwrap();
        settle_root(handle, cx);
        drain(&cmd_rx);
        cx.update_window(handle.into(), |_, window, cx| {
            let first = window.find("root-0.0/0").bounds();
            let last = window.find("root-0.0/2").bounds();
            assert!(
                last.origin.y > first.origin.y,
                "words must wrap at the scroller width"
            );
            window.click("root-0.0/2", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert!(
            callback_pairs(&drain(&cmd_rx))
                .iter()
                .any(|(id, _)| id == callback)
        );
    }
}

fn transcript_follow_tree(passage: usize, word: usize, width: usize, generation: usize) -> Node {
    let children: Vec<_> =
        (0..8)
            .map(|p| {
                let words: Vec<_> = (0..12).map(|w| json!({
                "type": "label", "id": format!("word-{p}-{w}"), "text": format!("Word {w}"),
                "color": if p < passage || (p == passage && w <= word) { "#eeeeee" } else { "#999999" },
                "width": 90, "height": 24, "on-click": format!("seek-{p}-{w}")
            })).collect();
                json!({"type": "hstack", "id": format!("passage-{p}"), "flex-wrap": "wrap",
                   "gap": 6, "padding": 4, "children": words})
            })
            .collect();
    serde_json::from_value(json!({"type": "window", "children": [{
        "type": "message-scroller", "id": "transcript", "width": width, "height": 180,
        "list-style": {"padding": 0}, "jump-button": false,
        "follow-resume-delay": 3, "follow-generation": generation,
        "scroll-to-item": format!("passage-{passage}"),
        "scroll-generation": format!("{passage}-{word}"),
        "follow-child": format!("word-{passage}-{word}"), "children": children
    }]}))
    .unwrap()
}

#[gpui_kit::test]
async fn virtual_scroller_stacks_click_through_text_and_padding_after_refresh(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
    });
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(360.), px(280.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let tree = |scroller: &str, kind: &str, callback: &str, disabled: bool| {
        serde_json::from_value(json!({"type": "window", "children": [{
            "type": scroller, "id": "results", "width": 320, "height": 240,
            "scroll-to-item": "result", "jump-button": false,
            "children": [{"type": kind, "id": "result", "width": 300, "height": 120,
                "padding": 20, "on-click": callback, "disabled": disabled,
                "children": [{"type": "label", "text": "Transcript search result"}]}]
        }]}))
        .unwrap()
    };
    let mut acknowledged = None;
    for scroller in ["virtual-scroll", "message-scroller"] {
        for kind in ["vstack", "hstack"] {
            for (callback, padding, disabled) in [
                ("old", false, false),
                ("current", true, false),
                ("disabled", false, true),
            ] {
                event_tx
                    .send(HostEvent::tree(
                        tree(scroller, kind, callback, disabled),
                        acknowledged.take(),
                        vec![],
                    ))
                    .await
                    .unwrap();
                settle_root(handle, cx);
                drain(&cmd_rx);
                cx.update_window(handle.into(), |_, window, cx| {
                    let bounds = window.find("result").bounds();
                    assert_eq!(bounds.size.width, px(300.));
                    assert_eq!(bounds.size.height, px(120.));
                    // The first point hits the text; the second hits otherwise
                    // empty container padding. Neither needs a nested button.
                    let position = bounds.origin
                        + if padding {
                            point(px(8.), px(100.))
                        } else {
                            point(px(35.), px(30.))
                        };
                    window.dispatch_event(
                        gpui_kit::MouseMoveEvent {
                            position,
                            ..Default::default()
                        }
                        .to_platform_input(),
                        cx,
                    );
                    window.dispatch_event(
                        gpui_kit::MouseDownEvent {
                            position,
                            button: MouseButton::Left,
                            click_count: 1,
                            ..Default::default()
                        }
                        .to_platform_input(),
                        cx,
                    );
                    window.dispatch_event(
                        gpui_kit::MouseUpEvent {
                            position,
                            button: MouseButton::Left,
                            click_count: 1,
                            ..Default::default()
                        }
                        .to_platform_input(),
                        cx,
                    );
                })
                .unwrap();
                cx.run_until_parked();
                let commands = drain(&cmd_rx);
                if disabled {
                    assert!(callback_pairs(&commands).is_empty());
                } else {
                    assert_eq!(callback_pairs(&commands), vec![(callback.into(), None)]);
                    acknowledged = callback_sequence(&commands);
                    assert!(acknowledged.is_some());
                }
            }
        }
    }
}

#[gpui_kit::test]
async fn transcript_follow_keeps_wrapped_line_at_top_after_seeks_and_resize(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
    });
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(320.), px(260.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    // Same line, next wrapped line, far forward, final line, backward, and reflow.
    for (passage, word, width) in [
        (0, 0, 240),
        (0, 1, 240),
        (0, 2, 240),
        (5, 10, 240),
        (7, 11, 240),
        (2, 4, 240),
        (2, 4, 160),
    ] {
        event_tx
            .send(HostEvent::tree(
                transcript_follow_tree(passage, word, width, 0),
                None,
                vec![],
            ))
            .await
            .unwrap();
        settle_root(handle, cx);
        cx.update_window(handle.into(), |_, window, _| {
            let current = window.find(gpui_kit::SharedString::from(format!("root-0.{passage}/{word}"))).bounds();
            assert!(f32::from(current.origin.y).abs() < 1.,
                    "line must start at the viewport top for {passage}/{word} at width {width}: {current:?}");
        }).unwrap();
    }
    // The layout anchor must not intercept clicks on the followed word.
    drain(&cmd_rx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("root-0.2/4", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        callback_pairs(&drain(&cmd_rx))
            .iter()
            .any(|(id, _)| id == "seek-2-4")
    );
}

fn transcript_word_y(handle: WindowHandle<Root>, cx: &mut TestAppContext, word: usize) -> f32 {
    cx.update_window(handle.into(), |_, window, _| {
        f32::from(
            window
                .find(gpui_kit::SharedString::from(format!("root-0.2/{word}")))
                .bounds()
                .top(),
        )
    })
    .unwrap()
}

#[gpui_kit::test]
async fn transcript_word_click_waits_for_the_current_playback_callback_registry(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
    });
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(320.), px(260.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let tree = |callback: &str| {
        let mut tree = transcript_follow_tree(2, 4, 240, 0);
        tree.children[0].follow_child = Some("passage-2".into());
        tree.children[0].children[2].children[4].on_click = Some(callback.into());
        tree
    };
    event_tx
        .send(HostEvent::tree(tree("seek-stale"), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    drain(&cmd_rx);
    // Playback replaces Clojure's registry while a large transcript snapshot
    // is still travelling to the native host. The visible labels are stale.
    event_tx.send(HostEvent::RenderRequested).await.unwrap();
    cx.run_until_parked();
    drain(&cmd_rx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("root-0.2/4", cx)
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        callback_pairs(&drain(&cmd_rx)).is_empty(),
        "must not send a retired callback ID"
    );
    event_tx
        .send(HostEvent::tree(tree("seek-current"), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        callback_pairs(&drain(&cmd_rx)),
        vec![("seek-current".into(), None)]
    );
}

#[gpui_kit::test]
async fn transcript_can_follow_whole_paragraphs_without_moving_between_words(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
    });
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(320.), px(260.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    for (passage, word, width) in [
        (2, 0, 240),
        (2, 6, 240),
        (3, 0, 240),
        (7, 11, 240),
        (2, 6, 160),
    ] {
        let mut tree = transcript_follow_tree(passage, word, width, 0);
        tree.children[0].follow_child = Some(format!("passage-{passage}"));
        tree.children[0].scroll_generation = Some(json!(passage));
        event_tx
            .send(HostEvent::tree(tree, None, vec![]))
            .await
            .unwrap();
        settle_root(handle, cx);
        cx.update_window(handle.into(), |_, window, _| {
            let first = window
                .find(gpui_kit::SharedString::from(format!("root-0.{passage}/0")))
                .bounds();
            assert!(
                (f32::from(first.top()) - 4.).abs() < 1.,
                "paragraph starts at the top, with its 4px padding: {first:?}"
            );
            if word >= 6 {
                let current = window
                    .find(gpui_kit::SharedString::from(format!(
                        "root-0.{passage}/{word}"
                    )))
                    .bounds();
                assert!(
                    current.top() > first.top(),
                    "spoken word must stay on its wrapped line"
                );
            }
        })
        .unwrap();
    }
    scroll_transcript(handle, cx, 60.);
    assert!(transcript_word_y(handle, cx, 0) > 4.);
    cx.executor().advance_clock(Duration::from_millis(3000));
    paint_root(handle, cx);
    assert!((transcript_word_y(handle, cx, 0) - 4.).abs() < 1.);
    // The paragraph's layout anchor leaves individual word seeking intact.
    drain(&cmd_rx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("root-0.2/0", cx)
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        callback_pairs(&drain(&cmd_rx))
            .iter()
            .any(|(id, _)| id == "seek-2-0")
    );
}

fn scroll_transcript(handle: WindowHandle<Root>, cx: &mut TestAppContext, delta: f32) {
    cx.update_window(handle.into(), |_, window, cx| {
        window.scroll(
            "transcript-follow-viewport",
            ScrollDelta::Pixels(point(px(0.), px(delta))),
            cx,
        );
    })
    .unwrap();
    paint_root(handle, cx);
}

#[gpui_kit::test]
async fn long_transcript_scroll_reuses_rows_until_a_new_snapshot(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
    });
    let (cmd_tx, _cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(640.), px(480.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let mut tree = transcript_follow_tree(2, 4, 600, 0);
    let scroller = &mut tree.children[0];
    scroller.height = Some(420.);
    scroller.children = (0..400)
        .map(|p| Node {
            id: Some(format!("passage-{p}")),
            children: (0..50)
                .map(|w| Node {
                    id: Some(format!("word-{p}-{w}")),
                    text: Some("Readalong".into()),
                    on_click: Some(format!("seek-{p}-{w}")),
                    ..scroller.children[0].children[0].clone()
                })
                .collect(),
            ..scroller.children[0].clone()
        })
        .collect();
    let mut updated = tree.clone();
    updated.children[0].children[2].children[4].color = Some("#ffffff".into());
    updated.children[0].children[2].children[4].on_click = Some("seek-current".into());
    event_tx
        .send(HostEvent::tree(tree, None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let view = production_view(handle, cx);
    assert_eq!(
        view.read_with(cx, |view, _| view.test_scroller_sync_count("transcript")),
        1
    );
    let start = std::time::Instant::now();
    for _ in 0..12 {
        scroll_transcript(handle, cx, -20.);
    }
    eprintln!("20,000 words, 12 wheel events: {:?}", start.elapsed());
    assert_eq!(
        view.read_with(cx, |view, _| view.test_scroller_sync_count("transcript")),
        1,
        "wheel frames must not rebuild or compare all rows"
    );

    // Unsequenced playback snapshots still refresh rows and callback IDs.
    let start = std::time::Instant::now();
    event_tx
        .send(HostEvent::tree(updated, None, vec![]))
        .await
        .unwrap();
    paint_root(handle, cx);
    eprintln!("20,000 words, playback snapshot: {:?}", start.elapsed());
    assert_eq!(
        view.read_with(cx, |view, _| view.test_scroller_sync_count("transcript")),
        2
    );
    scroll_transcript(handle, cx, -20.);
    assert_eq!(
        view.read_with(cx, |view, _| view.test_scroller_sync_count("transcript")),
        2
    );
}

#[gpui_kit::test]
async fn transcript_manual_scroll_resumes_three_seconds_after_the_last_event(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
    });
    let (cmd_tx, _cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(320.), px(260.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    event_tx
        .send(HostEvent::tree(
            transcript_follow_tree(2, 4, 240, 0),
            None,
            vec![],
        ))
        .await
        .unwrap();
    paint_root(handle, cx);
    assert!(transcript_word_y(handle, cx, 4).abs() < 1.);

    scroll_transcript(handle, cx, 80.);
    let browsed_y = transcript_word_y(handle, cx, 4);
    assert!(
        browsed_y > 50.,
        "manual scrolling must actually move the transcript: {browsed_y}"
    );

    // Playback still changes words and scroll-generation during the pause.
    event_tx
        .send(HostEvent::tree(
            transcript_follow_tree(2, 6, 240, 0),
            None,
            vec![],
        ))
        .await
        .unwrap();
    paint_root(handle, cx);
    assert!((transcript_word_y(handle, cx, 4) - browsed_y).abs() < 1.);
    cx.executor().advance_clock(Duration::from_millis(2900));
    paint_root(handle, cx);
    assert!((transcript_word_y(handle, cx, 4) - browsed_y).abs() < 1.);

    scroll_transcript(handle, cx, 20.);
    let browsed_again_y = transcript_word_y(handle, cx, 4);
    assert!(browsed_again_y > browsed_y + 10.);
    cx.executor().advance_clock(Duration::from_millis(200));
    paint_root(handle, cx);
    assert!(
        (transcript_word_y(handle, cx, 4) - browsed_again_y).abs() < 1.,
        "the previous deadline must be cancelled"
    );
    cx.executor().advance_clock(Duration::from_millis(2799));
    paint_root(handle, cx);
    assert!((transcript_word_y(handle, cx, 4) - browsed_again_y).abs() < 1.);
    cx.executor().advance_clock(Duration::from_millis(1));
    paint_root(handle, cx);
    assert!(
        transcript_word_y(handle, cx, 6).abs() < 1.,
        "resume at the latest playback word"
    );

    // Resume also works with no playback updates (paused audio or silence).
    scroll_transcript(handle, cx, 60.);
    assert!(transcript_word_y(handle, cx, 6) > 40.);
    cx.executor().advance_clock(Duration::from_secs(3));
    paint_root(handle, cx);
    assert!(transcript_word_y(handle, cx, 6).abs() < 1.);

    // Current position / word clicks bypass the delay via a separate token.
    scroll_transcript(handle, cx, 60.);
    event_tx
        .send(HostEvent::tree(
            transcript_follow_tree(2, 6, 240, 1),
            None,
            vec![],
        ))
        .await
        .unwrap();
    paint_root(handle, cx);
    assert!(transcript_word_y(handle, cx, 6).abs() < 1.);
}

#[gpui_kit::test]
async fn transcript_scrollbar_hold_waits_for_release_before_resuming(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
    });
    let (cmd_tx, _cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(320.), px(260.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    event_tx
        .send(HostEvent::tree(
            transcript_follow_tree(2, 6, 240, 0),
            None,
            vec![],
        ))
        .await
        .unwrap();
    paint_root(handle, cx);
    scroll_transcript(handle, cx, 60.);
    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    // Clicking above the thumb moves toward the start; holding must never
    // snap back, even when the pointer is stationary for more than 3 seconds.
    let track = point(px(238.), px(18.));
    visual.simulate_mouse_move(track, None, Modifiers::default());
    visual.update(|window, cx| window.render_frame(cx));
    visual.simulate_mouse_down(track, MouseButton::Left, Modifiers::default());
    visual.run_until_parked();
    visual.executor().advance_clock(Duration::from_secs(4));
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        let current = window.try_find("root-0.2/6");
        assert!(
            current.is_none_or(|word| f32::from(word.bounds().top()).abs() > 1.),
            "holding the scrollbar must keep following suspended"
        );
    });
    visual.simulate_mouse_up(track, MouseButton::Left, Modifiers::default());
    visual.run_until_parked();
    visual.executor().advance_clock(Duration::from_millis(2999));
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        let current = window.try_find("root-0.2/6");
        assert!(current.is_none_or(|word| f32::from(word.bounds().top()).abs() > 1.));
    });
    visual.executor().advance_clock(Duration::from_millis(1));
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.render_frame(cx);
        assert!(f32::from(window.find("root-0.2/6").bounds().top()).abs() < 1.);
    });
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
async fn search_input_waits_for_current_callbacks_and_preserves_newer_typing(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(520.), px(300.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let tree = |generation: &str, value: &str| {
        let mut tree = fixture_tree(value, "Save", "Search results");
        tree.children[0].on_change = Some(format!("query-{generation}"));
        tree.children[1] = serde_json::from_value(json!({
            "type": "input", "id": "other", "text": ""
        }))
        .unwrap();
        tree
    };
    event_tx
        .send(HostEvent::tree(tree("old", ""), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    drain(&cmd_rx);
    event_tx.send(HostEvent::RenderRequested).await.unwrap();
    settle_root(handle, cx);
    assert!(matches!(cmd_rx.recv().unwrap(), Cmd::Render));
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("name", cx);
        window.input("cou", cx);
        window.input("ntry", cx);
    })
    .unwrap();
    paint_root(handle, cx);
    assert!(
        drain(&cmd_rx).is_empty(),
        "typing must wait for the replacement registry"
    );

    event_tx
        .send(HostEvent::tree(tree("current", ""), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let changed = drain(&cmd_rx);
    assert_eq!(
        callback_pairs(&changed),
        vec![("query-current".into(), Some(json!("country")))]
    );
    assert!(callback_sequence(&changed).is_some());
    cx.update_window(handle.into(), |_, window, cx| {
        window.input(" roads", cx);
        window.click("other", cx);
    })
    .unwrap();
    paint_root(handle, cx);
    assert!(drain(&cmd_rx).is_empty());

    // An asynchronous results render cannot release the callback in flight,
    // or overwrite later typing, even after focus has left the search field.
    event_tx
        .send(HostEvent::tree(tree("results", "country"), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert!(drain(&cmd_rx).is_empty());
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(window.find("name").value(), Some("country roads"));
    })
    .unwrap();
    event_tx
        .send(HostEvent::tree(
            tree("ack", "country"),
            callback_sequence(&changed),
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    let latest = drain(&cmd_rx);
    assert_eq!(
        callback_pairs(&latest),
        vec![("query-ack".into(), Some(json!("country roads")))]
    );
    event_tx
        .send(HostEvent::tree(
            tree("done", "country roads"),
            callback_sequence(&latest),
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert!(
        drain(&cmd_rx).is_empty(),
        "controlled updates must not echo another search"
    );

    event_tx.send(HostEvent::RenderRequested).await.unwrap();
    settle_root(handle, cx);
    drain(&cmd_rx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("name", cx);
        window.input("!", cx);
    })
    .unwrap();
    paint_root(handle, cx);
    assert!(drain(&cmd_rx).is_empty());
    let mut switched = tree("other-podcast", "");
    switched.children[0].id = Some("different-podcast-query".into());
    event_tx
        .send(HostEvent::tree(switched, None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert!(
        drain(&cmd_rx).is_empty(),
        "a removed search must not target the next podcast"
    );
}

#[gpui_kit::test]
async fn input_submit_serializes_with_changes_and_survives_skipped_paints(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(520.), px(300.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let tree = |generation: &str, value: &str| {
        let mut tree = fixture_tree(value, "Save", "Ready");
        tree.children[0].on_change = Some(format!("change-{generation}"));
        tree.children[0].on_submit = Some(format!("submit-{generation}"));
        tree
    };
    event_tx
        .send(HostEvent::tree(tree("old", ""), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    drain(&cmd_rx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("name", cx);
        window.input("Ada", cx);
    })
    .unwrap();
    paint_root(handle, cx);
    let change = drain(&cmd_rx);
    assert_eq!(
        callback_pairs(&change),
        vec![("change-old".into(), Some(json!("Ada")))]
    );
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("enter", cx);
    })
    .unwrap();
    paint_root(handle, cx);
    assert!(drain(&cmd_rx).is_empty());
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(
            window.find("name").value(),
            Some(""),
            "compose fields clear immediately"
        );
    })
    .unwrap();

    event_tx
        .send(HostEvent::tree(
            tree("changed", "Ada"),
            callback_sequence(&change),
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    let submit = drain(&cmd_rx);
    assert_eq!(
        callback_pairs(&submit),
        vec![("submit-changed".into(), Some(json!("Ada")))]
    );
    cx.update_window(handle.into(), |_, window, _| {
        assert_eq!(
            window.find("name").value(),
            Some(""),
            "change acknowledgement cannot undo submit"
        );
    })
    .unwrap();

    // The acknowledgement need not be painted before the next background tree.
    event_tx
        .send(HostEvent::tree(
            tree("submitted", ""),
            callback_sequence(&submit),
            vec![],
        ))
        .await
        .unwrap();
    event_tx
        .send(HostEvent::tree(tree("background", ""), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert!(drain(&cmd_rx).is_empty());
    cx.update_window(handle.into(), |_, window, cx| {
        window.input("Ada", cx);
    })
    .unwrap();
    paint_root(handle, cx);
    assert_eq!(
        callback_pairs(&drain(&cmd_rx)),
        vec![("change-background".into(), Some(json!("Ada")))],
        "typing the same draft again must still emit a change"
    );
}

#[gpui_kit::test]
async fn input_blur_and_escape_wait_for_fresh_callbacks(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    for escape in [false, true] {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = async_channel::unbounded();
        let handle = cx.open_window(size(px(520.), px(300.)), |window, cx| {
            let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
            Root::new(view, window, cx)
        });
        let tree = |generation: &str| {
            let mut tree = fixture_tree("Ada", "Save", "Ready");
            tree.children[0].on_blur = Some(format!("blur-{generation}"));
            tree.children[0].on_escape = Some(format!("escape-{generation}"));
            tree.children[1] = serde_json::from_value(json!({
                "type": "input", "id": "other", "text": ""
            }))
            .unwrap();
            tree
        };
        event_tx
            .send(HostEvent::tree(tree("old"), None, vec![]))
            .await
            .unwrap();
        settle_root(handle, cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.activate_window();
            window.click("name", cx);
        })
        .unwrap();
        drain(&cmd_rx);
        event_tx.send(HostEvent::RenderRequested).await.unwrap();
        settle_root(handle, cx);
        drain(&cmd_rx);
        cx.update_window(handle.into(), |_, window, cx| {
            if escape {
                window.press("escape", cx);
            } else {
                window.click("other", cx);
                assert_eq!(window.find("name").focused(), Some(false));
                assert_eq!(window.find("other").focused(), Some(true));
            }
        })
        .unwrap();
        settle_root(handle, cx);
        assert!(drain(&cmd_rx).is_empty());
        event_tx
            .send(HostEvent::tree(tree("current"), None, vec![]))
            .await
            .unwrap();
        settle_root(handle, cx);
        assert_eq!(
            callback_pairs(&drain(&cmd_rx)),
            if escape {
                vec![("escape-current".into(), None)]
            } else {
                vec![("blur-current".into(), Some(json!("Ada")))]
            }
        );
    }
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
        .send(HostEvent::tree(
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
    })
    .unwrap();

    paint_root(handle, cx);
    let changed = drain(&cmd_rx);
    assert!(changed.iter().any(|cmd| matches!(cmd,
        Cmd::Callback { id, value: Some(value), .. }
        if id == "name-change" && value == &json!("Ada λ🦀")
    )));
    // A click during the change round-trip must wait for its acknowledgement.
    cx.update_window(handle.into(), |_, window, cx| window.click("save", cx))
        .unwrap();
    paint_root(handle, cx);
    assert!(drain(&cmd_rx).is_empty());
    event_tx
        .send(HostEvent::tree(
            fixture_tree("Ada λ🦀", "Save", "Ready"),
            callback_sequence(&changed),
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    let emitted = drain(&cmd_rx);
    let save_seq = emitted.iter().find_map(|cmd| match cmd {
        Cmd::Callback { id, seq, .. } if id == "save-click" => *seq,
        _ => None,
    });
    assert_eq!(save_seq, Some(2));

    event_tx
        .send(HostEvent::tree(
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
        .send(HostEvent::tree(
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
        .send(HostEvent::tree(
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
        .send(HostEvent::tree(select_tree("clj"), None, vec![]))
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
        .send(HostEvent::tree(select_tree("rs"), None, vec![]))
        .await
        .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(1), |window, _| {
        window.find("language").value() == Some("Rust")
    })
    .await;
}

#[gpui_kit::test]
async fn production_searchable_select_reopens_with_all_options(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    for confirm in [false, true] {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = async_channel::unbounded();
        let handle = cx.open_window(size(px(480.), px(320.)), |window, cx| {
            let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
            Root::new(view, window, cx)
        });
        assert!(matches!(cmd_rx.recv().unwrap(), Cmd::Render));
        let tree = |value: &str| {
            serde_json::from_value(json!({
                "type": "window", "chrome": "app", "padding": 16,
                "children": [{
                    "type": "select", "id": "language", "value": value,
                    "searchable": true, "on-change": "language-change",
                    "options": [
                        {"label": "JVM", "items": [{"id": "clj", "label": "Clojure"}]},
                        {"label": "Systems", "items": [
                            {"id": "rs", "label": "Rust"},
                            {"id": "zig", "label": "Zig"}
                        ]}
                    ]
                }]
            }))
            .unwrap()
        };
        event_tx
            .send(HostEvent::tree(tree("clj"), None, vec![]))
            .await
            .unwrap();
        settle_root(handle, cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.within("language").click("input", cx);
            window.input("zig", cx);
        })
        .unwrap();
        settle_root(handle, cx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.press(if confirm { "enter" } else { "escape" }, cx);
        })
        .unwrap();
        settle_root(handle, cx);
        let commands = drain(&cmd_rx);
        if confirm {
            assert_eq!(
                callback_pairs(&commands),
                vec![("language-change".into(), Some(json!("zig")))]
            );
        } else {
            assert!(
                callback_pairs(&commands).is_empty(),
                "cancel must not change Clojure's value"
            );
        }
        // Acknowledge the callback with the real controlled-tree path. The
        // retained Select must keep the committed ID after clearing its filter.
        event_tx
            .send(HostEvent::tree(
                tree(if confirm { "zig" } else { "clj" }),
                callback_sequence(&commands),
                vec![],
            ))
            .await
            .unwrap();
        settle_root(handle, cx);
        cx.update_window(handle.into(), |_, window, cx| {
            assert_eq!(window.find("language").expanded(), Some(false));
            assert_eq!(
                window.find("language").value(),
                Some(if confirm { "Zig" } else { "Clojure" })
            );
            window.within("language").click("input", cx);
            window.press(if confirm { "up" } else { "down" }, cx);
            window.press("enter", cx);
        })
        .unwrap();
        settle_root(handle, cx);
        assert_eq!(
            callback_pairs(&drain(&cmd_rx)),
            vec![("language-change".into(), Some(json!("rs")))]
        );
    }
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
    let slider_tree = |value: f32, extra: bool, on_change: &str, on_release: &str| {
        let mut children = vec![json!({
            "type": "slider", "id": "volume", "value": value,
            "min": 0, "max": 100, "step": 1, "height": 28,
            "on-change": on_change, "on-release": on_release
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
        .send(HostEvent::tree(
            slider_tree(20., false, "volume-change", "volume-release"),
            None,
            vec![],
        ))
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
        .send(HostEvent::tree(
            slider_tree(65., true, "volume-change", "volume-release"),
            None,
            vec![],
        ))
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

    event_tx.send(HostEvent::RenderRequested).await.unwrap();
    settle_root(handle, cx);
    assert!(matches!(cmd_rx.recv().unwrap(), Cmd::Render));

    cx.update_window(handle.into(), |_, window, cx| {
        first_state.update(cx, |state, cx| {
            let bounds = state.bounds();
            state.update_value_by_position(
                Axis::Horizontal,
                point(
                    bounds.left() + bounds.size.width * 0.8,
                    bounds.top() + bounds.size.height / 2.,
                ),
                false,
                window,
                cx,
            );
            state.handle_release(cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().timer(Duration::from_millis(20)).await;
    assert_eq!(
        view.read_with(cx, |view, cx| view.test_slider_value("volume", cx)),
        Some(SliderValue::Single(80.)),
        "the native click value is visible before its callback can run"
    );
    assert!(
        drain(&cmd_rx).is_empty(),
        "the gesture must wait rather than send ids from the registry being replaced"
    );

    event_tx
        .send(HostEvent::tree(
            slider_tree(65., true, "volume-change-fresh", "volume-release-fresh"),
            None,
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        view.read_with(cx, |view, cx| view.test_slider_value("volume", cx)),
        Some(SliderValue::Single(80.)),
        "an unrelated old controlled value must not undo the click"
    );
    let emitted = drain(&cmd_rx);
    let slider_seq = emitted.iter().find_map(|cmd| match cmd {
        Cmd::Callback { seq, .. } | Cmd::CallbackBatch { seq, .. } => *seq,
        _ => None,
    });
    assert!(slider_seq.is_some());
    let emitted_pairs = callback_pairs(&emitted);
    assert!(
        emitted_pairs
            .iter()
            .any(|(id, value)| id == "volume-change-fresh" && value.is_some())
    );
    assert!(
        emitted_pairs
            .iter()
            .any(|(id, value)| id == "volume-release-fresh" && value.is_some())
    );

    event_tx
        .send(HostEvent::tree(
            slider_tree(80., true, "volume-change-ack", "volume-release-ack"),
            slider_seq,
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        view.read_with(cx, |view, cx| view.test_slider_value("volume", cx)),
        Some(SliderValue::Single(80.))
    );

    let without_slider: Node = serde_json::from_value(json!({
        "type": "window", "chrome": "app", "children": [
            {"type": "label", "id": "absent-marker", "text": "Temporarily absent"}
        ]
    }))
    .unwrap();
    event_tx
        .send(HostEvent::tree(without_slider, None, vec![]))
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
        .send(HostEvent::tree(
            slider_tree(35., false, "volume-change", "volume-release"),
            None,
            vec![],
        ))
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
async fn production_markdown_selection_autoscroll_stops_on_release(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let source = (0..100)
        .map(|index| format!("Paragraph {index} with enough text to select"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let tree: Node = serde_json::from_value(json!({
        "type": "window", "chrome": "app", "padding": 16,
        "children": [{
            "type": "markdown", "id": "reader", "height": 220,
            "selectable": true, "text": source
        }]
    }))
    .unwrap();

    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(520.), px(340.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    assert!(matches!(cmd_rx.recv().unwrap(), Cmd::Render));
    event_tx
        .send(HostEvent::tree(tree, None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);

    let mut visual = VisualTestContext::from_window(handle.into(), cx);
    visual.update(|window, cx| window.render_frame(cx));
    let bounds = visual
        .debug_bounds("production-markdown-viewport")
        .expect("production Markdown viewport bounds");
    let start = point(bounds.left() + px(30.), bounds.top() + px(30.));
    let edge = point(bounds.left() + px(60.), bounds.bottom() - px(2.));

    visual.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    visual.simulate_mouse_move(edge, Some(MouseButton::Left), Modifiers::default());
    visual.update(|window, cx| window.render_frame(cx));
    let before = visual.update(TextSelection::selected_text);
    assert!(!before.is_empty(), "the drag must start a text selection");

    for _ in 0..12 {
        visual.executor().advance_clock(Duration::from_millis(16));
        visual.run_until_parked();
        visual.update(|window, cx| window.render_frame(cx));
    }
    let held = visual.update(TextSelection::selected_text);
    assert!(
        held.len() > before.len(),
        "holding at the viewport edge must extend selection: before={before:?}, held={held:?}"
    );

    visual.simulate_mouse_up(edge, MouseButton::Left, Modifiers::default());
    visual.executor().advance_clock(Duration::from_millis(64));
    visual.run_until_parked();
    visual.update(|window, cx| window.render_frame(cx));
    assert_eq!(
        visual.update(TextSelection::selected_text),
        held,
        "mouse release must stop Markdown selection autoscroll"
    );
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
        .send(HostEvent::tree(
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
        .send(HostEvent::tree(editor_tree("rust", original), None, vec![]))
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
        .send(HostEvent::tree(collection_tree(false), None, vec![]))
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
        .send(HostEvent::tree(collection_tree(true), None, vec![]))
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
        .send(HostEvent::tree(tree, None, vec![]))
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
        .send(HostEvent::tree(tree, None, vec![]))
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

fn input_dialog_tree(kind: &str, open: bool, generation: &str, value: &str, focus: bool) -> Node {
    serde_json::from_value(json!({
        "type": "window", "chrome": "app", "children": [
            {"type": "input", "id": "url", "text": "outside"},
            {"type": kind, "id": "add", "open": open, "title": "Add podcast",
             "on-close": format!("close-{generation}"), "children": [
                {"type": "vstack", "children": [
                    {"type": "input", "id": "url", "text": value, "focus": focus,
                     "placeholder": "Podcast URL", "accessibility-label": "Podcast URL",
                     "on-change": format!("change-{generation}"),
                     "on-submit": format!("submit-{generation}"),
                     "on-blur": format!("blur-{generation}")}
                ]}
             ]}
        ]
    }))
    .unwrap()
}

#[gpui_kit::test]
async fn production_dialog_inputs_edit_submit_refresh_and_restore_focus(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    for kind in ["dialog", "alert-dialog"] {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = async_channel::unbounded();
        let handle = cx.open_window(size(px(640.), px(420.)), |window, cx| {
            let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
            Root::new(view, window, cx)
        });
        let path = "add/content/0/0";
        let key = "dialog-input/3:add/id/3:url";
        event_tx
            .send(HostEvent::tree(
                input_dialog_tree(kind, false, "1", "", true),
                None,
                vec![],
            ))
            .await
            .unwrap();
        settle_root(handle, cx);
        let view = production_view(handle, cx);
        assert!(
            view.read_with(cx, |view, _| view.test_input_state_id(key))
                .is_none()
        );
        cx.update_window(handle.into(), |_, window, cx| window.click("url", cx))
            .unwrap();

        event_tx
            .send(HostEvent::tree(
                input_dialog_tree(kind, true, "1", "", true),
                None,
                vec![],
            ))
            .await
            .unwrap();
        settle_root(handle, cx);
        let original = view
            .read_with(cx, |view, _| view.test_input_state_id(key))
            .unwrap();
        assert_ne!(
            Some(original),
            view.read_with(cx, |view, _| view.test_input_state_id("url"))
        );
        drain(&cmd_rx);
        cx.update_window(handle.into(), |_, window, cx| {
            assert_eq!(window.find(path).focused(), Some(true));
            window.input("https://example.com/feed", cx);
            assert_eq!(window.find(path).value(), Some("https://example.com/feed"));
            assert_eq!(window.find("url").value(), Some("outside"));
        })
        .unwrap();
        paint_root(handle, cx);
        let changed = drain(&cmd_rx);
        assert!(
            callback_pairs(&changed)
                .contains(&("change-1".into(), Some(json!("https://example.com/feed"))))
        );

        // Enter arrives while on-change is replacing Clojure's registry.
        // It must wait and resolve the new submit callback from the response.
        cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx))
            .unwrap();
        cx.run_until_parked();
        assert!(callback_pairs(&drain(&cmd_rx)).is_empty());

        event_tx
            .send(HostEvent::tree(
                input_dialog_tree(kind, true, "2", "https://example.com/feed", false),
                callback_sequence(&changed),
                vec![],
            ))
            .await
            .unwrap();
        settle_root(handle, cx);
        assert_eq!(
            Some(original),
            view.read_with(cx, |view, _| view.test_input_state_id(key))
        );
        let submitted = drain(&cmd_rx);
        assert!(
            callback_pairs(&submitted)
                .contains(&("submit-2".into(), Some(json!("https://example.com/feed")))),
            "kind={kind}, changed={:?}, submitted={:?}",
            callback_pairs(&changed),
            callback_pairs(&submitted)
        );
        // App-driven close after successful submission restores the original
        // input and prunes the dialog slot, even with the same user-facing id.
        event_tx
            .send(HostEvent::tree(
                input_dialog_tree(kind, false, "3", "", false),
                callback_sequence(&submitted),
                vec![],
            ))
            .await
            .unwrap();
        settle_root(handle, cx);
        assert!(
            view.read_with(cx, |view, _| view.test_input_state_id(key))
                .is_none()
        );
        cx.update_window(handle.into(), |_, window, _| {
            assert_eq!(window.find("url").focused(), Some(true));
            assert!(window.try_find(path).is_none());
        })
        .unwrap();

        event_tx
            .send(HostEvent::tree(
                input_dialog_tree(kind, true, "4", "", true),
                None,
                vec![],
            ))
            .await
            .unwrap();
        settle_root(handle, cx);
        assert_ne!(
            Some(original),
            view.read_with(cx, |view, _| view.test_input_state_id(key))
        );
        drain(&cmd_rx);
        cx.update_window(handle.into(), |_, window, cx| {
            assert_eq!(window.find(path).value(), Some(""));
            window.press("escape", cx);
        })
        .unwrap();
        settle_root(handle, cx);
        let closed = drain(&cmd_rx);
        assert!(callback_pairs(&closed).contains(&("close-4".into(), None)));
        cx.update_window(handle.into(), |_, window, _| {
            assert_eq!(window.find("url").focused(), Some(true));
            assert!(window.try_find(path).is_none());
        })
        .unwrap();
    }
}

#[gpui_kit::test]
async fn dialog_job_cancel_keeps_its_identity_when_an_earlier_job_finishes(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(600.), px(500.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let tree = |jobs: &[&str], generation: &str| {
        let children: Vec<_> = jobs
            .iter()
            .map(|id| {
                json!({
                    "type": "vstack", "gap": 8, "children": [
                        {"type": "label", "text": id},
                        {"type": "progress", "id": format!("progress-{id}"), "value": 25},
                        {"type": "button", "id": format!("cancel-{id}"), "text": "Cancel",
                         "on-click": format!("cancel-{id}-{generation}")}
                    ]
                })
            })
            .collect();
        serde_json::from_value(json!({"type": "window", "children": [{
            "type": "dialog", "id": "jobs", "open": true, "title": "Transcription progress",
            "width": 500, "children": children
        }]}))
        .unwrap()
    };
    event_tx
        .send(HostEvent::tree(tree(&["a", "b", "c"], "old"), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    drain(&cmd_rx);
    event_tx.send(HostEvent::RenderRequested).await.unwrap();
    cx.run_until_parked();
    drain(&cmd_rx);
    cx.update_window(handle.into(), |_, window, cx| window.click("cancel-b", cx))
        .unwrap();
    cx.run_until_parked();
    assert!(callback_pairs(&drain(&cmd_rx)).is_empty());
    // A completes while the click waits for the updated callback registry.
    // B moved from row 1 to row 0; row 1 now belongs to C.
    event_tx
        .send(HostEvent::tree(tree(&["b", "c"], "current"), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        callback_pairs(&drain(&cmd_rx)),
        vec![("cancel-b-current".into(), None)]
    );
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
        .send(HostEvent::tree(dialog_tree(false, "1"), None, vec![]))
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
        .send(HostEvent::tree(dialog_tree(true, "1"), None, vec![]))
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
        .send(HostEvent::tree(dialog_tree(true, "2"), None, vec![]))
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
        .send(HostEvent::tree(
            dialog_tree(false, "2"),
            ok_sequence,
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    event_tx
        .send(HostEvent::tree(dialog_tree(true, "3"), None, vec![]))
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
        .send(HostEvent::tree(
            dialog_tree(false, "3"),
            cancel_sequence,
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    event_tx
        .send(HostEvent::tree(dialog_tree(true, "4"), None, vec![]))
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
        .send(HostEvent::tree(tree, None, vec![]))
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
        .send(HostEvent::tree(tree, None, vec![]))
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

#[gpui_kit::test]
async fn nested_surfaces_mount_and_retain_production_widgets(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
    });
    // These containers previously erased stateful children in a static painter.
    for surface in [
        "dialog",
        "sheet",
        "dock",
        "nav-stack",
        "data-table",
        "virtual-scroll",
        "message-scroller",
    ] {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = async_channel::unbounded();
        let handle = cx.open_window(size(px(900.), px(800.)), |window, cx| {
            let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
            Root::new(view, window, cx)
        });
        let children = json!({"type": "vstack", "id": "nested-body", "width": 380, "height": 260, "children": [
            {"type": "editor", "id": "nested-editor", "text": "hello", "height": 70},
            {"type": "list", "id": "nested-list", "items": [{"id": "one", "label": "One"}], "height": 60},
            {"type": "button", "id": "nested-action", "text": "Action", "on-click": "old"}
        ]});
        let container = match surface {
            "dialog" | "sheet" => {
                json!({"type": surface, "id": "surface", "open": true, "width": 500, "children": [children]})
            }
            "dock" => {
                json!({"type": "dock", "id": "surface", "height": 500, "items": [{"id": "panel", "label": "Panel", "content": children}]})
            }
            "nav-stack" => {
                json!({"type": "nav-stack", "id": "surface", "height": 500, "children": [{"type": "nav-page", "id": "page", "children": [children]}]})
            }
            "data-table" => {
                json!({"type": "data-table", "id": "surface", "height": 500, "row-height": 300, "options": [{"id": "body", "label": "Body", "width": 440}], "items": [{"id": "row", "cells": [children]}]})
            }
            _ => {
                json!({"type": surface, "id": "surface", "height": 500, "row-height": 280, "children": [children]})
            }
        };
        let tree: Node = serde_json::from_value(
            json!({"type": "window", "chrome": "app", "children": [container]}),
        )
        .unwrap();
        event_tx
            .send(HostEvent::tree(tree.clone(), None, vec![]))
            .await
            .unwrap();
        settle_root(handle, cx);
        let view = production_view(handle, cx);
        let before = view.read_with(cx, |view, _| {
            view.test_editor_state("nested-editor")
                .map(|state| state.entity_id())
        });
        assert!(
            before.is_some(),
            "{surface} must use the production editor renderer"
        );
        assert!(
            view.read_with(cx, |view, cx| view
                .test_list_state("nested-list", cx)
                .is_some()),
            "{surface} must mount a real list"
        );
        event_tx
            .send(HostEvent::tree(tree, None, vec![]))
            .await
            .unwrap();
        paint_root(handle, cx);
        assert_eq!(
            before,
            view.read_with(cx, |view, _| view
                .test_editor_state("nested-editor")
                .map(|state| state.entity_id())),
            "{surface} must retain the editor across trees"
        );
        drain(&cmd_rx);
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("nested-action", cx)
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(
            callback_pairs(&drain(&cmd_rx)),
            vec![("old".into(), None)],
            "{surface} child must remain interactive"
        );
        cx.update_window(handle.into(), |_, window, _| window.remove_window())
            .unwrap();
    }
}

#[gpui_kit::test]
async fn new_kit_families_mount_real_controls_and_keep_live_slot_callbacks(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
    });
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(1000.), px(900.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let tree = |callback: &str| {
        serde_json::from_value(json!({"type":"window", "chrome":"app", "children":[
        {"type":"input-group", "id":"group", "children":[
            {"type":"input", "id":"group-input", "text":"search"},
            {"type":"input-group-addon", "align":"inline-end", "children":[
                {"type":"input-group-button", "id":"group-action", "text":"Go", "on-click":callback}
            ]}
        ]},
        {"type":"form", "columns":2, "children":[
            {"type":"field", "title":"Name", "required":true, "children":[{"type":"input","id":"form-input","text":"Jane"}]},
            {"type":"field", "title":"Notes", "children":[{"type":"editor","id":"form-editor","text":"hello","height":50}]}
        ]},
        {"type":"empty", "children":[
            {"type":"empty-header", "children":[{"type":"empty-title","children":[{"type":"label","text":"Nothing yet"}]}]},
            {"type":"empty-content", "children":[{"type":"button","id":"empty-action","text":"Create","on-click":callback}]}
        ]},
        {"type":"collapsible", "open":true, "content":{"type":"input","text":"Anonymous","on-change":callback}},
        {"type":"list", "id":"custom-list", "height":75, "items":[{"id":"one","label":"One","content":{"type":"button","id":"row-action","text":"Row action","on-click":callback}}]},
        {"type":"select", "id":"custom-select", "value":"one", "options":[{"id":"one","label":"One", "display-content":{"type":"button","id":"display-action","text":"Display action","on-click":callback}}]},
        {"type":"carousel", "id":"slides", "height":100, "value":0, "children":[
            {"type":"carousel-content", "children":[{"type":"carousel-item", "children":[{"type":"input", "id":"slide-input", "text":"slide"}]}]},
            {"type":"carousel-previous"}, {"type":"carousel-next"},
            {"type":"carousel-pagination", "children":[{"type":"carousel-pagination-item","value":0}]}
        ]}
    ]})).unwrap()
    };
    event_tx
        .send(HostEvent::tree(tree("old"), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let view = production_view(handle, cx);
    let anonymous_before = view.read_with(cx, |view, _| {
        for key in ["group-input", "form-input", "slide-input", "root-3-content"] {
            assert!(
                view.test_input_state_id(key).is_some(),
                "{key} should be a native input"
            );
        }
        assert!(view.test_editor_state("form-editor").is_some());
        view.test_input_state_id("root-3-content")
    });
    event_tx
        .send(HostEvent::tree(tree("new"), None, vec![]))
        .await
        .unwrap();
    paint_root(handle, cx);
    assert_eq!(
        anonymous_before,
        view.read_with(cx, |view, _| view.test_input_state_id("root-3-content"))
    );
    for action in [
        "group-action",
        "empty-action",
        "row-action",
        "display-action",
    ] {
        drain(&cmd_rx);
        cx.update_window(handle.into(), |_, window, cx| window.click(action, cx))
            .unwrap();
        cx.run_until_parked();
        let commands = drain(&cmd_rx);
        let calls = callback_pairs(&commands);
        assert!(
            calls.contains(&("new".into(), None)),
            "{action} must use the latest callback, got {calls:?}"
        );
        event_tx
            .send(HostEvent::tree(
                tree("new"),
                callback_sequence(&commands),
                vec![],
            ))
            .await
            .unwrap();
        paint_root(handle, cx);
    }
}

#[gpui_kit::test]
async fn calendar_controlled_changes_do_not_echo_and_date_disabling_updates(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(700.), px(600.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    for value in ["2026-09-01", "2026-09-02"] {
        let tree = serde_json::from_value(json!({"type":"window", "children":[{"type":"calendar", "id":"calendar", "value":value,"on-change":"selected", "year-range":[2025,2027],"disabled-dates":["2026-09-10"]}]})).unwrap();
        event_tx
            .send(HostEvent::tree(tree, None, vec![]))
            .await
            .unwrap();
        settle_root(handle, cx);
        assert!(
            callback_pairs(&drain(&cmd_rx)).is_empty(),
            "controlled dates must not echo"
        );
    }
}

#[gpui_kit::test]
async fn editor_search_requests_preserve_identity_and_apply_actions_once(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(700.), px(500.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let tree = |search: serde_json::Value| {
        serde_json::from_value(json!({"type":"window", "children":[{"type":"editor", "id":"search-editor", "text":"one TWO one", "height":300, "on-search":"search-state", "search": search}]})).unwrap()
    };
    event_tx
        .send(HostEvent::tree(tree(json!({"query":"one"})), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let view = production_view(handle, cx);
    let editor = view.read_with(cx, |view, _| {
        view.test_editor_state("search-editor").unwrap()
    });
    assert_eq!(
        editor.read_with(cx, |editor, _| editor.search_session().matcher.len()),
        2
    );
    let initial = callback_pairs(&drain(&cmd_rx));
    assert!(initial.iter().any(|(id, value)| {
        id == "search-state"
            && value
                .as_ref()
                .is_some_and(|v| v["count"] == 2 && v["query"] == "one")
    }));
    let request = json!({"query":"one", "action":"next", "generation":1});
    event_tx
        .send(HostEvent::tree(tree(request.clone()), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let index = editor.read_with(cx, |editor, _| editor.search_session().matcher.current());
    event_tx
        .send(HostEvent::tree(tree(request), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        editor.read_with(cx, |editor, _| editor.search_session().matcher.current()),
        index
    );
    assert_eq!(
        view.read_with(cx, |view, _| view
            .test_editor_state("search-editor")
            .unwrap()
            .entity_id()),
        editor.entity_id()
    );
    event_tx
        .send(HostEvent::tree(
            tree(json!({"query":"two", "case-insensitive":false})),
            None,
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        editor.read_with(cx, |editor, _| editor.search_session().matcher.len()),
        0
    );
    event_tx
        .send(HostEvent::tree(
            tree(json!({"query":"two", "case-insensitive":true})),
            None,
            vec![],
        ))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        editor.read_with(cx, |editor, _| editor.search_session().matcher.len()),
        1
    );
    handle
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
}

#[gpui_kit::test]
async fn editor_provider_configuration_retains_state_and_refreshes_live(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (cmd_tx, _cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(700.), px(500.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let tree = |lsp: serde_json::Value| {
        serde_json::from_value(json!({"type":"window", "children":[{"type":"editor", "id":"provider-editor", "text":"hello", "height":250,"lsp":lsp}]})).unwrap()
    };
    let config =
        json!({"completions":"stable-provider","hover":"stable-hover","completion-menu-width":480});
    event_tx
        .send(HostEvent::tree(tree(config.clone()), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let view = production_view(handle, cx);
    let editor = view.read_with(cx, |view, _| {
        view.test_editor_state("provider-editor").unwrap()
    });
    let provider = editor.read_with(cx, |editor, _| {
        assert!(editor.lsp().hover_provider.is_some());
        assert_eq!(editor.lsp().completion_menu.max_width, px(480.));
        editor.lsp().completion_provider.clone().unwrap()
    });
    event_tx
        .send(HostEvent::tree(tree(config), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    editor.read_with(cx, |editor, _| {
        assert!(std::rc::Rc::ptr_eq(
            &provider,
            editor.lsp().completion_provider.as_ref().unwrap()
        ))
    });
    event_tx
        .send(HostEvent::tree(tree(serde_json::Value::Null), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        view.read_with(cx, |view, _| view
            .test_editor_state("provider-editor")
            .unwrap()
            .entity_id()),
        editor.entity_id()
    );
    editor.read_with(cx, |editor, _| {
        assert!(editor.lsp().completion_provider.is_none())
    });
    handle
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
}

#[gpui_kit::test]
async fn dock_layout_round_trip_keeps_native_panels_and_rich_content(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(850.), px(600.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let tree = |layout: serde_json::Value| {
        serde_json::from_value(json!({"type":"window", "children":[{"type":"dock", "id":"layout", "height":450,"on-layout-change":"layout-changed","dock-layout":layout,"items":[
        {"id":"source","label":"Source","content":{"type":"editor","id":"docked-editor","text":"retained","height":300}},
        {"id":"help","label":"Help","content":{"type":"label","text":"Help"}}
    ]}]})).unwrap()
    };
    event_tx
        .send(HostEvent::tree(tree(serde_json::Value::Null), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let view = production_view(handle, cx);
    let area = view.read_with(cx, |view, _| view.test_dock_state("layout").unwrap());
    let editor = view.read_with(cx, |view, _| {
        view.test_editor_state("docked-editor").unwrap()
    });
    cx.update(|cx| {
        area.update(cx, |_, cx| {
            cx.emit(gpui_kit::base::dock::DockEvent::LayoutChanged)
        })
    });
    settle_root(handle, cx);
    let commands = drain(&cmd_rx);
    let calls = callback_pairs(&commands);
    let layout = calls
        .iter()
        .find(|(id, _)| id == "layout-changed")
        .unwrap()
        .1
        .clone()
        .unwrap();
    let seq = callback_sequence(&commands);
    assert!(layout.to_string().contains("clj-gpui-panel"));
    event_tx
        .send(HostEvent::tree(tree(layout), seq, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        area.entity_id(),
        view.read_with(cx, |view, _| view
            .test_dock_state("layout")
            .unwrap()
            .entity_id())
    );
    assert_eq!(
        editor.entity_id(),
        view.read_with(cx, |view, _| view
            .test_editor_state("docked-editor")
            .unwrap()
            .entity_id())
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "echoing a native layout must not load and emit it again"
    );
    handle
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
}

#[gpui_kit::test]
async fn markdown_plugins_and_list_initial_slots_mount_retained_controls(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (cmd_tx, _cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(900.), px(700.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let tree = |text: &str| {
        serde_json::from_value(json!({"type":"window","children":[
        {"type":"app-menu-bar","id":"application-menu","items":[{"id":"file","label":"File","items":[{"id":"new","label":"New","on-click":"new"}]}]},
        {"type":"list","id":"initial-list","searchable":true,"height":150,
         "initial-content":{"type":"input","id":"initial-control","text":text}},
        {"type":"markdown","id":"extended-markdown","height":400,
         "text":"```widget\ncustom\n```\n\nUse [chip](action) here.",
         "items":[
             {"id":"block","source":"```widget\ncustom\n```","content":{"type":"editor","id":"markdown-editor","text":text,"height":150}},
             {"id":"inline","variant":"inline","source":"[chip](action)","content":{"type":"input","id":"markdown-inline","text":text,"width":100,"height":25}}
         ]}
    ]})).unwrap()
    };
    event_tx
        .send(HostEvent::tree(tree("first"), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let view = production_view(handle, cx);
    let editor = view.read_with(cx, |view, _| {
        view.test_editor_state("markdown-editor")
            .expect("custom Markdown block")
    });
    for id in ["initial-control", "markdown-inline"] {
        assert!(
            view.read_with(cx, |view, _| view.test_input_state_id(id).is_some()),
            "missing {id}"
        );
    }
    assert!(
        cx.update(|cx| gpui_kit::component::GlobalState::global(cx)
            .app_menus()
            .is_empty()),
        "menu snapshots must not replace global menus"
    );
    event_tx
        .send(HostEvent::tree(tree("second"), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        editor.entity_id(),
        view.read_with(cx, |view, _| view
            .test_editor_state("markdown-editor")
            .unwrap()
            .entity_id())
    );
    assert_eq!(
        editor.read_with(cx, |state, _| state.value().to_string()),
        "second"
    );
    handle
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
}

#[gpui_kit::test]
async fn virtualized_editors_retain_state_until_removed_from_the_tree(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(800.), px(600.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let tree = |target: &str, include_first: bool| {
        let children: Vec<_> = (0..80).map(|i| {
            if i == 0 && include_first {
                json!({"type":"vstack","id":"row-0","height":200,"children":[{"type":"editor","id":"virtual-editor","text":"keep me","height":150,"on-blur":format!("blur-{target}")}]})
            } else {
                json!({"type":"label","id":format!("row-{i}"),"text":format!("Row {i}"),"height":200})
            }
        }).collect();
        serde_json::from_value(json!({"type":"window","children":[{"type":"virtual-scroll","id":"virtual-editors","height":400,"row-height":200,"scroll-to-item":target,"children":children}]})).unwrap()
    };
    event_tx
        .send(HostEvent::tree(tree("row-0", true), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let view = production_view(handle, cx);
    let editor = view.read_with(cx, |view, _| {
        view.test_editor_state("virtual-editor").unwrap()
    });
    event_tx
        .send(HostEvent::tree(tree("row-70", true), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert!(
        view.read_with(cx, |view, _| view
            .test_editor_state("virtual-editor")
            .is_some()),
        "offscreen is not removed"
    );
    drain(&cmd_rx);
    editor.update(cx, |_, cx| {
        cx.emit(gpui_kit::component::input::InputEvent::Blur);
    });
    cx.run_until_parked();
    assert!(
        callback_pairs(&drain(&cmd_rx)).contains(&("blur-row-70".into(), Some(json!("keep me")))),
        "retained offscreen controls must use the latest callback registry"
    );
    event_tx
        .send(HostEvent::tree(tree("row-0", true), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        editor.entity_id(),
        view.read_with(cx, |view, _| view
            .test_editor_state("virtual-editor")
            .unwrap()
            .entity_id())
    );
    event_tx
        .send(HostEvent::tree(tree("row-0", false), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert!(view.read_with(cx, |view, _| {
        view.test_editor_state("virtual-editor").is_none()
    }));
    handle
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
}

#[gpui_kit::test]
async fn editor_options_and_annotations_update_without_replacing_native_state(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        syntax::init(cx);
    });
    let (cmd_tx, _cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(800.), px(600.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let mut tree: Node = serde_json::from_value(json!({"type":"window","children":[{
        "type":"editor","id":"options-editor","text":"héllo", "height":200,
        "placeholder":"Write code", "focus":true,
        "soft-wrap":false,"folding":false,"line-number":false,"indent-guides":false,
        "show-whitespaces":true,"tab-size":4,"hard-tabs":true,"wrapping-indent":"none",
        "scroll-beyond-last-line":0,"cursor-surrounding-lines":1,"selected-range":[2,4],
        "diagnostics":[{"range":{"start":{"line":0,"character":1},"end":{"line":0,"character":3}},"message":"Example","severity":2}],
        "decorations":[{"range":[2,4],"background":"#ff0000","underline":"wavy"}]
    }]})).unwrap();
    event_tx
        .send(HostEvent::tree(tree.clone(), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let view = production_view(handle, cx);
    let editor = view.read_with(cx, |view, _| {
        view.test_editor_state("options-editor").unwrap()
    });
    editor.read_with(cx, |state, _| {
        assert_eq!(
            state.selected_range(),
            1..4,
            "selection clips UTF-8 boundaries"
        );
        assert_eq!(state.diagnostics().unwrap().len(), 1);
        assert_eq!(state.presentation().placeholder().as_ref(), "Write code");
    });
    assert_eq!(
        view.read_with(cx, |view, cx| view
            .test_decoration_ranges("options-editor", cx)),
        vec![1..4]
    );
    editor.update(cx, |state, cx| state.set_selected_range(5..6, cx));
    event_tx
        .send(HostEvent::tree(tree.clone(), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        editor.read_with(cx, |state, _| state.selected_range()),
        5..6,
        "an unchanged selection request must not undo native cursor movement"
    );
    tree.children[0].placeholder = None;
    tree.children[0].selection_generation = Some(json!(1));
    tree.children[0].diagnostics.clear();
    tree.children[0].decorations.clear();
    tree.children[0].soft_wrap = None;
    tree.children[0].folding = None;
    tree.children[0].line_number = None;
    tree.children[0].indent_guides = None;
    event_tx
        .send(HostEvent::tree(tree, None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    editor.read_with(cx, |state, _| {
        assert_eq!(state.selected_range(), 1..4);
        assert!(state.diagnostics().unwrap().is_empty());
        assert!(state.presentation().placeholder().is_empty());
    });
    assert!(
        view.read_with(cx, |view, cx| view
            .test_decoration_ranges("options-editor", cx))
            .is_empty()
    );
    assert_eq!(
        editor.entity_id(),
        view.read_with(cx, |view, _| view
            .test_editor_state("options-editor")
            .unwrap()
            .entity_id())
    );
    handle
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
}

#[gpui_kit::test]
async fn textarea_native_growth_and_search_survive_tree_updates(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (cmd_tx, _cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = async_channel::unbounded();
    let handle = cx.open_window(size(px(700.), px(600.)), |window, cx| {
        let view = cx.new(|cx| RootView::new(7331, cmd_tx, event_rx, window, cx));
        Root::new(view, window, cx)
    });
    let mut tree: Node = serde_json::from_value(json!({"type":"window","children":[{
        "type":"textarea","id":"growing-notes","text":"hello", "width":500,
        "auto-grow":[2,6], "search":{"query":"hello"}
    }]}))
    .unwrap();
    event_tx
        .send(HostEvent::tree(tree.clone(), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let view = production_view(handle, cx);
    let notes = view.read_with(cx, |view, _| {
        view.test_textarea_state("growing-notes").unwrap()
    });
    let initial_height = notes.read_with(cx, |state, _| state.input_bounds().size.height);
    assert_eq!(
        notes.read_with(cx, |state, _| state.search_session().matcher.len()),
        1
    );
    tree.children[0].text = Some("hello\nsecond\nthird\nfourth\nfifth\nhello\nseventh".into());
    event_tx
        .send(HostEvent::tree(tree.clone(), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    let grown_height = notes.read_with(cx, |state, _| state.input_bounds().size.height);
    assert!(
        grown_height > initial_height,
        "auto-grow must increase the native control height"
    );
    assert_eq!(
        notes.read_with(cx, |state, _| state.search_session().matcher.len()),
        2
    );
    event_tx
        .send(HostEvent::tree(tree.clone(), None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        notes.read_with(cx, |state, _| state.input_bounds().size.height),
        grown_height
    );
    tree.children[0].auto_grow = None;
    tree.children[0].rows = Some(2);
    event_tx
        .send(HostEvent::tree(tree, None, vec![]))
        .await
        .unwrap();
    settle_root(handle, cx);
    assert_eq!(
        notes.entity_id(),
        view.read_with(cx, |view, _| view
            .test_textarea_state("growing-notes")
            .unwrap()
            .entity_id())
    );
    assert_eq!(
        notes.read_with(cx, |state, _| state.input_bounds().size.height),
        initial_height
    );
    handle
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
}
