//! Retained state and compound controls introduced by Kit 0.7.
use super::*;
use gpui_component::{
    questionnaire::*,
    time_field::{HourCycle, TimeField, TimeFieldEvent, TimeFieldState, TimePrecision},
    toolbar::{Toolbar, ToolbarGroup},
};

struct QuestionnaireSlot {
    state: Entity<QuestionnaireState>,
    schema: Vec<Value>,
    shortcuts: Option<String>,
    node: Node,
    request: Option<Value>,
}

#[derive(Default)]
pub(super) struct State {
    attachment_scrolls: HashMap<String, (gpui::ScrollHandle, Value)>,
    times: HashMap<String, (Entity<TimeFieldState>, Node)>,
    questionnaires: HashMap<String, QuestionnaireSlot>,
    questionnaire: Option<Entity<QuestionnaireState>>,
    question: Option<String>,
}
impl State {
    pub fn refresh_callbacks(&mut self, key: &str, node: &Node) {
        if let Some((_, live)) = self.times.get_mut(key) {
            live.on_change = node.on_change.clone();
        }
        if let Some(slot) = self.questionnaires.get_mut(key) {
            slot.node.on_change = node.on_change.clone();
            slot.node.on_submit = node.on_submit.clone();
            slot.node.on_complete = node.on_complete.clone();
            slot.node.on_current_change = node.on_current_change.clone();
        }
    }
    pub fn prune(&mut self, mounted: &HashMap<String, String>) {
        self.attachment_scrolls
            .retain(|key, _| mounted.contains_key(key));
        self.times
            .retain(|key, _| mounted.get(key).is_some_and(|k| k == "time-field"));
        self.questionnaires
            .retain(|key, _| mounted.get(key).is_some_and(|k| k == "questionnaire"));
    }
}
pub(super) fn is_kind(kind: &str) -> bool {
    matches!(kind, "toolbar" | "toolbar-group" | "time-field") || kind.starts_with("questionnaire")
}
pub(super) fn precision(value: Option<&str>) -> TimePrecision {
    if value == Some("second") {
        TimePrecision::Second
    } else {
        TimePrecision::Minute
    }
}
pub(super) fn hour_cycle(value: Option<&str>) -> HourCycle {
    if value == Some("h12") {
        HourCycle::H12
    } else {
        HourCycle::H23
    }
}
pub(super) fn parse_time(value: &str) -> Option<chrono::NaiveTime> {
    chrono::NaiveTime::parse_from_str(value, "%H:%M:%S")
        .or_else(|_| chrono::NaiveTime::parse_from_str(value, "%H:%M"))
        .ok()
}
fn answer_json(answer: &QuestionnaireAnswer) -> Value {
    json!({"choices": answer.choices().iter().map(|s| s.as_ref()).collect::<Vec<_>>(), "freeform": answer.freeform().map(|s| s.as_ref())})
}
fn submission_json(value: &QuestionnaireSubmission) -> Value {
    Value::Array(value.items().iter().map(|i| json!({"item": i.name().as_ref(), "status": format!("{:?}", i.status()).to_lowercase(), "answer": answer_json(i.answer())})).collect())
}
fn str_field<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}
fn bool_field(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

impl RootView {
    pub(super) fn render_release_070(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match node.kind.as_str() {
            "toolbar" | "toolbar-group" => self.render_toolbar_070(node, path, key, window, cx),
            "time-field" => self.render_time_field_070(node, path, key, window, cx),
            "questionnaire" => self.render_questionnaire_070(node, path, key, window, cx),
            _ => self.render_questionnaire_part(node, path, window, cx),
        }
    }
    pub(super) fn render_attachment_group_070(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (scroll, previous) = self
            .release
            .attachment_scrolls
            .entry(key.into())
            .or_insert_with(|| (gpui::ScrollHandle::new(), Value::Null));
        let request = json!([node.scroll_offset, node.scroll_generation]);
        if *previous != request {
            if let Some([x, y]) = node.scroll_offset {
                scroll.set_offset(gpui::point(px(-x), px(-y)));
            }
            *previous = request;
        }
        let mut group =
            gpui_component::attachment::AttachmentGroup::new(key.to_string()).track_scroll(scroll);
        if let Some(color) = node.edge_fade.as_deref().and_then(extra::parse_hex_color) {
            group = group.with_edge_fade(color);
        }
        for (i, child) in node.children.iter().enumerate() {
            group = group.child(self.render_node(child, &format!("{path}-{i}"), window, cx));
        }
        apply_style(group, node, cx).into_any_element()
    }
    fn render_toolbar_070(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Prepare declarative controls before type erasure, just as Kit's
        // Sizable::prepare_for_toolbar / with_size do for Rust children.
        let size = match node.control_size.as_deref().unwrap_or("small") {
            "large" => "medium",
            s => s,
        };
        let children = node
            .children
            .iter()
            .enumerate()
            .map(|(i, child)| {
                let mut child = child.clone();
                child.control_size = Some(size.into());
                if child.kind == "button" {
                    child.variant = Some("ghost".into());
                    child.compact = true;
                }
                self.render_node(&child, &format!("{path}-{i}"), window, cx)
            })
            .collect::<Vec<_>>();
        if node.kind == "toolbar" {
            apply_style(
                Toolbar::new(key.to_string())
                    .with_size(mapping::parse_scale(Some(size)))
                    .disabled(node.disabled)
                    .contents(children),
                node,
                cx,
            )
            .into_any_element()
        } else {
            let mut group =
                ToolbarGroup::new(key.to_string()).with_size(mapping::parse_scale(Some(size)));
            if let Some(label) = node.accessibility_label.clone().or(node.text.clone()) {
                group = group.label(label);
            }
            for child in children {
                group = group.content(child);
            }
            apply_style(group, node, cx).into_any_element()
        }
    }
    fn render_time_field_070(
        &mut self,
        node: &Node,
        _path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.release.times.contains_key(key) {
            let state = cx.new(|cx| TimeFieldState::new(window, cx));
            let owned = key.to_string();
            cx.subscribe(&state, move |this, _, event: &TimeFieldEvent, cx| {
                let TimeFieldEvent::Change(time) = event;
                if let Some((_, node)) = this.release.times.get(&owned) {
                    let value = time
                        .format(if node.time_precision.as_deref() == Some("second") {
                            "%H:%M:%S"
                        } else {
                            "%H:%M"
                        })
                        .to_string();
                    this.callback_queue
                        .push(overlay::QueuedAction::WidgetValue {
                            key: owned.clone(),
                            event: "change".into(),
                            value: json!(value),
                        });
                    let entity = cx.entity();
                    cx.defer(move |cx| {
                        entity.update(cx, |this, _| this.flush_callback_queue());
                    });
                }
            })
            .detach();
            self.release
                .times
                .insert(key.into(), (state, Node::default()));
        }
        let (state, live) = self.release.times.get_mut(key).unwrap();
        let value_changed = live.value != node.value;
        *live = node.clone();
        let state = state.clone();
        state.update(cx, |s, cx| {
            s.set_precision(precision(node.time_precision.as_deref()), window, cx);
            s.set_hour_cycle(hour_cycle(node.hour_cycle.as_deref()), window, cx);
            if value_changed
                && let Some(time) = node
                    .value
                    .as_ref()
                    .and_then(Value::as_str)
                    .and_then(parse_time)
            {
                s.set_time(time, window, cx);
            }
        });
        if node.focus && !state.read(cx).focus_handle(cx).is_focused(window) {
            state.read(cx).focus_handle(cx).focus(window, cx);
        }
        apply_style(
            TimeField::new(&state)
                .disabled(node.disabled)
                .invalid(node.invalid.unwrap_or(false))
                .with_size(mapping::parse_scale(node.control_size.as_deref())),
            node,
            cx,
        )
        .into_any_element()
    }
    fn render_questionnaire_070(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = match self.questionnaire_slot(node, key, window, cx) {
            Ok(state) => state,
            Err(error) => {
                return div()
                    .child(format!("Questionnaire: {error}"))
                    .into_any_element();
            }
        };
        let previous = self.release.questionnaire.replace(state.clone());
        let children = if node.children.is_empty() {
            default_questionnaire_children(node)
        } else {
            node.children.clone()
        };
        let children = children
            .iter()
            .enumerate()
            .map(|(i, child)| {
                self.render_questionnaire_child(child, &format!("{path}-{i}"), window, cx)
            })
            .collect::<Vec<_>>();
        self.release.questionnaire = previous;
        apply_style(
            Questionnaire::new(&state)
                .with_size(mapping::parse_scale(node.control_size.as_deref()))
                .children(children),
            node,
            cx,
        )
        .into_any_element()
    }
    fn questionnaire_slot(
        &mut self,
        node: &Node,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Entity<QuestionnaireState>, String> {
        if self
            .release
            .questionnaires
            .get(key)
            .is_some_and(|slot| slot.schema != node.questions || slot.shortcuts != node.shortcuts)
        {
            self.release.questionnaires.remove(key);
        }
        if !self.release.questionnaires.contains_key(key) {
            let mut items = Vec::new();
            for q in &node.questions {
                let name = str_field(q, "id");
                if name.is_empty() {
                    return Err("each question requires a non-empty :id".into());
                }
                let mut item = QuestionnaireItemDefinition::new(
                    name.to_string(),
                    str_field(q, "label").to_string(),
                )
                .with_required(bool_field(q, "required"))
                .with_multiple(bool_field(q, "multiple"))
                .with_disabled(bool_field(q, "disabled"));
                if let Some(description) = q.get("description").and_then(Value::as_str) {
                    item = item.with_description(description.to_string());
                }
                for choice in q
                    .get("choices")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let mut c = QuestionnaireChoiceDefinition::new(
                        str_field(choice, "id").to_string(),
                        str_field(choice, "label").to_string(),
                    )
                    .with_disabled(bool_field(choice, "disabled"))
                    .with_default_selected(bool_field(choice, "selected"));
                    if let Some(description) = choice.get("description").and_then(Value::as_str) {
                        c = c.with_description(description.to_string());
                    }
                    item = item.with_choice(c);
                }
                if let Some(input) = q
                    .get("input")
                    .filter(|v| **v != Value::Bool(false) && !v.is_null())
                {
                    let state = cx.new(|cx| {
                        InputState::new(window, cx)
                            .placeholder(str_field(input, "placeholder").to_string())
                    });
                    item = item.with_input(
                        QuestionnaireInputDefinition::new(
                            state,
                            str_field(input, "label").to_string(),
                        )
                        .with_disabled(bool_field(input, "disabled")),
                    );
                }
                if let Some(pattern) = q.get("pattern").and_then(Value::as_str) {
                    let pattern = regex::Regex::new(pattern).map_err(|e| e.to_string())?;
                    let message = q
                        .get("validation-message")
                        .and_then(Value::as_str)
                        .unwrap_or("Invalid answer")
                        .to_string();
                    item = item.with_validator(move |context| {
                        if pattern.is_match(
                            context
                                .answer()
                                .freeform()
                                .map(|s| s.as_ref())
                                .unwrap_or(""),
                        ) {
                            Ok(())
                        } else {
                            Err(message.clone().into())
                        }
                    });
                }
                items.push(item);
            }
            let mut error = None;
            let state = cx.new(|cx| {
                let state = match QuestionnaireState::new(items, cx) {
                    Ok(state) => state,
                    Err(e) => {
                        error = Some(e.to_string());
                        QuestionnaireState::new(Vec::new(), cx).expect("empty schema is valid")
                    }
                };
                match node.shortcuts.as_deref() {
                    Some("letters") => state.with_shortcuts(QuestionnaireShortcutMode::Letters),
                    Some("numbers") => state.with_shortcuts(QuestionnaireShortcutMode::Numbers),
                    _ => state,
                }
            });
            if let Some(error) = error {
                return Err(error);
            }
            let owned = key.to_string();
            cx.subscribe(&state, move |this, _, event: &QuestionnaireEvent, cx| {
                let (event, value) = match event {
                    QuestionnaireEvent::AnswerChanged(change) => ("change", json!({"item": change.item().as_ref(), "answer": answer_json(change.answer()), "status": format!("{:?}", change.status()).to_lowercase()})),
                    QuestionnaireEvent::CurrentItemChanged { previous, current } => ("current-change", json!({"previous": previous.as_ref().map(|s| s.as_ref()), "current": current.as_ref().map(|s| s.as_ref())})),
                    QuestionnaireEvent::Completed(s) => ("complete", submission_json(s)),
                    QuestionnaireEvent::Submit(s) => ("submit", submission_json(s)),
                    _ => return,
                };
                this.callback_queue.push(overlay::QueuedAction::WidgetValue {
                    key: owned.clone(), event: event.into(), value,
                });
                let entity = cx.entity();
                cx.defer(move |cx| { entity.update(cx, |this, _| this.flush_callback_queue()); });
            }).detach();
            self.release.questionnaires.insert(
                key.into(),
                QuestionnaireSlot {
                    state,
                    schema: node.questions.clone(),
                    shortcuts: node.shortcuts.clone(),
                    node: Node::default(),
                    request: None,
                },
            );
        }
        let slot = self.release.questionnaires.get_mut(key).unwrap();
        let state = slot.state.clone();
        // Declarative requests apply only when changed, so unrelated renders do
        // not undo native choice/keyboard navigation or replay submissions.
        state.update(cx, |s, cx| {
            if node.answers != slot.node.answers
                && let Some(answers) = node.answers.as_ref().and_then(Value::as_object)
            {
                for (name, value) in answers {
                    let choices = value
                        .get("choices")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(str::to_string);
                    let mut answer = QuestionnaireAnswer::new().with_choices(choices);
                    if let Some(text) = value.get("freeform").and_then(Value::as_str) {
                        answer = answer.with_freeform(text.to_string());
                    }
                    if let Err(e) = s.set_answer(name, answer, window, cx) {
                        eprintln!("[host] questionnaire {key}: {e}");
                    }
                }
            }
            if node.current_item != slot.node.current_item
                && let Some(name) = &node.current_item
            {
                let _ = s.set_current_item(name, window, cx);
            }
            if node.questionnaire_errors != slot.node.questionnaire_errors {
                for q in &node.questions {
                    let name = str_field(q, "id");
                    if let Some(message) = node
                        .questionnaire_errors
                        .as_ref()
                        .and_then(|v| v.get(name))
                        .and_then(Value::as_str)
                    {
                        let _ = s.set_external_error(name, message.to_string(), cx);
                    } else {
                        let _ = s.clear_external_error(name, cx);
                    }
                }
            }
            if node.questionnaire_action != slot.request {
                if let Some(request) = &node.questionnaire_action {
                    match request
                        .as_str()
                        .unwrap_or_else(|| str_field(request, "action"))
                    {
                        "next" => {
                            s.go_next(window, cx);
                        }
                        "previous" => {
                            s.go_previous(window, cx);
                        }
                        "skip" => {
                            s.skip_current(window, cx);
                        }
                        "submit" => {
                            s.submit(window, cx);
                        }
                        "reset" => s.reset(window, cx),
                        _ => {}
                    }
                }
                slot.request = node.questionnaire_action.clone();
            }
        });
        slot.node = node.clone();
        Ok(state)
    }
    fn render_questionnaire_child(
        &mut self,
        node: &Node,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if node.kind.starts_with("questionnaire-") {
            self.render_questionnaire_part(node, path, window, cx)
        } else {
            self.render_node(node, path, window, cx)
        }
    }
    fn render_questionnaire_part(
        &mut self,
        node: &Node,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(state) = self.release.questionnaire.clone() else {
            return div().into_any_element();
        };
        let question = node
            .question
            .clone()
            .or_else(|| self.release.question.clone())
            .or_else(|| state.read(cx).current_item().map(ToString::to_string))
            .unwrap_or_default();
        let previous = self.release.question.replace(question.clone());
        let children = node
            .children
            .iter()
            .enumerate()
            .map(|(i, c)| self.render_questionnaire_child(c, &format!("{path}-{i}"), window, cx))
            .collect::<Vec<_>>();
        self.release.question = previous;
        let size = mapping::parse_scale(node.control_size.as_deref());
        macro_rules! part {
            ($element:expr) => {
                apply_style($element.with_size(size).children(children), node, cx)
                    .into_any_element()
            };
        }
        match node.kind.as_str() {
            "questionnaire-actions" => part!(QuestionnaireActions::new(&state)),
            "questionnaire-progress" => part!(QuestionnaireProgress::new(&state)),
            "questionnaire-item" => part!(QuestionnaireItem::new(&state, question)),
            "questionnaire-title" => part!(QuestionnaireTitle::new(&state, question)),
            "questionnaire-description" => part!(QuestionnaireDescription::new(&state, question)),
            "questionnaire-choices" => part!(QuestionnaireChoices::new(&state, question)),
            "questionnaire-choice" => {
                let mut choice = QuestionnaireChoice::new(
                    &state,
                    question,
                    node.value
                        .as_ref()
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                );
                for (kind, style) in [
                    ("indicator", &node.indicator_style),
                    ("content", &node.content_style),
                    ("shortcut", &node.shortcut_style),
                ] {
                    if let Some(style) = style {
                        let style = mapping::apply_styled(div(), style).style().clone();
                        choice = match kind {
                            "indicator" => choice.indicator_style(style),
                            "content" => choice.content_style(style),
                            _ => choice.shortcut_style(style),
                        };
                    }
                }
                part!(choice)
            }
            "questionnaire-choice-description" => part!(QuestionnaireChoiceDescription::new()),
            "questionnaire-error" => part!(QuestionnaireError::new(&state, question)),
            "questionnaire-input" => apply_style(
                QuestionnaireInput::new(&state, question).with_size(size),
                node,
                cx,
            )
            .into_any_element(),
            "questionnaire-previous" => part!(QuestionnairePrevious::new(&state)),
            "questionnaire-next" => part!(QuestionnaireNext::new(&state)),
            "questionnaire-skip" => part!(QuestionnaireSkip::new(&state)),
            "questionnaire-submit" => part!(QuestionnaireSubmit::new(&state)),
            _ => div().children(children).into_any_element(),
        }
    }
}
fn default_questionnaire_children(node: &Node) -> Vec<Node> {
    let part = |kind: &str| Node {
        kind: kind.into(),
        ..Node::default()
    };
    let mut children = vec![part("questionnaire-progress")];
    for q in &node.questions {
        let mut item = part("questionnaire-item");
        item.question = Some(str_field(q, "id").into());
        item.children = vec![
            part("questionnaire-title"),
            part("questionnaire-description"),
        ];
        let mut choices = part("questionnaire-choices");
        for c in q
            .get("choices")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let mut choice = part("questionnaire-choice");
            choice.value = Some(json!(str_field(c, "id")));
            choices.children.push(choice);
        }
        item.children.extend([
            choices,
            part("questionnaire-input"),
            part("questionnaire-error"),
        ]);
        children.push(item);
    }
    children.push(Node {
        kind: "questionnaire-actions".into(),
        gap: Some(8.),
        children: vec![
            part("questionnaire-previous"),
            part("questionnaire-skip"),
            part("questionnaire-next"),
            part("questionnaire-submit"),
        ],
        ..Node::default()
    });
    children
}

pub(super) fn parse_date_time(value: &str) -> Option<chrono::NaiveDateTime> {
    chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M"))
        .ok()
}
pub(super) fn date_time_value(node: &Node, range: bool) -> gpui_component::date_picker::DateTime {
    use gpui_component::date_picker::DateTime;
    if !range && node.time_precision.is_some() {
        let value = node.value.as_ref().and_then(Value::as_str).and_then(|s| {
            parse_date_time(s).or_else(|| {
                extra::parse_iso_date(s).map(|d| {
                    d.and_time(
                        node.default_time
                            .as_deref()
                            .and_then(parse_time)
                            .unwrap_or_default(),
                    )
                })
            })
        });
        return DateTime::Single(value);
    }
    let date = extra::date_from_value(&node.value, range);
    let time = node
        .default_time
        .as_deref()
        .and_then(parse_time)
        .unwrap_or_default();
    if range {
        DateTime::Range(
            date.start().map(|d| d.and_time(time)),
            date.end().map(|d| d.and_time(time)),
        )
    } else {
        DateTime::Single(date.start().map(|d| d.and_time(time)))
    }
}
pub(super) fn date_time_json(
    value: gpui_component::date_picker::DateTime,
    precision: Option<&str>,
) -> Value {
    if let Some(precision) = precision {
        json!(value.start().map(|v| {
            v.format(if precision == "second" {
                "%Y-%m-%dT%H:%M:%S"
            } else {
                "%Y-%m-%dT%H:%M"
            })
            .to_string()
        }))
    } else {
        extra::date_to_value(value.date())
    }
}

#[cfg(test)]
impl RootView {
    pub(crate) fn test_time_state(&self, key: &str) -> Option<Entity<TimeFieldState>> {
        self.release.times.get(key).map(|s| s.0.clone())
    }
    pub(crate) fn test_questionnaire_state(&self, key: &str) -> Option<Entity<QuestionnaireState>> {
        self.release
            .questionnaires
            .get(key)
            .map(|s| s.state.clone())
    }
    pub(crate) fn test_date_state(&self, key: &str) -> Option<Entity<DatePickerState>> {
        self.dates.get(key).map(|s| s.state.clone())
    }
}
