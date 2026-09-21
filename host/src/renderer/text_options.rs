//! Retained text-control options and editor annotations.
use super::*;
use gpui_kit::base::input::{
    InputBaseState, InputModeKind, MultiLineMode, TabSize, TextDecoration, WrappingIndent,
};

fn options(node: &Node, entity: EntityId) -> Value {
    json!({"entity":format!("{entity:?}"), "soft-wrap":node.soft_wrap,
        "folding":node.folding,"line-number":node.line_number,
        "indent-guides":node.indent_guides,"show-whitespaces":node.show_whitespaces,
        "tab-size":node.tab_size,"hard-tabs":node.hard_tabs,
        "wrapping-indent":node.wrapping_indent,"auto-grow":node.auto_grow,"rows":node.rows,
        "scroll-beyond-last-line":node.scroll_beyond_last_line,
        "cursor-surrounding-lines":node.cursor_surrounding_lines})
}

fn common_options<M: MultiLineMode>(
    state: &mut InputBaseState<M>,
    node: &Node,
    window: &mut Window,
    cx: &mut Context<InputBaseState<M>>,
) {
    state.set_soft_wrap(node.soft_wrap.unwrap_or(true), window, cx);
    state.set_wrapping_indent(
        if node.wrapping_indent.as_deref() == Some("none") {
            WrappingIndent::None
        } else {
            WrappingIndent::Same
        },
        window,
        cx,
    );
    state.set_show_whitespaces(node.show_whitespaces.unwrap_or(false), window, cx);
    state.set_tab_size(
        TabSize {
            tab_size: node.tab_size.unwrap_or(2).max(1),
            hard_tabs: node.hard_tabs.unwrap_or(false),
        },
        cx,
    );
    state.set_scroll_beyond_last_line(node.scroll_beyond_last_line, window, cx);
    state.set_cursor_surrounding_lines(node.cursor_surrounding_lines, window, cx);
}

impl RootView {
    pub(super) fn observe_text_search<M: MultiLineMode>(
        &self,
        key: &str,
        state: &Entity<InputBaseState<M>>,
        cx: &mut Context<Self>,
    ) {
        let search_key = key.to_string();
        let mut previous_search = Value::Null;
        cx.observe(state, move |this, state, cx| {
            let session = state.read(cx).search_session();
            let value = json!({"query":session.query,"replacement":session.replacement,
                "open":session.open,"active":session.is_active(),"replace-mode":session.replace_mode,
                "case-insensitive":session.case_insensitive,"count":session.matcher.len(),
                "current":session.matcher.current()});
            if value != previous_search {
                previous_search = value.clone();
                this.callback_queue.push(overlay::QueuedAction::WidgetValue { key:search_key.clone(), event:"search".into(), value });
                this.flush_callback_queue();
            }
        }).detach();
    }
    pub(super) fn sync_text_selection<M: InputModeKind>(
        &mut self,
        key: &str,
        node: &Node,
        state: &Entity<InputBaseState<M>>,
        cx: &mut Context<Self>,
    ) {
        let request = json!([
            format!("{:?}", state.entity_id()),
            node.selected_range,
            node.selection_generation
        ]);
        if self.parity.text_requests.get(key) == Some(&request) {
            return;
        }
        if let Some([start, end]) = node.selected_range {
            state.update(cx, |state, cx| {
                state.set_selected_range(start..end.max(start), cx)
            });
        }
        self.parity.text_requests.insert(key.into(), request);
    }

    pub(super) fn sync_textarea_options(
        &mut self,
        key: &str,
        node: &Node,
        state: &Entity<TextareaState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let config = options(node, state.entity_id());
        if self.parity.text_options.get(key) != Some(&config) {
            let was_growing = self
                .parity
                .text_options
                .get(key)
                .is_some_and(|previous| !previous["auto-grow"].is_null());
            state.update(cx, |state, cx| {
                common_options(state, node, window, cx);
                if let Some([min, max]) = node.auto_grow {
                    state.set_auto_grow(min.max(1), max.max(min).max(1), cx);
                } else if was_growing {
                    let rows = node.rows.unwrap_or(3).max(1) as usize;
                    state.set_auto_grow(rows, rows, cx);
                }
            });
            self.parity.text_options.insert(key.into(), config);
        }
        self.sync_text_selection(key, node, state, cx);
    }

    pub(super) fn sync_editor_options(
        &mut self,
        key: &str,
        node: &Node,
        state: &Entity<EditorState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let config = options(node, state.entity_id());
        if self.parity.text_options.get(key) != Some(&config) {
            state.update(cx, |state, cx| {
                common_options(state, node, window, cx);
                state.set_folding(node.folding.unwrap_or(true), window, cx);
                state.set_line_number(node.line_number.unwrap_or(true), window, cx);
                state.set_indent_guides(node.indent_guides.unwrap_or(true), window, cx);
            });
            self.parity.text_options.insert(key.into(), config);
        }
        state.update(cx, |state, cx| {
            let placeholder = node.placeholder.clone().unwrap_or_default();
            if state.presentation().placeholder().as_ref() != placeholder {
                state.set_placeholder(placeholder, window, cx);
            }
            if node.focus && !state.focus_handle(cx).is_focused(window) {
                state.focus(window, cx);
            }
        });
        self.sync_text_selection(key, node, state, cx);
        self.sync_editor_annotations(key, node, state, cx);
    }

    fn sync_editor_annotations(
        &mut self,
        key: &str,
        node: &Node,
        state: &Entity<EditorState>,
        cx: &mut Context<Self>,
    ) {
        // Diagnostics address the supplied document. Decorations instead track
        // native edits until their declarative definition/generation changes.
        let diagnostics = json!([
            format!("{:?}", state.entity_id()),
            node.text,
            node.diagnostics
        ]);
        if self.parity.diagnostics.get(key) != Some(&diagnostics) {
            let values: Vec<lsp_types::Diagnostic> = node
                .diagnostics
                .iter()
                .filter_map(|value| serde_json::from_value(value.clone()).ok())
                .collect();
            state.update(cx, |state, cx| {
                let text = state.text().clone();
                if let Some(set) = state.diagnostics_mut() {
                    set.reset(&text);
                    set.extend(values);
                }
                cx.notify();
            });
            self.parity.diagnostics.insert(key.into(), diagnostics);
        }
        let decorations = json!([
            format!("{:?}", state.entity_id()),
            node.decorations,
            node.decoration_generation
        ]);
        if self
            .parity
            .decorations
            .get(key)
            .is_some_and(|(config, _)| config == &decorations)
        {
            return;
        }
        let values = node.decorations.iter().filter_map(decoration).collect();
        if let Some((config, collection)) = self.parity.decorations.get_mut(key)
            && config[0] == decorations[0]
        {
            collection.set(values, cx);
            *config = decorations;
        } else if !node.decorations.is_empty() {
            let collection = state.update(cx, |state, cx| {
                state.create_decorations_collection(values, cx)
            });
            self.parity
                .decorations
                .insert(key.into(), (decorations, collection));
        }
    }
}

fn decoration(value: &Value) -> Option<TextDecoration> {
    let range = value.get("range")?.as_array()?;
    let start = usize::try_from(range.first()?.as_u64()?).ok()?;
    let end = usize::try_from(range.get(1)?.as_u64()?).ok()?;
    let color = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .and_then(extra::parse_hex_color)
    };
    let style = gpui::HighlightStyle {
        color: color("color"),
        background_color: color("background"),
        font_weight: value.get("font-weight").map(|v| {
            gpui::FontWeight(match v.as_str() {
                Some("bold") => 700.,
                Some("semibold") => 600.,
                Some("medium") => 500.,
                Some("light") => 300.,
                Some("thin") => 100.,
                _ => v.as_f64().unwrap_or(400.) as f32,
            })
        }),
        font_style: value.get("italic").and_then(Value::as_bool).map(|italic| {
            if italic {
                gpui::FontStyle::Italic
            } else {
                gpui::FontStyle::Normal
            }
        }),
        underline: value
            .get("underline")
            .filter(|v| **v != Value::Bool(false))
            .map(|_| gpui::UnderlineStyle {
                thickness: px(1.),
                color: color("underline-color"),
                wavy: value.get("underline").and_then(Value::as_str) == Some("wavy"),
            }),
        strikethrough: value
            .get("strikethrough")
            .and_then(Value::as_bool)
            .filter(|v| *v)
            .map(|_| gpui::StrikethroughStyle {
                thickness: px(1.),
                color: color("strikethrough-color"),
            }),
        fade_out: value
            .get("fade-out")
            .and_then(Value::as_f64)
            .map(|v| (v as f32).clamp(0., 1.)),
    };
    (start < end).then(|| TextDecoration::new(start..end, style))
}
