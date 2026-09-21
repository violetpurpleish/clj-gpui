//! Kit widgets that were absent from the original binding inventory.
use super::*;
use gpui_component::Selectable as _;
use gpui_component::{
    button::ButtonGroup,
    calendar::{Calendar, CalendarEvent, CalendarState},
    carousel::{
        Carousel, CarouselContent, CarouselEvent, CarouselItem, CarouselNext, CarouselPagination,
        CarouselPaginationItem, CarouselPrevious, CarouselState,
    },
    collapsible::Collapsible,
    dialog::{
        DialogAction, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader,
        DialogTitle,
    },
    empty::{
        Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant,
        EmptyTitle,
    },
    form::{Field, Form},
    input::{
        InputGroup, InputGroupAddon, InputGroupAddonAlignment, InputGroupButton, InputGroupText,
    },
    list::{ListItem, ListSeparatorItem},
    searchable_list::SearchableListItemElement,
    sidebar::{SidebarFooter, SidebarHeader},
};

#[derive(Default)]
pub(super) struct State {
    app_menus: HashMap<String, (Entity<gpui_component::menu::AppMenuBar>, Value)>,
    carousels: HashMap<String, (Entity<CarouselState>, Option<String>)>,
    calendars: HashMap<String, (Entity<CalendarState>, Option<String>)>,
    calendar_config: HashMap<String, (Option<[i32; 2]>, Vec<String>)>,
    carousel: Option<Entity<CarouselState>>,
    carousel_index: usize,
    pub used: HashSet<String>,
    pub requests: HashMap<String, Value>,
    pub counts: HashMap<String, usize>,
    pub searches: HashMap<String, Value>,
    pub providers: HashMap<String, Value>,
    pub text_options: HashMap<String, Value>,
    pub text_requests: HashMap<String, Value>,
    pub diagnostics: HashMap<String, Value>,
    pub decorations: HashMap<String, (Value, gpui_kit::base::input::TextDecorationCollection)>,
}
impl State {
    pub fn refresh_callbacks(&mut self, key: &str, node: &Node) {
        if let Some((_, callback)) = self.carousels.get_mut(key) {
            *callback = node.on_change.clone();
        }
        if let Some((_, callback)) = self.calendars.get_mut(key) {
            *callback = node.on_change.clone();
        }
    }

    pub fn prune(&mut self, mounted: &HashMap<String, String>) {
        self.app_menus
            .retain(|key, _| self.used.contains(key) || mounted.contains_key(key));
        self.carousels
            .retain(|key, _| self.used.contains(key) || mounted.contains_key(key));
        self.calendars
            .retain(|key, _| self.used.contains(key) || mounted.contains_key(key));
        self.calendar_config
            .retain(|key, _| self.used.contains(key) || mounted.contains_key(key));
        self.requests
            .retain(|key, _| self.used.contains(key) || mounted.contains_key(key));
        self.counts
            .retain(|key, _| self.used.contains(key) || mounted.contains_key(key));
        self.searches
            .retain(|key, _| self.used.contains(key) || mounted.contains_key(key));
        self.providers
            .retain(|key, _| self.used.contains(key) || mounted.contains_key(key));
        self.text_options.retain(|key, _| mounted.contains_key(key));
        self.text_requests
            .retain(|key, _| mounted.contains_key(key));
        self.diagnostics.retain(|key, _| mounted.contains_key(key));
        self.decorations.retain(|key, _| mounted.contains_key(key));
        self.used.clear();
    }
}

pub(super) fn is_kind(kind: &str) -> bool {
    matches!(
        kind,
        "app-menu-bar"
            | "empty"
            | "empty-header"
            | "empty-media"
            | "empty-title"
            | "empty-description"
            | "empty-content"
            | "collapsible"
            | "form"
            | "field"
            | "button-group"
            | "radio"
            | "input-group"
            | "input-group-addon"
            | "input-group-button"
            | "input-group-text"
            | "carousel"
            | "carousel-content"
            | "carousel-item"
            | "carousel-previous"
            | "carousel-next"
            | "carousel-pagination"
            | "carousel-pagination-item"
            | "calendar"
            | "dialog-close"
            | "dialog-action"
            | "dialog-content"
            | "dialog-header"
            | "dialog-title"
            | "dialog-description"
            | "dialog-footer"
            | "sidebar-group"
            | "sidebar-menu"
            | "sidebar-menu-item"
            | "sidebar-toggle-button"
            | "sidebar-header"
            | "sidebar-footer"
            | "list-item"
            | "list-separator-item"
            | "searchable-list-item"
            | "tab"
            | "stepper-item"
    )
}

fn slot_children(node: &Node) -> Vec<Node> {
    let mut children = node.children.clone();
    if let Some(text) = &node.text {
        children.insert(
            0,
            Node {
                kind: "label".into(),
                text: Some(text.clone()),
                ..Node::default()
            },
        );
    }
    children
}

impl RootView {
    pub(super) fn sync_text_search<M: gpui_kit::base::input::MultiLineMode>(
        &mut self,
        key: &str,
        node: &Node,
        state: &Entity<gpui_kit::base::input::InputBaseState<M>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.parity.used.insert(key.to_string());
        let Some(search) = node.search.as_ref().filter(|s| s.is_object()) else {
            self.parity.searches.remove(key);
            return;
        };
        let previous = self
            .parity
            .searches
            .get(key)
            .cloned()
            .unwrap_or(Value::Null);
        if &previous == search {
            return;
        }
        state.update(cx, |state, cx| {
            if let Some(enabled) = search.get("enabled").and_then(Value::as_bool)
                && previous.get("enabled") != search.get("enabled")
            {
                state.set_searchable(enabled, cx);
            }
            let replace_mode = search
                .get("replace-mode")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if search.get("open").and_then(Value::as_bool) == Some(true)
                && previous.get("open") != search.get("open")
            {
                state.open_search(replace_mode, cx);
            }
            if previous.get("replace-mode") != search.get("replace-mode") {
                state.set_search_replace_mode(replace_mode, cx);
            }
            if let Some(query) = search.get("query").and_then(Value::as_str)
                && (previous.get("query") != search.get("query")
                    || previous.get("case-insensitive") != search.get("case-insensitive"))
            {
                state.set_search_query(
                    query,
                    search
                        .get("case-insensitive")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                    cx,
                );
            }
            if search.get("open").and_then(Value::as_bool) == Some(false)
                && previous.get("open") != search.get("open")
            {
                state.close_search(cx);
            }
            if previous.get("action") != search.get("action")
                || previous.get("generation") != search.get("generation")
            {
                let replacement = search
                    .get("replacement")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                match search.get("action").and_then(Value::as_str) {
                    Some("next") => {
                        state.next_search_match(cx);
                    }
                    Some("previous") => {
                        state.previous_search_match(cx);
                    }
                    Some("replace") => {
                        if state.replace_current_search_match(replacement, window, cx) {
                            cx.emit(InputEvent::Change);
                        }
                    }
                    Some("replace-all") => {
                        if state.replace_all_search_matches(replacement, window, cx) > 0 {
                            cx.emit(InputEvent::Change);
                        }
                    }
                    Some("close") => state.close_search(cx),
                    _ => {}
                }
            }
        });
        self.parity.searches.insert(key.to_string(), search.clone());
    }

    fn empty_media(
        &mut self,
        node: &Node,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> EmptyMedia {
        let variant = if node.variant.as_deref() == Some("icon") {
            EmptyMediaVariant::Icon
        } else {
            EmptyMediaVariant::Default
        };
        apply_style(EmptyMedia::new().with_variant(variant), node, cx)
            .children(self.render_children(node, path, window, cx))
    }
    fn empty_header(
        &mut self,
        node: &Node,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> EmptyHeader {
        let mut header = apply_style(EmptyHeader::new(), node, cx);
        for (i, child) in node.children.iter().enumerate() {
            let path = format!("{path}-{i}");
            match child.kind.as_str() {
                "empty-media" => header = header.media(self.empty_media(child, &path, window, cx)),
                "empty-title" => {
                    header = header.title(apply_style(EmptyTitle::new(), child, cx).children(
                        self.render_children(
                            &Node {
                                children: slot_children(child),
                                ..child.clone()
                            },
                            &path,
                            window,
                            cx,
                        ),
                    ))
                }
                "empty-description" => {
                    header = header.description(
                        apply_style(EmptyDescription::new(), child, cx).children(
                            self.render_children(
                                &Node {
                                    children: slot_children(child),
                                    ..child.clone()
                                },
                                &path,
                                window,
                                cx,
                            ),
                        ),
                    )
                }
                _ => {}
            }
        }
        header
    }
    fn form_field(
        &mut self,
        node: &Node,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Field {
        let mut field = apply_style(Field::new(), node, cx)
            .children(self.render_children(node, path, window, cx));
        if let Some(text) = node.title.clone().or_else(|| node.text.clone()) {
            field = field.label(text);
        }
        if let Some(label) = node.label_content.clone() {
            field = field.label_fn(move |_, cx| {
                embedded_node(&label, "field-label", Some(cx), None).unwrap()
            });
        }
        if let Some(description) = node.description.clone() {
            field = field.description_fn(move |_, _| description.clone());
        }
        if let Some(v) = node.visible {
            field = field.visible(v);
        }
        if let Some(v) = node.required {
            field = field.required(v);
        }
        if let Some(v) = node.label_indent {
            field = field.label_indent(v);
        }
        if let Some(v) = node.col_span {
            field = field.col_span(v);
        }
        if let Some(v) = node.col_start {
            field = field.col_start(v);
        }
        if let Some(v) = node.col_end {
            field = field.col_end(v);
        }
        match node.align.as_deref() {
            Some("end") => field.items_end(),
            Some("center") => field.items_center(),
            _ => field.items_start(),
        }
    }
    fn input_group_addon(
        &mut self,
        node: &Node,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> InputGroupAddon {
        let align = match node.align.as_deref() {
            Some("inline-end") => InputGroupAddonAlignment::InlineEnd,
            Some("block-start") => InputGroupAddonAlignment::BlockStart,
            Some("block-end") => InputGroupAddonAlignment::BlockEnd,
            _ => InputGroupAddonAlignment::InlineStart,
        };
        apply_style(
            InputGroupAddon::new(eid(&widget_key(node, path))).align(align),
            node,
            cx,
        )
        .children(self.render_children(node, path, window, cx))
    }

    pub(super) fn render_parity(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match node.kind.as_str() {
            "sidebar-group" | "sidebar-menu" | "sidebar-menu-item" | "sidebar-toggle-button" => {
                self.paint_sidebar_part(node, path, key, window, cx)
            }
            "dialog-close" => self.paint_dialog_close(node, path, key, window, cx),
            "dialog-action" => self.paint_dialog_action(node, path, key, window, cx),
            "dialog-content" => self.paint_dialog_content(node, path, key, window, cx),
            "dialog-header" => self.paint_dialog_header(node, path, key, window, cx),
            "dialog-title" => self.paint_dialog_title(node, path, key, window, cx),
            "dialog-description" => self.paint_dialog_description(node, path, key, window, cx),
            "dialog-footer" => self.paint_dialog_footer(node, path, key, window, cx),
            "sidebar-header" => self.paint_sidebar_header(node, path, key, window, cx),
            "sidebar-footer" => self.paint_sidebar_footer(node, path, key, window, cx),
            "list-item" => self.paint_list_item(node, path, key, window, cx),
            "list-separator-item" => self.paint_list_separator_item(node, path, key, window, cx),
            "searchable-list-item" => self.paint_searchable_list_item(node, path, key, window, cx),
            "tab" => self.paint_tab(node, path, key, window, cx),
            "app-menu-bar" => self.paint_app_menu_bar(node, key, cx),
            "stepper-item" => self.paint_stepper_item(node, path, key, window, cx),
            "empty" => self.paint_empty(node, path, key, window, cx),
            "empty-header" => self.paint_empty_header(node, path, key, window, cx),
            "empty-media" => self.paint_empty_media(node, path, key, window, cx),
            "empty-title" => self.paint_empty_title(node, path, key, window, cx),
            "empty-description" => self.paint_empty_description(node, path, key, window, cx),
            "empty-content" => self.paint_empty_content(node, path, key, window, cx),
            "collapsible" => self.paint_collapsible(node, path, key, window, cx),
            "field" => self.paint_field(node, path, key, window, cx),
            "form" => self.paint_form(node, path, key, window, cx),
            "button-group" => self.paint_button_group(node, path, key, window, cx),
            "radio" => self.paint_radio(node, path, key, window, cx),
            "input-group" => self.paint_input_group(node, path, key, window, cx),
            "input-group-addon" => self.paint_input_group_addon(node, path, key, window, cx),
            "input-group-text" => self.paint_input_group_text(node, path, key, window, cx),
            "input-group-button" => self.paint_input_group_button(node, path, key, window, cx),
            "calendar" => self.render_calendar(node, key, window, cx),
            "carousel" => self.render_carousel(node, path, key, window, cx),
            "carousel-pagination" => self.paint_carousel_pagination(node, path, key, window, cx),
            _ => self.paint_carousel_part(node, path, key, window, cx),
        }
    }
    fn paint_app_menu_bar(&mut self, node: &Node, key: &str, cx: &mut Context<Self>) -> AnyElement {
        fn presentation(items: &[Item]) -> Value {
            Value::Array(
                items
                    .iter()
                    .map(|item| {
                        json!({"id":item.id_or_label(),
                "label":item.label_or_id(), "disabled":item.disabled,
                "checked":item.checked,"separator":item.is_separator(),
                "items":presentation(&item.items)})
                    })
                    .collect(),
            )
        }
        self.parity.used.insert(key.to_string());
        let fingerprint = presentation(node.collection());
        let changed = self
            .parity
            .app_menus
            .get(key)
            .is_none_or(|(_, old)| *old != fingerprint);
        if changed {
            // Kit reads GlobalState only in new/reload. Restore the application's
            // menu snapshot afterwards so independent window bars cannot overwrite it.
            let previous = gpui_component::GlobalState::global(cx).app_menus().to_vec();
            let menus = node
                .collection()
                .iter()
                .filter(|item| !item.is_separator())
                .map(|item| {
                    gpui::Menu::new(item.label_or_id())
                        .disabled(item.disabled)
                        .items(action_bridge::gpui_menu_items(
                            &item.items,
                            key,
                            &[item.id_or_label()],
                        ))
                        .owned()
                })
                .collect();
            gpui_component::GlobalState::global_mut(cx).set_app_menus(menus);
            if let Some((bar, old)) = self.parity.app_menus.get_mut(key) {
                bar.update(cx, |bar, cx| bar.reload(cx));
                *old = fingerprint;
            } else {
                let bar = gpui_component::menu::AppMenuBar::new(cx);
                self.parity
                    .app_menus
                    .insert(key.to_string(), (bar, fingerprint));
            }
            gpui_component::GlobalState::global_mut(cx).set_app_menus(previous);
        }
        let bar = self.parity.app_menus.get(key).unwrap().0.clone();
        apply_style(div().h(px(node.height.unwrap_or(28.))), node, cx)
            .child(bar)
            .into_any_element()
    }

    fn paint_dialog_close(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        {
            let mut close =
                DialogClose::new().children(self.render_children(node, path, window, cx));
            if let Some(trigger) = &node.trigger {
                close = close.trigger(|button| {
                    let button = overlay::apply_button_chrome(button, trigger, Some(cx));
                    if let Some(text) = &trigger.text {
                        button.label(text.clone())
                    } else {
                        button
                    }
                });
            }
            close.into_any_element()
        }
    }

    fn paint_dialog_action(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        DialogAction::new()
            .children(self.render_children(node, path, window, cx))
            .into_any_element()
    }

    fn paint_dialog_content(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        apply_style(DialogContent::new(), node, cx)
            .children(self.render_children(node, path, window, cx))
            .into_any_element()
    }

    fn paint_dialog_header(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        apply_style(DialogHeader::new(), node, cx)
            .children(self.render_children(node, path, window, cx))
            .into_any_element()
    }

    fn paint_dialog_title(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        apply_style(DialogTitle::new(), node, cx)
            .children(self.render_children(node, path, window, cx))
            .into_any_element()
    }

    fn paint_dialog_description(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        apply_style(DialogDescription::new(), node, cx)
            .children(self.render_children(node, path, window, cx))
            .into_any_element()
    }

    fn paint_dialog_footer(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        apply_style(DialogFooter::new(), node, cx)
            .children(self.render_children(node, path, window, cx))
            .into_any_element()
    }

    fn paint_sidebar_header(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        apply_style(SidebarHeader::new(), node, cx)
            .children(self.render_children(node, path, window, cx))
            .into_any_element()
    }

    fn paint_sidebar_footer(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        apply_style(SidebarFooter::new(), node, cx)
            .children(self.render_children(node, path, window, cx))
            .into_any_element()
    }

    fn paint_list_item(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut el = ListItem::new(eid(key))
            .disabled(node.disabled)
            .selected(node.selected)
            .confirmed(node.checked.unwrap_or(false));
        if let Some(icon) = mapping::icon_from_parts(node.check_icon.as_deref(), None) {
            el = el.check_icon(icon);
        }
        if let Some(suffix) = &node.suffix {
            let suffix = suffix.clone();
            el = el.suffix(move |_, _| suffix.clone());
        }
        if let Some(callback) = &node.on_click {
            el = el.on_click(self.click(callback.clone()));
        }
        apply_style(el, node, cx)
            .children(self.render_children(node, path, window, cx))
            .into_any_element()
    }

    fn paint_list_separator_item(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        gpui::RenderOnce::render(
            ListSeparatorItem::new().children(self.render_children(node, path, window, cx)),
            window,
            cx,
        )
        .into_any_element()
    }

    fn paint_searchable_list_item(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        let mut el = SearchableListItemElement::new(hasher.finish() as usize)
            .with_size(mapping::parse_scale(node.control_size.as_deref()))
            .disabled(node.disabled)
            .selected(node.selected)
            .checked(node.checked.unwrap_or(false));
        if let Some(icon) = mapping::icon_from_parts(node.check_icon.as_deref(), None) {
            el = el.check_icon(icon);
        }
        apply_style(el, node, cx)
            .children(self.render_children(
                &Node {
                    children: slot_children(node),
                    ..node.clone()
                },
                path,
                window,
                cx,
            ))
            .into_any_element()
    }

    fn paint_tab(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut el = Tab::new()
            .selected(node.selected)
            .disabled(node.disabled)
            .with_variant(mapping::parse_tab_variant(node.variant.as_deref()))
            .with_size(mapping::parse_scale(node.control_size.as_deref()));
        if let Some(text) = &node.text {
            el = el.label(text.clone());
        }
        if let Some(label) = &node.accessibility_label {
            el = el.aria_label(label.clone());
        }
        if let Some(icon) = mapping::icon_from_parts(node.icon.as_deref(), node.icon_svg.as_deref())
        {
            el = el.icon(icon);
        }
        if let Some(prefix) = &node.prefix {
            el = el.prefix(prefix.clone());
        }
        if let Some(suffix) = &node.suffix {
            el = el.suffix(suffix.clone());
        }
        if let Some(callback) = &node.on_click {
            el = el.on_click(self.click(callback.clone()));
        }
        apply_style(el, node, cx)
            .children(self.render_children(node, path, window, cx))
            .into_any_element()
    }

    fn paint_stepper_item(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut el = StepperItem::new().disabled(node.disabled);
        if let Some(icon) = mapping::icon_from_parts(node.icon.as_deref(), node.icon_svg.as_deref())
        {
            el = el.icon(icon);
        }
        apply_style(el, node, cx)
            .children(self.render_children(node, path, window, cx))
            .into_any_element()
    }

    fn paint_empty(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        {
            let mut el = apply_style(Empty::new(), node, cx);
            for (i, child) in node.children.iter().enumerate() {
                let path = format!("{path}-{i}");
                match child.kind.as_str() {
                    "empty-header" => el = el.header(self.empty_header(child, &path, window, cx)),
                    "empty-content" => {
                        el = el.content(
                            apply_style(EmptyContent::new(), child, cx)
                                .children(self.render_children(child, &path, window, cx)),
                        )
                    }
                    _ => el = el.child(self.render_node(child, &path, window, cx)),
                }
            }
            el.into_any_element()
        }
    }

    fn paint_empty_header(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.empty_header(node, path, window, cx).into_any_element()
    }

    fn paint_empty_media(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.empty_media(node, path, window, cx).into_any_element()
    }

    fn paint_empty_title(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        apply_style(EmptyTitle::new(), node, cx)
            .children(self.render_children(node, path, window, cx))
            .into_any_element()
    }

    fn paint_empty_description(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        apply_style(EmptyDescription::new(), node, cx)
            .children(self.render_children(node, path, window, cx))
            .into_any_element()
    }

    fn paint_empty_content(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        apply_style(EmptyContent::new(), node, cx)
            .children(self.render_children(node, path, window, cx))
            .into_any_element()
    }

    fn paint_collapsible(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        {
            let mut el = apply_style(
                Collapsible::new().open(node.open.unwrap_or(false)),
                node,
                cx,
            )
            .children(self.render_children(node, path, window, cx));
            if node.id.is_some() {
                el = el.motion_id(eid(key));
            }
            if let Some(content) = &node.content {
                el = el.content(self.render_node(content, &format!("{path}-content"), window, cx));
            }
            el.into_any_element()
        }
    }

    fn paint_field(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.form_field(node, path, window, cx).into_any_element()
    }

    fn paint_form(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        {
            let mut form = apply_style(
                Form::new()
                    .layout(if node.orientation.is_some() {
                        mapping::parse_axis(node.orientation.as_deref())
                    } else {
                        Axis::Vertical
                    })
                    .with_size(mapping::parse_scale(node.control_size.as_deref())),
                node,
                cx,
            );
            if let Some(columns) = node.columns {
                form = form.columns(columns as usize);
            }
            if let Some(layout) = &node.label_layout {
                form = form.label_layout(mapping::parse_axis(Some(layout)));
            }
            if let Some(width) = node.label_width {
                form = form.label_width(px(width));
            }
            if let Some(size) = node.label_text_size {
                form = form.label_text_size(gpui::rems(size));
            }
            for (i, child) in node.children.iter().enumerate() {
                form = form.child(self.form_field(child, &format!("{path}-{i}"), window, cx));
            }
            if let Some(footer) = &node.footer {
                form = form.footer(self.render_node(footer, &format!("{path}-footer"), window, cx));
            }
            form.into_any_element()
        }
    }

    fn paint_button_group(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        {
            let mut group = ButtonGroup::new(eid(key))
                .multiple(node.multiple)
                .disabled(node.disabled)
                .layout(mapping::parse_axis(node.orientation.as_deref()));
            group = mapping::apply_named_button_variant(
                group,
                mapping::button_chrome(
                    node.variant.as_deref(),
                    node.primary,
                    node.outline,
                    node.selected,
                    node.control_size.as_deref(),
                )
                .variant,
            );
            if node.compact {
                group = group.compact();
            }
            if node.outline {
                group = group.outline();
            }
            if node.control_size.is_some() {
                group = group.with_size(mapping::parse_scale(node.control_size.as_deref()));
            }
            for (i, child) in node.children.iter().enumerate() {
                let child_path = format!("{path}-{i}");
                let mut button = overlay::apply_button_chrome(
                    Button::new(eid(&widget_key(child, &child_path))),
                    child,
                    Some(cx),
                )
                .children(self.render_children(child, &child_path, window, cx));
                if let Some(text) = &child.text {
                    button = button.label(text.clone());
                }
                if let Some(callback) = &child.on_click {
                    button = button.on_click(self.click(callback.clone()));
                }
                group = group.child(apply_style(button, child, cx));
            }
            if let Some(callback) = &node.on_change {
                let tx = self.cmd_tx.clone();
                let callback = callback.clone();
                group = group.on_click(move |indices, _, _| {
                    let _ = tx.send(Cmd::Callback {
                        id: callback.clone(),
                        value: Some(json!(indices)),
                        seq: None,
                    });
                });
            }
            apply_style(group, node, cx).into_any_element()
        }
    }

    fn paint_radio(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        {
            let mut radio = Radio::new(eid(key))
                .checked(node.checked.unwrap_or(false))
                .disabled(node.disabled);
            if let Some(text) = &node.text {
                radio = radio.label(text.clone());
            }
            if let Some(text) = mapping::kit_tooltip(node) {
                radio = radio.tooltip(text.clone());
            }
            if let Some(label) = &node.accessibility_label {
                radio = radio.accessibility_label(label.clone());
            }
            if let Some(tab) = node.tab_index {
                radio = radio.tab_index(tab as isize);
            }
            if let Some(tab) = node.tab_stop {
                radio = radio.tab_stop(tab);
            }
            if node.on_change.is_some() || node.on_click.is_some() {
                let tx = self.cmd_tx.clone();
                let change = node.on_change.clone();
                let click = node.on_click.clone();
                radio = radio.on_change(move |checked, _, _| {
                    let mut calls = Vec::new();
                    if let Some(callback) = &change {
                        calls.push(protocol::CallbackCall::with_value(
                            callback.clone(),
                            json!(checked),
                        ));
                    }
                    if let Some(callback) = &click {
                        calls.push(protocol::CallbackCall::fire(callback.clone()));
                    }
                    protocol::send_callbacks(&tx, calls);
                });
            }
            apply_style(
                radio.children(self.render_children(node, path, window, cx)),
                node,
                cx,
            )
            .into_any_element()
        }
    }

    fn paint_input_group(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        {
            let mut group = InputGroup::new(eid(key))
                .disabled(node.disabled)
                .readonly(node.readonly)
                .invalid(node.invalid.unwrap_or(false))
                .with_size(mapping::parse_scale(node.control_size.as_deref()));
            if let Some(focus) = node.focus_ring {
                group = group.focus_ring(focus);
            }
            if let Some(label) = &node.accessibility_label {
                group = group.aria_label(label.clone());
            }
            for (i, child) in node.children.iter().enumerate() {
                let path = format!("{path}-{i}");
                let key = widget_key(child, &path);
                match child.kind.as_str() {
                    "input" => {
                        let state = self.input_slot(&key, child, window, cx);
                        group = group.input(mapping::apply_input_chrome(Input::new(&state), child));
                    }
                    "textarea" => {
                        let state = self.textarea_slot(&key, child, window, cx);
                        group = group
                            .input(mapping::apply_textarea_chrome(Textarea::new(&state), child));
                    }
                    "input-group-addon" => {
                        group = group.addon(self.input_group_addon(child, &path, window, cx))
                    }
                    _ => {}
                }
            }
            apply_style(group, node, cx).into_any_element()
        }
    }

    fn paint_input_group_addon(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.input_group_addon(node, path, window, cx)
            .into_any_element()
    }

    fn paint_input_group_text(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        apply_style(InputGroupText::new(), node, cx)
            .children(self.render_children(
                &Node {
                    children: slot_children(node),
                    ..node.clone()
                },
                path,
                window,
                cx,
            ))
            .into_any_element()
    }

    fn paint_input_group_button(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        {
            let mut button = InputGroupButton::new(eid(key))
                .disabled(node.disabled)
                .loading(node.loading)
                .with_size(mapping::parse_scale(node.control_size.as_deref()))
                .children(self.render_children(node, path, window, cx));
            button = mapping::apply_named_button_variant(
                button,
                mapping::button_chrome(
                    node.variant.as_deref(),
                    node.primary,
                    node.outline,
                    node.selected,
                    node.control_size.as_deref(),
                )
                .variant,
            );
            if let Some(text) = &node.text {
                button = button.label(text.clone());
            }
            if let Some(icon) =
                mapping::icon_from_parts(node.icon.as_deref(), node.icon_svg.as_deref())
            {
                button = button.icon(icon);
            }
            if let Some(icon) = mapping::icon_from_parts(node.loading_icon.as_deref(), None) {
                button = button.loading_icon(icon);
            }
            if let Some(tooltip) = mapping::kit_tooltip(node) {
                button = button.tooltip(tooltip.clone());
            }
            if let Some(label) = &node.accessibility_label {
                button = button.accessibility_label(label.clone());
            }
            if let Some(tab) = node.tab_index {
                button = button.tab_index(tab as isize);
            }
            if node.outline {
                button = button.outline();
            }
            if node.dropdown_caret {
                button = button.dropdown_caret(true);
            }
            if let Some(callback) = &node.on_click {
                button = button.on_click(self.click(callback.clone()));
            }
            apply_style(button, node, cx).into_any_element()
        }
    }

    fn paint_carousel_pagination(
        &mut self,
        node: &Node,
        path: &str,
        _key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        apply_style(CarouselPagination::new(), node, cx)
            .children(self.render_children(node, path, window, cx))
            .into_any_element()
    }

    fn paint_carousel_part(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let kind = node.kind.as_str();
        {
            let Some(state) = self.parity.carousel.clone() else {
                return div()
                    .child(format!("{kind} requires a carousel parent"))
                    .into_any_element();
            };
            match kind {
                "carousel-content" => {
                    let previous = self.parity.carousel_index;
                    self.parity.carousel_index = 0;
                    let mut content = apply_style(CarouselContent::new(&state), node, cx)
                        .children(self.render_children(node, path, window, cx));
                    self.parity.carousel_index = previous;
                    if let Some(style) = &node.track_style {
                        content = content
                            .track_style(mapping::apply_styled(div(), style).style().clone());
                    }
                    content.into_any_element()
                }
                "carousel-item" => {
                    let index = self.parity.carousel_index;
                    self.parity.carousel_index += 1;
                    let mut item =
                        apply_style(CarouselItem::new(eid(key), index, &state), node, cx)
                            .children(self.render_children(node, path, window, cx));
                    if let Some(label) = &node.accessibility_label {
                        item = item.accessibility_label(label.clone());
                    }
                    item.into_any_element()
                }
                "carousel-previous" => {
                    let mut el = apply_style(CarouselPrevious::new(&state), node, cx);
                    if let Some(label) = &node.accessibility_label {
                        el = el.accessibility_label(label.clone());
                    }
                    el.children(self.render_children(node, path, window, cx))
                        .into_any_element()
                }
                "carousel-next" => {
                    let mut el = apply_style(CarouselNext::new(&state), node, cx);
                    if let Some(label) = &node.accessibility_label {
                        el = el.accessibility_label(label.clone());
                    }
                    el.children(self.render_children(node, path, window, cx))
                        .into_any_element()
                }
                "carousel-pagination-item" => {
                    let index = node.value.as_ref().and_then(Value::as_u64).unwrap_or(0) as usize;
                    let mut el = apply_style(
                        CarouselPaginationItem::new(eid(key), index, &state),
                        node,
                        cx,
                    );
                    if let Some(label) = &node.accessibility_label {
                        el = el.accessibility_label(label.clone());
                    }
                    el.children(self.render_children(node, path, window, cx))
                        .into_any_element()
                }
                _ => unreachable!(),
            }
        }
    }

    fn render_carousel(
        &mut self,
        node: &Node,
        path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.parity.used.insert(key.to_string());
        let count = node
            .children
            .iter()
            .filter(|child| child.kind == "carousel-content")
            .map(|child| {
                child
                    .children
                    .iter()
                    .filter(|item| item.kind == "carousel-item")
                    .count()
            })
            .sum();
        if !self.parity.carousels.contains_key(key) {
            let state = cx.new(|_| CarouselState::new(count));
            let key_owned = key.to_string();
            cx.subscribe(&state, move |this, _, event: &CarouselEvent, _| {
                let CarouselEvent::Change(index) = event;
                if let Some((_, Some(callback))) = this.parity.carousels.get(&key_owned) {
                    this.emit_value(callback.clone(), json!(index));
                }
            })
            .detach();
            self.parity
                .carousels
                .insert(key.to_string(), (state, node.on_change.clone()));
        }
        let slot = self.parity.carousels.get_mut(key).unwrap();
        slot.1 = node.on_change.clone();
        let state = slot.0.clone();
        state.update(cx, |state, cx| {
            state.set_item_count(count, cx);
            state.set_axis(mapping::parse_axis(node.orientation.as_deref()), cx);
            state.set_looping(node.looping.unwrap_or(false), cx);
            if let Some(index) = node.value.as_ref().and_then(Value::as_u64)
                && state.selected_index() != Some(index as usize)
            {
                state.set_selected_index(index as usize, cx);
            }
        });
        let previous = self.parity.carousel.replace(state.clone());
        let mut el = apply_style(Carousel::new(eid(key), &state), node, cx)
            .children(self.render_children(node, path, window, cx));
        self.parity.carousel = previous;
        if let Some(label) = &node.accessibility_label {
            el = el.accessibility_label(label.clone());
        }
        if let Some(focus) = node.focus_ring {
            el = el.focus_ring(focus);
        }
        el.into_any_element()
    }
    fn render_calendar(
        &mut self,
        node: &Node,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.parity.used.insert(key.to_string());
        if !self.parity.calendars.contains_key(key) {
            let state = cx.new(|cx| CalendarState::new(window, cx));
            let key_owned = key.to_string();
            cx.subscribe(&state, move |this, _, event: &CalendarEvent, _| {
                let CalendarEvent::Selected(date) = event;
                if let Some((_, Some(callback))) = this.parity.calendars.get(&key_owned) {
                    this.emit_value(callback.clone(), extra::date_to_value(*date));
                }
            })
            .detach();
            self.parity
                .calendars
                .insert(key.to_string(), (state, node.on_change.clone()));
        }
        let slot = self.parity.calendars.get_mut(key).unwrap();
        slot.1 = node.on_change.clone();
        let state = slot.0.clone();
        let desired = extra::date_from_value(&node.value, node.range || node.multiple);
        let config = (node.year_range, node.disabled_dates.clone());
        let changed = self.parity.calendar_config.get(key) != Some(&config);
        state.update(cx, |state, cx| {
            if changed {
                if node.disabled_dates.is_empty() {
                    state.set_disabled_matcher_shared(None);
                } else {
                    let dates = node.disabled_dates.clone();
                    state.set_disabled_matcher(
                        gpui_component::calendar::Matcher::custom(move |date| {
                            dates.contains(&date.format("%Y-%m-%d").to_string())
                        }),
                        window,
                        cx,
                    );
                }
                let [min, max] = node.year_range.unwrap_or_else(|| {
                    use chrono::Datelike as _;
                    let year = chrono::Local::now().year();
                    [year - 50, year + 50]
                });
                state.set_year_range((min, max), cx);
            }
            if state.date() != desired {
                state.set_date(desired, window, cx);
            }
        });
        self.parity.calendar_config.insert(key.to_string(), config);
        let mut el =
            Calendar::new(&state).with_size(mapping::parse_scale(node.control_size.as_deref()));
        if let Some(months) = node.number_of_months {
            el = el.number_of_months(months as usize);
        }
        el = el.first_day_of_week(mapping::parse_first_day_of_week(
            node.first_day_of_week.as_ref(),
        ));
        apply_style(el, node, cx).into_any_element()
    }
}

#[derive(Clone)]
pub(super) struct SidebarEntry(pub Node);
impl gpui_component::Collapsible for SidebarEntry {
    fn is_collapsed(&self) -> bool {
        self.0.collapsed
    }
    fn collapsed(mut self, collapsed: bool) -> Self {
        self.0.collapsed = collapsed;
        self
    }
}
impl gpui_component::sidebar::SidebarItem for SidebarEntry {
    fn render(self, id: impl Into<ElementId>, _: &mut Window, cx: &mut App) -> impl IntoElement {
        embedded_node(
            &self.0,
            self.0
                .source_path
                .as_deref()
                .unwrap_or(&id.into().to_string()),
            Some(cx),
            None,
        )
        .unwrap()
    }
}
impl RootView {
    fn paint_sidebar_part(
        &mut self,
        node: &Node,
        _path: &str,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use gpui_component::{
            Collapsible as _,
            sidebar::{SidebarGroup, SidebarItem as _, SidebarToggleButton},
        };
        match node.kind.as_str() {
            "sidebar-toggle-button" => {
                let mut button = SidebarToggleButton::new()
                    .side(extra::parse_sidebar_side(node))
                    .collapsed(node.collapsed);
                if let Some(callback) = &node.on_click {
                    button = button.on_click(self.click(callback.clone()));
                }
                button.into_any_element()
            }
            "sidebar-group" => SidebarGroup::new(
                node.title
                    .clone()
                    .or_else(|| node.text.clone())
                    .unwrap_or_default(),
            )
            .collapsed(node.collapsed)
            .children(node.children.iter().cloned().map(SidebarEntry))
            .render(eid(key), window, cx)
            .into_any_element(),
            "sidebar-menu" => {
                let items = node
                    .children
                    .iter()
                    .map(|child| self.sidebar_node_item(child, node.collapsed))
                    .collect::<Vec<_>>();
                apply_style(SidebarMenu::new().children(items), node, cx)
                    .collapsed(node.collapsed)
                    .render(eid(key), window, cx)
                    .into_any_element()
            }
            _ => self
                .sidebar_node_item(node, node.collapsed)
                .render(eid(key), window, cx)
                .into_any_element(),
        }
    }
    fn sidebar_node_item(&self, node: &Node, collapsed: bool) -> SidebarMenuItem {
        let mut item = SidebarMenuItem::new(
            node.text
                .clone()
                .or_else(|| node.title.clone())
                .unwrap_or_default(),
        )
        .active(node.selected)
        .collapsed(collapsed)
        .disable(node.disabled)
        .default_open(node.default_open.unwrap_or(false))
        .click_to_open(node.click_to_open.unwrap_or(false))
        .click_to_toggle(node.click_to_toggle.unwrap_or(false));
        if let Some(icon) = mapping::icon_from_parts(node.icon.as_deref(), node.icon_svg.as_deref())
        {
            item = item.icon(icon);
        }
        if let Some(suffix) = node.suffix.clone() {
            item = item.suffix(move |_, _| suffix.clone());
        }
        if let Some(callback) = &node.on_click {
            item = item.on_click(self.click(callback.clone()));
        }
        item = item.children(
            node.children
                .iter()
                .map(|child| self.sidebar_node_item(child, collapsed)),
        );
        mapping::apply_styled(item, node)
    }
}
