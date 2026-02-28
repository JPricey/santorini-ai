use eframe::egui::{
    Event, Id, Key, Popup, PopupCloseBehavior, ScrollArea, TextEdit, TextWrapMode, Widget,
};
pub struct DropdownComboBox<'a, V: Clone, S: Fn(&V) -> String, I: Iterator<Item = V>> {
    hint_text: String,
    popup_id: Id,
    buf: &'a mut String,
    items: I,
    selected: &'a mut V,
    stringer: S,
}

impl<'a, V: Clone, S: Fn(&V) -> String, I: Iterator<Item = V>> DropdownComboBox<'a, V, S, I> {
    pub fn new(
        hint_text: String,
        buf: &'a mut String,
        items: I,
        selected: &'a mut V,
        stringer: S,
    ) -> Self {
        let popup_id = Id::new(hint_text.clone());
        Self {
            hint_text,
            popup_id,
            buf,
            items,
            selected,
            stringer,
        }
    }
}

fn get_highlighted(ctx: &eframe::egui::Context, id: Id) -> Option<usize> {
    ctx.data(|d| d.get_temp::<Option<usize>>(id)).flatten()
}

fn set_highlighted(ctx: &eframe::egui::Context, id: Id, val: Option<usize>) {
    ctx.data_mut(|d| d.insert_temp(id, val));
}

impl<'a, V: Clone + PartialEq, S: Fn(&V) -> String, I: Iterator<Item = V>> Widget
    for DropdownComboBox<'a, V, S, I>
{
    fn ui(self, ui: &mut eframe::egui::Ui) -> eframe::egui::Response {
        let Self {
            hint_text,
            popup_id,
            buf,
            items,
            mut selected,
            stringer,
        } = self;
        let old_selected = selected.clone();
        let highlight_id = popup_id.with("_highlight");

        let edit = TextEdit::singleline(buf)
            .hint_text(hint_text)
            .desired_width(100.0);
        let edit_show = edit.show(ui);
        let edit_response = edit_show.response;

        let mut items_for_display: Vec<_> = items
            .filter(|item| {
                let item_text = stringer(&item);
                item_text.to_lowercase().contains(&buf.to_lowercase())
            })
            .collect();
        if buf.len() > 1 {
            items_for_display.sort_by_key(|f| {
                let item_text = stringer(&f);
                !item_text.to_lowercase().starts_with(&buf.to_lowercase())
            });
        }

        let item_count = items_for_display.len();
        let mut highlighted_idx = get_highlighted(ui.ctx(), highlight_id);

        // Handle keyboard navigation while the text field has focus
        if edit_response.has_focus() && item_count > 0 {
            let mut arrow_pressed = false;
            let mut enter_pressed = false;
            ui.input(|input| {
                for event in &input.events {
                    match event {
                        Event::Key { key: Key::ArrowDown, pressed: true, .. } => {
                            arrow_pressed = true;
                            highlighted_idx = Some(match highlighted_idx {
                                Some(i) => (i + 1).min(item_count - 1),
                                None => 0,
                            });
                        }
                        Event::Key { key: Key::ArrowUp, pressed: true, .. } => {
                            arrow_pressed = true;
                            highlighted_idx = Some(match highlighted_idx {
                                Some(i) => i.saturating_sub(1),
                                None => 0,
                            });
                        }
                        Event::Key { key: Key::Enter, pressed: true, .. } => {
                            if highlighted_idx.is_some() {
                                enter_pressed = true;
                            }
                        }
                        _ => {}
                    }
                }
            });

            if enter_pressed {
                if let Some(idx) = highlighted_idx {
                    if let Some(item) = items_for_display.get(idx) {
                        *selected = item.clone();
                        *buf = (stringer)(selected);
                        highlighted_idx = None;
                        Popup::close_all(ui.ctx());
                        edit_response.surrender_focus();
                    }
                }
            }

            // Prevent arrow keys from moving the text cursor
            if arrow_pressed {
                edit_show.state.store(ui.ctx(), edit_response.id);
            }
        }

        // Clamp highlighted index to valid range when filter changes
        if let Some(idx) = highlighted_idx {
            if item_count == 0 {
                highlighted_idx = None;
            } else if idx >= item_count {
                highlighted_idx = Some(item_count - 1);
            }
        }

        if edit_response.lost_focus() {
            ui.input(|input| {
                if input.events.iter().any(|event| {
                    matches!(
                        event,
                        Event::Key {
                            key: Key::Enter,
                            pressed: true,
                            ..
                        }
                    )
                }) {
                    let pick_idx = highlighted_idx.unwrap_or(0);
                    if let Some(item) = items_for_display.get(pick_idx) {
                        *selected = item.clone();
                        *buf = (stringer)(selected);
                    }
                }
            });
            highlighted_idx = None;
        }

        let close_behavior = PopupCloseBehavior::CloseOnClick;

        if edit_response.gained_focus() {
            buf.clear();
            highlighted_idx = None;
        }

        let current_highlighted = highlighted_idx;
        set_highlighted(ui.ctx(), highlight_id, highlighted_idx);

        let _inner = Popup::menu(&edit_response)
            .id(popup_id)
            .width(edit_response.rect.width())
            .close_behavior(close_behavior)
            .show(|ui| {
                ui.set_min_width(ui.available_width());

                ScrollArea::vertical()
                    .max_height(f32::INFINITY)
                    .min_scrolled_height(400.0)
                    .show(ui, |ui| {
                        // Often the button is very narrow, which means this popup
                        // is also very narrow. Having wrapping on would therefore
                        // result in labels that wrap very early.
                        // Instead, we turn it off by default so that the labels
                        // expand the width of the menu.
                        ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);
                        for (i, item) in items_for_display.iter().enumerate() {
                            let item_text = stringer(&item);

                            if !item_text.to_lowercase().contains(&buf.to_lowercase()) {
                                continue;
                            }

                            let is_highlighted = current_highlighted == Some(i);
                            let item: V = item.clone();

                            let resp = ui.selectable_value::<V>(&mut selected, item, item_text);
                            if is_highlighted {
                                resp.scroll_to_me(None);
                                resp.highlight();
                            }
                        }
                    })
                    .inner
            });

        let is_popup_open = Popup::is_id_open(ui.ctx(), popup_id);
        let new_selected = selected.clone();
        if !(edit_response.has_focus() || is_popup_open) {
            *buf = (stringer)(&new_selected);
        } else if new_selected != old_selected {
            *buf = (stringer)(&new_selected);
            Popup::close_all(ui.ctx());
        }

        edit_response
    }
}
