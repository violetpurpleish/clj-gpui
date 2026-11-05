//! Kit 0.9 retained Diff and speech bindings; native components own interaction.
use super::*;
use gpui_component::diff::*;
use gpui_component::speech::{
    SpeechButton, SpeechEvent, SpeechIndicator, SpeechState, SpeechWaveform,
};

struct DiffSlot {
    state: Entity<DiffState>,
    source: Value,
    request: Option<Value>,
    error: Option<String>,
}

#[derive(Default)]
pub(super) struct State {
    edits: HashMap<String, (EntityId, bool)>,
    marks: HashMap<String, EditorMarks>,
    diffs: HashMap<String, DiffSlot>,
    speech: HashMap<String, Entity<SpeechState>>,
    speech_owners: HashMap<String, String>,
}
impl State {
    pub fn edits_enabled(&self, key: &str) -> bool {
        self.edits.get(key).is_some_and(|(_, enabled)| *enabled)
    }
    pub fn refresh_edits(&mut self, key: &str, node: &Node) {
        if let Some((_, enabled)) = self.edits.get_mut(key) {
            *enabled = node.on_edits.is_some();
        }
    }
    pub fn speech_owner_mut(&mut self, key: &str) -> Option<&mut String> {
        self.speech_owners.get_mut(key)
    }
    pub fn prune(
        &mut self,
        mounted: &HashMap<String, String>,
        inputs: &HashSet<String>,
        textareas: &HashSet<String>,
    ) {
        self.edits.retain(|key, _| {
            mounted.contains_key(key) || inputs.contains(key) || textareas.contains(key)
        });
        self.marks
            .retain(|key, _| mounted.get(key).is_some_and(|k| k == "editor"));
        self.diffs
            .retain(|key, _| mounted.get(key).is_some_and(|k| k == "diff"));
        self.speech_owners
            .retain(|key, _| mounted.get(key).is_some_and(|k| k.starts_with("speech-")));
        self.speech
            .retain(|id, _| self.speech_owners.values().any(|owner| owner == id));
    }
}
fn side(value: Option<&str>) -> DiffSide {
    if value == Some("original") {
        DiffSide::Original
    } else {
        DiffSide::Modified
    }
}
fn side_name(value: DiffSide) -> &'static str {
    match value {
        DiffSide::Original => "original",
        DiffSide::Modified => "modified",
    }
}
fn position(value: &Value) -> Option<DiffLinePosition> {
    Some(DiffLinePosition::new(
        value.get("path")?.as_str()?.to_string(),
        side(value.get("side").and_then(Value::as_str)),
        value.get("line")?.as_u64()? as usize,
    ))
}
fn line_range(value: &Value) -> Option<DiffLineRange> {
    let range = value.get("range")?.as_array()?;
    let start_side = side(value.get("side").and_then(Value::as_str));
    Some(
        DiffLineRange::new(
            value.get("path")?.as_str()?.to_string(),
            start_side,
            range.first()?.as_u64()? as usize,
            range.get(1)?.as_u64()? as usize,
        )
        .with_end_side(
            value
                .get("end-side")
                .and_then(Value::as_str)
                .map(|s| side(Some(s)))
                .unwrap_or(start_side),
        ),
    )
}
fn position_json(value: &DiffLinePosition) -> Value {
    json!({"path":value.path().as_ref(), "side":side_name(value.side()), "line":value.line()})
}
fn range_json(value: &DiffLineRange) -> Value {
    json!({"path":value.path().as_ref(), "side":side_name(value.side()), "end-side":side_name(value.end_side()), "range":[value.start(),value.end()]})
}
fn option<'a>(node: &'a Node, key: &str) -> Option<&'a Value> {
    node.diff_options.as_ref()?.get(key)
}
fn flag(node: &Node, key: &str, default: bool) -> bool {
    option(node, key)
        .and_then(Value::as_bool)
        .unwrap_or(default)
}
fn number(node: &Node, key: &str, default: usize) -> usize {
    option(node, key)
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(default)
}
fn name<'a>(node: &'a Node, key: &str) -> Option<&'a str> {
    option(node, key).and_then(Value::as_str)
}
fn parse_files(node: &Node) -> Result<Vec<DiffFile>, String> {
    let text = node.text.as_deref().unwrap_or("");
    let files = match name(node, "source") {
        Some("text") => vec![DiffFile::unchanged(
            name(node, "path").unwrap_or("source").to_string(),
            text,
        )],
        Some("conflicts") => vec![
            DiffFile::parse_conflicts(name(node, "path").unwrap_or("source").to_string(), text)
                .map_err(|e| e.to_string())?,
        ],
        _ => DiffFile::parse_with(
            text,
            DiffParseOptions::new().truncated_hunks(flag(node, "truncated-hunks", false)),
        )
        .map_err(|e| e.to_string())?,
    };
    Ok(files
        .into_iter()
        .map(|file| {
            if let Some(language) = &node.language {
                file.with_language(language.clone())
            } else {
                file
            }
        })
        .collect())
}

impl RootView {
    pub(super) fn render_release_090(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match node.kind.as_str() {
            "diff" => self.render_diff(node, path, key, window, cx),
            "resize-handle" => render_resize_handle(node, key, cx),
            _ => self.render_speech(node, key, cx),
        }
    }
    fn render_speech(&mut self, node: &Node, key: &str, cx: &mut Context<Self>) -> AnyElement {
        let id = node.speech_id.clone().unwrap_or_else(|| key.to_string());
        self.release_090
            .speech_owners
            .insert(key.into(), id.clone());
        let state = if let Some(state) = self.release_090.speech.get(&id) {
            state.clone()
        } else {
            let state = cx.new(SpeechState::new);
            let owned = id.clone();
            cx.subscribe(&state, move |this, _, event: &SpeechEvent, _| {
                let value = match event {
                    SpeechEvent::Started => json!({"event":"started"}),
                    SpeechEvent::Partial(text) => json!({"event":"partial","text":text.as_ref()}),
                    SpeechEvent::Final(text) => json!({"event":"final","text":text.as_ref()}),
                    SpeechEvent::Cancelled => json!({"event":"cancelled"}),
                    SpeechEvent::Error(error) => json!({"event":"error","error":error.to_string()}),
                };
                for (key, id) in &this.release_090.speech_owners {
                    if id == &owned {
                        this.callback_queue
                            .push(overlay::QueuedAction::WidgetValue {
                                key: key.clone(),
                                event: "speech".into(),
                                value: value.clone(),
                            });
                    }
                }
                this.flush_callback_queue();
            })
            .detach();
            self.release_090.speech.insert(id, state.clone());
            state
        };
        match node.kind.as_str() {
            "speech-button" => apply_style(
                SpeechButton::new(&state)
                    .disabled(node.disabled)
                    .show_when_unsupported(node.show_when_unsupported.unwrap_or(false))
                    .with_size(mapping::parse_scale(node.control_size.as_deref())),
                node,
                cx,
            )
            .into_any_element(),
            "speech-indicator" => apply_style(div(), node, cx)
                .child(
                    SpeechIndicator::new(&state)
                        .with_size(mapping::parse_scale(node.control_size.as_deref())),
                )
                .into_any_element(),
            _ => {
                let mut waveform = SpeechWaveform::new(&state);
                if node.variant.as_deref() == Some("liquid") {
                    waveform = waveform.liquid();
                }
                apply_style(waveform, node, cx).into_any_element()
            }
        }
    }
    fn render_diff(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let source = json!([
            node.text,
            node.language,
            option(node, "source"),
            option(node, "path"),
            option(node, "truncated-hunks")
        ]);
        if !self.release_090.diffs.contains_key(key) {
            let state = cx.new(|cx| DiffState::new([], cx));
            let owned = key.to_string();
            cx.subscribe(&state,move |this,state,event:&DiffEvent,cx| {
                let value = match event {
                    DiffEvent::SelectionStarted(range) => json!({"event":"selection-started","selection":range_json(range)}),
                    DiffEvent::SelectionChanged(range) => json!({"event":"selection-changed","selection":range.as_ref().map(range_json),"text":state.read(cx).selected_text(cx)}),
                    DiffEvent::SelectionEnded(range) => json!({"event":"selection-ended","selection":range_json(range),"text":state.read(cx).selected_text(cx)}),
                    DiffEvent::FileExpanded(path) => json!({"event":"file-expanded","path":path.as_ref()}),
                    DiffEvent::FileCollapsed(path) => json!({"event":"file-collapsed","path":path.as_ref()}),
                    DiffEvent::ConflictResolved(path,index) => json!({"event":"conflict-resolved","path":path.as_ref(),"index":index,"text":state.read(cx).resolved_text(path)}),
                    _ => return,
                };
                this.callback_queue.push(overlay::QueuedAction::WidgetValue{key:owned.clone(),event:"change".into(),value});
                this.flush_callback_queue();
            }).detach();
            self.release_090.diffs.insert(
                key.into(),
                DiffSlot {
                    state,
                    source: Value::Null,
                    request: None,
                    error: None,
                },
            );
        }
        let slot = self.release_090.diffs.get_mut(key).unwrap();
        if slot.source != source {
            match parse_files(node) {
                Ok(files) => {
                    slot.state
                        .update(cx, |state, cx| state.set_files(files, cx));
                    slot.error = None;
                }
                Err(error) => {
                    slot.error = Some(error);
                    slot.state.update(cx, |state, cx| state.set_files([], cx));
                }
            }
            slot.source = source;
        }
        if let Some(error) = &slot.error {
            return apply_style(div(), node, cx)
                .text_color(cx.theme().danger)
                .child(format!("Invalid diff: {error}"))
                .into_any_element();
        }
        let state = slot.state.clone();
        state.update(cx, |state, cx| {
            state.set_mode(
                if node.diff_mode.as_deref() == Some("split") {
                    DiffMode::Split
                } else {
                    DiffMode::Unified
                },
                cx,
            );
            state.set_context_lines(
                if flag(node, "expand-all", false) {
                    None
                } else {
                    Some(node.context_lines.unwrap_or(3))
                },
                cx,
            );
            state.set_expansion_lines(number(node, "expansion-lines", 20), cx);
            state.set_min_collapsed_lines(number(node, "min-collapsed-lines", 4), cx);
            state.set_inline_unit(
                match name(node, "inline-unit") {
                    Some("none") => None,
                    Some("character") => Some(DiffInlineUnit::Character),
                    _ => Some(DiffInlineUnit::Word),
                },
                cx,
            );
        });
        if slot.request != node.diff_action {
            if let Some(action) = &node.diff_action {
                state.update(cx, |state, cx| {
                    match action.get("action").and_then(Value::as_str) {
                        Some("next-change") => state.next_change(cx),
                        Some("previous-change") => state.previous_change(cx),
                        Some("expand-all") => state.expand_unchanged(cx),
                        Some("collapse-all") => state.collapse_unchanged(cx),
                        Some("scroll-to-line") => {
                            if let Some(pos) = position(action) {
                                state.scroll_to_line(pos, cx);
                            }
                        }
                        Some("scroll-to-file") => {
                            if let Some(path) = action.get("path").and_then(Value::as_str) {
                                state.scroll_to_file(path, cx);
                            }
                        }
                        Some("select") => state.set_selected_lines(line_range(action), cx),
                        _ => (),
                    }
                });
            }
            slot.request = node.diff_action.clone();
        }
        let mut diff = Diff::new(&state)
            .line_number(node.line_number.unwrap_or(true))
            .soft_wrap(node.soft_wrap.unwrap_or(true))
            .header_visible(flag(node, "header-visible", true))
            .syntax_highlight(flag(node, "syntax-highlight", true))
            .change_background(flag(node, "change-background", true))
            .hover_highlight(match name(node, "hover-highlight") {
                Some("line") => DiffHoverHighlight::Line,
                Some("line-number") => DiffHoverHighlight::LineNumber,
                Some("both") => DiffHoverHighlight::Both,
                _ => DiffHoverHighlight::None,
            })
            .hunk_separator(match name(node, "hunk-separator") {
                Some("simple") => DiffHunkSeparator::Simple,
                Some("line-info") => DiffHunkSeparator::LineInfo,
                _ => DiffHunkSeparator::Metadata,
            })
            .change_indicator(match name(node, "change-indicator") {
                Some("bars") => DiffChangeIndicator::Bars,
                Some("none") => DiffChangeIndicator::None,
                _ => DiffChangeIndicator::Signs,
            })
            .decorations(node.diff_decorations.iter().filter_map(|v| {
                let mut d = DiffDecoration::new(line_range(v)?);
                if let Some(color) = v
                    .get("color")
                    .and_then(Value::as_str)
                    .and_then(extra::parse_hex_color)
                {
                    d = d.with_background(color);
                }
                Some(d)
            }));
        if node.on_click.is_some() {
            let emit = Self::action_emitter(cx);
            let owned = key.to_string();
            diff = diff.on_line_click(move |pos, _, _, cx| {
                emit(
                    overlay::QueuedAction::WidgetValue {
                        key: owned.clone(),
                        event: "click".into(),
                        value: position_json(pos),
                    },
                    cx,
                )
            });
        }
        if node.on_hover.is_some() {
            let emit = Self::action_emitter(cx);
            let owned = key.to_string();
            diff = diff.on_line_hover(move |pos, _, cx| {
                emit(
                    overlay::QueuedAction::WidgetValue {
                        key: owned.clone(),
                        event: "hover".into(),
                        value: pos.map(position_json).unwrap_or(Value::Null),
                    },
                    cx,
                )
            });
        }
        if node.on_add_annotation.is_some() {
            let emit = Self::action_emitter(cx);
            let owned = key.to_string();
            diff = diff.on_add_annotation(move |range, _, cx| {
                emit(
                    overlay::QueuedAction::WidgetValue {
                        key: owned.clone(),
                        event: "add-annotation".into(),
                        value: range_json(range),
                    },
                    cx,
                )
            });
        }
        let annotations: Vec<_> = node
            .items
            .iter()
            .filter_map(|item| {
                let id = item.id.clone()?;
                let value = item.value.as_ref()?;
                Some(if let Some(pos) = position(value) {
                    DiffAnnotation::line(id, pos)
                } else {
                    DiffAnnotation::file(id, value.get("path")?.as_str()?.to_string())
                })
            })
            .collect();
        let items = node.items.clone();
        let weak = cx.entity().downgrade();
        let owned = path.to_string();
        diff = diff
            .annotations(annotations)
            .render_annotation(move |annotation, window, cx| {
                items
                    .iter()
                    .enumerate()
                    .find(|(_, i)| {
                        i.id.as_ref()
                            .is_some_and(|id| ElementId::from(eid(id)) == *annotation.id())
                    })
                    .and_then(|(i, item)| item.content.as_ref().map(|n| (i, n)))
                    .and_then(|(i, n)| {
                        weak.update(cx, |this, cx| {
                            this.render_node(n, &format!("{owned}-item-{i}"), window, cx)
                        })
                        .ok()
                    })
                    .unwrap_or_else(|| div().into_any_element())
            });
        let markers: Vec<_> = node
            .options
            .iter()
            .filter_map(|item| {
                Some(
                    DiffGutterMarker::new(item.id.clone()?, position(item.value.as_ref()?)?)
                        .with_label(item.label.clone().unwrap_or_default()),
                )
            })
            .collect();
        let items = node.options.clone();
        let weak = cx.entity().downgrade();
        let owned = path.to_string();
        diff = diff
            .gutter_markers(markers)
            .render_gutter_marker(move |marker, window, cx| {
                items
                    .iter()
                    .enumerate()
                    .find(|(_, i)| {
                        i.id.as_ref()
                            .is_some_and(|id| ElementId::from(eid(id)) == *marker.id())
                    })
                    .and_then(|(i, item)| item.content.as_ref().map(|n| (i, n)))
                    .and_then(|(i, n)| {
                        weak.update(cx, |this, cx| {
                            this.render_node(n, &format!("{owned}-option-{i}"), window, cx)
                        })
                        .ok()
                    })
                    .unwrap_or_else(|| div().into_any_element())
            });
        viewport_sized(diff, node, 360., cx)
    }
}

#[derive(Clone)]
struct ResizeDrag {
    key: String,
}
impl Render for ResizeDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
fn render_resize_handle(node: &Node, key: &str, cx: &mut Context<RootView>) -> AnyElement {
    use gpui_component::resizable::{HandleEdge, resize_handle};
    let mut handle = resize_handle(
        key.to_string(),
        mapping::parse_axis(node.orientation.as_deref()),
    )
    .on_drag(ResizeDrag { key: key.into() }, |drag, _, _, cx| {
        cx.new(|_| drag.clone())
    });
    if let Some(edge) = node.placement.as_deref() {
        handle = handle.inside(if edge == "leading" {
            HandleEdge::Leading
        } else {
            HandleEdge::Trailing
        });
    }
    let emit = RootView::action_emitter(cx);
    let owned = key.to_string();
    handle.on_drag_move(move |event:&gpui::DragMoveEvent<ResizeDrag>,_,cx| {
        if event.drag(cx).key==owned { emit(overlay::QueuedAction::WidgetValue{key:owned.clone(),event:"change".into(),value:json!({"x":f32::from(event.event.position.x),"y":f32::from(event.event.position.y)})},cx); }
    }).into_any_element()
}

pub(super) fn cursor_blink(value: &Value) -> Option<gpui_kit::base::input::CursorBlink> {
    use gpui_kit::base::input::CursorBlink;
    if value.as_str() == Some("steady") {
        return Some(CursorBlink::steady());
    }
    let interval = value
        .as_u64()
        .or_else(|| value.get("interval-ms").and_then(Value::as_u64))?;
    let policy = CursorBlink::blinking(Duration::from_millis(interval));
    Some(
        if let Some(timeout) = value.get("idle-timeout-ms").and_then(Value::as_u64) {
            policy.with_idle_timeout(Duration::from_millis(timeout))
        } else {
            policy
        },
    )
}

use gpui_kit::base::input::{
    GutterDecoration, GutterDecorationCollection, InputEditsEvent, OverviewRulerDecoration,
    OverviewRulerDecorationCollection, OverviewRulerLane,
};
struct EditorMarks {
    entity: EntityId,
    config: Value,
    gutter: Option<GutterDecorationCollection>,
    ruler: Option<OverviewRulerDecorationCollection>,
}
fn byte_range(value: &Value) -> Option<std::ops::Range<usize>> {
    let a = value.get("range")?.as_array()?;
    let start = a.first()?.as_u64()? as usize;
    let end = a.get(1)?.as_u64()? as usize;
    (start <= end).then_some(start..end)
}
fn paired_edit_value<S>(slot: &mut TextControlSlot<S>, text: String) -> Option<String> {
    slot.change.clear();
    if slot.wait_for_seq.is_some() || slot.submitted.as_ref() == Some(&text) {
        return None;
    }
    slot.submitted = None;
    Some(text)
}
impl RootView {
    pub(super) fn sync_input_edits<T: gpui::EventEmitter<InputEditsEvent> + 'static>(
        &mut self,
        key: &str,
        node: &Node,
        state: &Entity<T>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let enabled = node.on_edits.is_some();
        if let Some((entity, active)) = self.release_090.edits.get_mut(key)
            && *entity == state.entity_id()
        {
            *active = enabled;
            return;
        }
        if !enabled {
            return;
        }
        let entity = state.entity_id();
        self.release_090.edits.insert(key.into(), (entity, true));
        let slot_key = key.to_string();
        let key = node
            .id
            .clone()
            .or(node.source_path.clone())
            .unwrap_or_else(|| key.to_string());
        cx.subscribe_in(state,window,move |this,_,event:&InputEditsEvent,_,cx| {
            if this.release_090.edits.get(&slot_key) != Some(&(entity, true)) { return; }
            let value=Value::Array(event.edits().iter().map(|(edit,text)|json!({
                "start-byte":edit.start_byte,"old-end-byte":edit.old_end_byte,"new-end-byte":edit.new_end_byte,
                "start-position":[edit.start_position.row,edit.start_position.column],
                "old-end-position":[edit.old_end_position.row,edit.old_end_position.column],
                "new-end-position":[edit.new_end_position.row,edit.new_end_position.column],"text":text.to_string()
            })).collect());
            // Textarea/Editor use this immutable snapshot for both callbacks.
            // Their legacy coalescer is only used when :on-edits is absent.
            let change = event.edits().last().and_then(|(_, text)| {
                let text = text.to_string();
                if let Some(slot) = this.textareas.get_mut(&slot_key) {
                    paired_edit_value(slot, text)
                } else {
                    this.editors.get_mut(&slot_key).and_then(|slot| paired_edit_value(slot, text))
                }
            });
            this.callback_queue.push(overlay::QueuedAction::InputEdits {
                key:slot_key.clone(), callback_key:key.clone(), value, change,
            });
            // Input's separate Change event supplies its revision/echo metadata.
            let view = cx.entity().downgrade();
            cx.defer(move |cx| { let _ = view.update(cx, |this, _| this.flush_callback_queue()); });
        }).detach();
    }
    pub(super) fn sync_editor_marks(
        &mut self,
        key: &str,
        node: &Node,
        state: &Entity<EditorState>,
        cx: &mut Context<Self>,
    ) {
        if self
            .release_090
            .marks
            .get(key)
            .is_some_and(|m| m.entity != state.entity_id())
        {
            self.release_090.marks.remove(key);
        }
        let config = json!([
            node.gutter_decorations,
            node.overview_ruler_decorations,
            node.decoration_generation
        ]);
        if self
            .release_090
            .marks
            .get(key)
            .is_some_and(|m| m.config == config)
        {
            return;
        }
        if node.gutter_decorations.is_empty()
            && node.overview_ruler_decorations.is_empty()
            && !self.release_090.marks.contains_key(key)
        {
            return;
        }
        let gutter: Vec<_> = node
            .gutter_decorations
            .iter()
            .filter_map(|v| {
                let mut d = GutterDecoration::new(byte_range(v)?);
                if let Some(color) = v
                    .get("color")
                    .and_then(Value::as_str)
                    .and_then(extra::parse_hex_color)
                {
                    d = d.with_color(color);
                }
                if let Some(color) = v
                    .get("folded-color")
                    .and_then(Value::as_str)
                    .and_then(extra::parse_hex_color)
                {
                    d = d.with_folded_color(color);
                }
                Some(d)
            })
            .collect();
        let ruler: Vec<_> = node
            .overview_ruler_decorations
            .iter()
            .filter_map(|v| {
                let mut d = OverviewRulerDecoration::new(byte_range(v)?).with_lane(
                    match v.get("lane").and_then(Value::as_str) {
                        Some("left") => OverviewRulerLane::Left,
                        Some("right") => OverviewRulerLane::Right,
                        Some("full") => OverviewRulerLane::Full,
                        _ => OverviewRulerLane::Center,
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
        let marks = self
            .release_090
            .marks
            .entry(key.into())
            .or_insert_with(|| EditorMarks {
                entity: state.entity_id(),
                config: Value::Null,
                gutter: None,
                ruler: None,
            });
        if let Some(collection) = &marks.gutter {
            collection.set(gutter, cx);
        } else if !gutter.is_empty() {
            marks.gutter = Some(state.update(cx, |state, cx| {
                state.create_gutter_decorations_collection(gutter, cx)
            }));
        }
        if let Some(collection) = &marks.ruler {
            collection.set(ruler, cx);
        } else if !ruler.is_empty() {
            marks.ruler = Some(state.update(cx, |state, cx| {
                state.create_overview_ruler_decorations_collection(ruler, cx)
            }));
        }
        marks.config = config;
    }
}

#[cfg(test)]
impl RootView {
    pub(crate) fn test_diff_state(&self, key: &str) -> Option<Entity<DiffState>> {
        self.release_090.diffs.get(key).map(|s| s.state.clone())
    }
    pub(crate) fn test_speech_state(&self, key: &str) -> Option<Entity<SpeechState>> {
        self.release_090.speech.get(key).cloned()
    }
    pub(crate) fn test_editor_mark_ranges(
        &self,
        key: &str,
        cx: &App,
    ) -> Vec<std::ops::Range<usize>> {
        self.release_090
            .marks
            .get(key)
            .and_then(|m| m.gutter.as_ref().map(|c| c.get_ranges(cx)))
            .unwrap_or_default()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[gpui_kit::test]
    fn cursor_theme_scopes_restore_both_component_and_base(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| {
            let original = Theme::global(cx).clone();
            let policy = cursor_blink(&json!({"interval-ms":700,"idle-timeout-ms":3000})).unwrap();
            with_theme_apply(
                &ThemeApply::Cursor {
                    palette: Some(Box::new(ThemeApply::Appearance(ThemeMode::Dark))),
                    policy,
                },
                cx,
                |cx| {
                    assert_eq!(Theme::global(cx).cursor_blink, policy);
                    assert_eq!(gpui_kit::base::Theme::global(cx).cursor_blink, policy);
                    with_theme_apply(
                        &ThemeApply::Cursor {
                            palette: Some(Box::new(ThemeApply::Appearance(ThemeMode::Light))),
                            policy: cursor_blink(&json!("steady")).unwrap(),
                        },
                        cx,
                        |cx| {
                            assert!(Theme::global(cx).cursor_blink.is_steady());
                            assert_eq!(Theme::global(cx).mode, ThemeMode::Light);
                        },
                    );
                    assert_eq!(Theme::global(cx).cursor_blink, policy);
                    assert_eq!(Theme::global(cx).mode, ThemeMode::Dark);
                },
            );
            assert_eq!(Theme::global(cx).cursor_blink, original.cursor_blink);
            assert_eq!(
                gpui_kit::base::Theme::global(cx).cursor_blink,
                original.cursor_blink
            );
            assert_eq!(Theme::global(cx).colors, original.colors);
        });
    }
    #[test]
    fn diff_parser_handles_strict_truncated_source_and_conflicts() {
        let mut node = Node {
            text: Some("--- a/a.clj\n+++ b/a.clj\n@@ -1,2 +1,2 @@\n-old\n+new\n".into()),
            ..Node::default()
        };
        assert!(parse_files(&node).is_err());
        node.diff_options = Some(json!({"truncated-hunks":true}));
        assert!(parse_files(&node).unwrap()[0].is_truncated());
        node.diff_options = Some(json!({"source":"text","path":"a.clj"}));
        assert!(!parse_files(&node).unwrap()[0].has_changes());
        node.diff_options = Some(json!({"source":"conflicts","path":"a.clj"}));
        node.text = Some("<<<<<<< ours\nold\n=======\nnew\n>>>>>>> theirs\n".into());
        assert_eq!(parse_files(&node).unwrap()[0].conflicts().len(), 1);
    }
}
