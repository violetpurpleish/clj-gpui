//! Opt-in tokens, geometric editor annotations and retained Markdown search.
use super::*;
use gpui_component::text::{RangeHighlight, TextViewState};
use gpui_kit::base::input::{
    InlineToken, InlineTokenSpan, InputContent, RangeDecoration, RangeDecorationCollection,
    RangeDecorationStyle,
};

#[derive(Default)]
pub(super) struct State {
    tokens: HashMap<String, (EntityId, Value, Option<Value>)>,
    ranges: HashMap<String, (Value, RangeDecorationCollection)>,
    markdown: HashMap<String, MarkdownSlot>,
}
struct MarkdownSlot {
    state: Entity<TextViewState>,
    node: Node,
    annotations: Value,
    snapshot: Value,
}
impl State {
    pub fn pending_token_insert(&self, key: &str, node: &Node) -> bool {
        self.tokens.get(key).is_some_and(|p| {
            node.token_insert.is_some()
                && p.2 == node.token_insert
                && p.1 == json!([node.text, node.tokens])
        })
    }
    pub fn prune(
        &mut self,
        mounted: &HashMap<String, String>,
        used_inputs: &HashSet<String>,
        used_textareas: &HashSet<String>,
    ) {
        self.tokens.retain(|key, _| {
            mounted.contains_key(key) || used_inputs.contains(key) || used_textareas.contains(key)
        });
        self.ranges.retain(|key, _| mounted.contains_key(key));
        self.markdown.retain(|key, _| mounted.contains_key(key));
    }
}
fn range(value: &Value) -> Option<std::ops::Range<usize>> {
    let a = value.as_array()?;
    let start = usize::try_from(a.first()?.as_u64()?).ok()?;
    let end = usize::try_from(a.get(1)?.as_u64()?).ok()?;
    (start <= end).then_some(start..end)
}
fn token(value: &Value) -> Option<InlineToken> {
    let mut token = InlineToken::new(
        value.get("id")?.as_str()?.to_string(),
        value.get("text")?.as_str()?.to_string(),
    );
    if let Some(label) = value.get("label").and_then(Value::as_str) {
        token = token.with_label(label.to_string());
    }
    Some(token)
}
fn token_content(text: &str, tokens: &[Value]) -> Result<InputContent, String> {
    let mut content = InputContent::new(text.to_string());
    for value in tokens {
        let range = value
            .get("range")
            .and_then(range)
            .ok_or("token requires a UTF-8 byte :range")?;
        let token = token(value).ok_or("token requires string :id and :text")?;
        content = content
            .with_token(range, token)
            .map_err(|e| e.to_string())?;
    }
    Ok(content)
}
fn span_json(span: &InlineTokenSpan) -> Value {
    let token = span.token();
    json!({"id":token.id().as_ref(),"text":token.text().as_ref(),"label":token.label().as_ref(),"range":[span.range().start,span.range().end]})
}
fn content_json(content: &InputContent) -> Value {
    json!({"text":content.text().as_ref(),"tokens":content.tokens().iter().map(span_json).collect::<Vec<_>>()})
}
macro_rules! token_methods {
    ($sync:ident, $state:ty) => {
        pub(super) fn $sync(
            &mut self,
            key: &str,
            node: &Node,
            state: &Entity<$state>,
            window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            let config = json!([node.text, node.tokens]);
            let previous = self
                .text_070
                .tokens
                .get(key)
                .filter(|p| p.0 == state.entity_id());
            let first = previous.is_none();
            let changed = previous.is_none_or(|p| p.1 != config);
            let insert = previous.is_none_or(|p| p.2 != node.token_insert);
            if first {
                let owned = node
                    .id
                    .clone()
                    .or(node.source_path.clone())
                    .unwrap_or_else(|| key.to_string());
                cx.subscribe(state, move |this, state, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        let value = content_json(&state.read(cx).content());
                        this.callback_queue
                            .push(overlay::QueuedAction::WidgetValue {
                                key: owned.clone(),
                                event: "content-change".into(),
                                value,
                            });
                        this.flush_callback_queue();
                    }
                })
                .detach();
            }
            if changed
                && (!node.tokens.is_empty()
                    || previous.is_some_and(|p| p.1[1].as_array().is_some_and(|v| !v.is_empty())))
            {
                match token_content(node.text.as_deref().unwrap_or(""), &node.tokens) {
                    Ok(content) => {
                        if content_json(&state.read(cx).content()) != content_json(&content) {
                            state.update(cx, |state, cx| state.set_value(content, window, cx));
                        }
                    }
                    Err(e) => eprintln!("[host] {key}: {e}"),
                }
            }
            if insert
                && let Some(request) = &node.token_insert
                && let Some(token) = token(request)
            {
                let result = state.update(cx, |state, cx| {
                    if let Some(range) = request.get("range").and_then(range) {
                        state.replace_range_with_token(range, token, window, cx)
                    } else {
                        state.replace_with_token(token, window, cx)
                    }
                });
                if let Err(e) = result {
                    eprintln!("[host] {key}: {e}");
                }
            }
            self.text_070.tokens.insert(
                key.into(),
                (state.entity_id(), config, node.token_insert.clone()),
            );
        }
    };
}
impl RootView {
    token_methods!(sync_input_tokens, InputState);
    token_methods!(sync_textarea_tokens, TextareaState);

    pub(super) fn sync_range_decorations(
        &mut self,
        key: &str,
        node: &Node,
        state: &Entity<EditorState>,
        cx: &mut Context<Self>,
    ) {
        let config = json!([
            format!("{:?}", state.entity_id()),
            node.range_decorations,
            node.decoration_generation
        ]);
        if self.text_070.ranges.get(key).is_some_and(|p| p.0 == config) {
            return;
        }
        let values = node
            .range_decorations
            .iter()
            .filter_map(|v| {
                let mut d = RangeDecoration::new(v.get("range").and_then(range)?).with_style(
                    if v.get("style").and_then(Value::as_str) == Some("fill") {
                        RangeDecorationStyle::Fill
                    } else {
                        RangeDecorationStyle::Frame
                    },
                );
                if let Some(color) = v
                    .get("color")
                    .and_then(Value::as_str)
                    .and_then(extra::parse_hex_color)
                {
                    d = d.with_color(color);
                }
                Some(d)
            })
            .collect();
        if let Some((previous, collection)) = self.text_070.ranges.get_mut(key)
            && previous[0] == config[0]
        {
            collection.set(values, cx);
            *previous = config;
        } else if !node.range_decorations.is_empty() {
            let collection = state.update(cx, |s, cx| {
                s.create_range_decorations_collection(values, cx)
            });
            self.text_070
                .ranges
                .insert(key.into(), (config, collection));
        }
    }
    pub(super) fn render_markdown_070(
        &mut self,
        node: &Node,
        key: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let html = node.kind == "html" || node.format.as_deref() == Some("html");
        if self.text_070.markdown.get(key).is_some_and(|s| {
            (s.node.kind == "html" || s.node.format.as_deref() == Some("html")) != html
        }) {
            self.text_070.markdown.remove(key);
        }
        if !self.text_070.markdown.contains_key(key) {
            let state = cx.new(|cx| {
                if html {
                    TextViewState::html("", cx)
                } else {
                    TextViewState::markdown("", cx)
                }
            });
            let owned = key.to_string();
            cx.observe(&state, move |this, _, cx| {
                this.update_markdown_annotations(&owned, cx)
            })
            .detach();
            self.text_070.markdown.insert(
                key.into(),
                MarkdownSlot {
                    state,
                    node: Node::default(),
                    annotations: Value::Null,
                    snapshot: Value::Null,
                },
            );
        }
        let slot = self.text_070.markdown.get_mut(key).unwrap();
        let state = slot.state.clone();
        let text = node
            .text
            .clone()
            .or(node.message.clone())
            .unwrap_or_default();
        if slot.node.text != node.text || slot.node.message != node.message {
            state.update(cx, |s, cx| s.set_text(&text, cx));
        }
        slot.node = node.clone();
        self.update_markdown_annotations(key, cx);
        extra::paint_markdown_with_state(node, key, Some(&state))
    }
    fn update_markdown_annotations(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(slot) = self.text_070.markdown.get_mut(key) else {
            return;
        };
        let html = slot.node.kind == "html" || slot.node.format.as_deref() == Some("html");
        let annotations_requested = !html
            && (!slot.node.range_highlights.is_empty()
                || slot.node.search.is_some()
                || slot.node.reveal_range.is_some()
                || !slot.annotations.is_null());
        if !annotations_requested && slot.node.on_text_state.is_none() {
            return;
        }
        let state = slot.state.clone();
        let rendered = state.read(cx).rendered_text();
        let config = json!([
            rendered.as_str(),
            slot.node.range_highlights,
            slot.node.search,
            slot.node.reveal_range,
            slot.node.reveal_generation
        ]);
        if annotations_requested && slot.annotations != config {
            let mut highlights = slot
                .node
                .range_highlights
                .iter()
                .filter_map(|v| {
                    Some(RangeHighlight::new(
                        v.get("range").and_then(range)?,
                        v.get("color")
                            .or(v.get("background"))
                            .and_then(Value::as_str)
                            .and_then(extra::parse_hex_color)?,
                    ))
                })
                .collect::<Vec<_>>();
            if let Some(search) = &slot.node.search
                && let Some(query) = search
                    .get("query")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
            {
                let color = search
                    .get("color")
                    .and_then(Value::as_str)
                    .and_then(extra::parse_hex_color)
                    .unwrap_or_else(|| cx.theme().primary.opacity(0.3));
                highlights.extend(
                    rendered
                        .as_str()
                        .match_indices(query)
                        .map(|(start, _)| RangeHighlight::new(start..start + query.len(), color)),
                );
            }
            let reveal = slot.node.reveal_range;
            let reveal_changed = slot.annotations.get(3) != config.get(3)
                || slot.annotations.get(4) != config.get(4);
            // Store before notifying; observations must not reapply themselves.
            slot.annotations = config;
            state.update(cx, |s, cx| {
                if let Err(e) = s.set_range_highlights(highlights, cx) {
                    eprintln!("[host] {key}: {e}");
                }
                if reveal_changed
                    && let Some([start, end]) = reveal
                    && let Err(e) = s.reveal_range(start..end, cx)
                {
                    eprintln!("[host] {key}: {e}");
                }
            });
        }
        if slot.node.on_text_state.is_none() {
            return;
        }
        let source = state.read(cx).selected_source_range();
        let snapshot = json!({"rendered-text":rendered.as_str(),"selected-text":state.read(cx).selected_text(),"source-range":source.map(|r|[r.start,r.end])});
        if slot.snapshot != snapshot {
            slot.snapshot = snapshot.clone();
            self.callback_queue
                .push(overlay::QueuedAction::WidgetValue {
                    key: key.into(),
                    event: "text-state".into(),
                    value: snapshot,
                });
            self.flush_callback_queue();
        }
    }
}

pub(super) fn input_tokens(mut input: Input, node: &Node, key: &str) -> Input {
    if let Some(style) = node.token_style.clone() {
        input = input.token(move |context, _, _| {
            mapping::apply_styled(gpui_component::input::InputToken::new(context), &style)
        });
    }
    if node.on_token_click.is_some() {
        let key = key.to_string();
        input = input
            .on_token_click(move |event, window, cx| emit_token_click(&key, event, window, cx));
    }
    input
}
pub(super) fn textarea_tokens(mut input: Textarea, node: &Node, key: &str) -> Textarea {
    if let Some(style) = node.token_style.clone() {
        input = input.token(move |context, _, _| {
            mapping::apply_styled(gpui_component::input::InputToken::new(context), &style)
        });
    }
    if node.on_token_click.is_some() {
        let key = key.to_string();
        input = input
            .on_token_click(move |event, window, cx| emit_token_click(&key, event, window, cx));
    }
    input
}
fn emit_token_click(
    key: &str,
    event: &gpui_component::input::InlineTokenClickEvent,
    window: &mut Window,
    cx: &mut App,
) {
    window_action_emitter(window, cx)(
        overlay::QueuedAction::WidgetValue {
            key: key.into(),
            event: "token-click".into(),
            value: json!({"id":event.token().id().as_ref(),"text":event.token().text().as_ref(),"label":event.token().label().as_ref(),"range":[event.range().start,event.range().end]}),
        },
        cx,
    );
}

#[cfg(test)]
impl RootView {
    pub(crate) fn test_input_state(&self, key: &str) -> Option<Entity<InputState>> {
        self.inputs.get(key).map(|s| s.state.clone())
    }
    pub(crate) fn test_markdown_state(&self, key: &str) -> Option<Entity<TextViewState>> {
        self.text_070.markdown.get(key).map(|s| s.state.clone())
    }
    pub(crate) fn test_range_decoration_ranges(
        &self,
        key: &str,
        cx: &App,
    ) -> Vec<std::ops::Range<usize>> {
        self.text_070
            .ranges
            .get(key)
            .map(|s| s.1.get_ranges(cx))
            .unwrap_or_default()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn token_validation_rejects_split_unicode_and_overlaps() {
        assert!(token_content("é @Ada", &[json!({"id":"a","text":"é","range":[1,2]})]).is_err());
        assert!(
            token_content(
                "é @Ada",
                &[
                    json!({"id":"a","text":"@Ada","range":[3,7]}),
                    json!({"id":"b","text":"Ada","range":[4,7]})
                ]
            )
            .is_err()
        );
        let content =
            token_content("é @Ada", &[json!({"id":"a","text":"@Ada","range":[3,7]})]).unwrap();
        assert_eq!(content_json(&content)["tokens"][0]["range"], json!([3, 7]));
    }
}
