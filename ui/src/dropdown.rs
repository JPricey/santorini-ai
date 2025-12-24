use eframe::egui::{
    EventFilter, Id, Key, Popup, PopupCloseBehavior, ScrollArea, TextEdit, TextWrapMode, Widget,
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

        let edit = TextEdit::singleline(buf)
            .hint_text(hint_text)
            .desired_width(100.0);
        let edit_show = edit.show(ui);
        let edit_response = edit_show.response;

        let event_filter = EventFilter {
            horizontal_arrows: true,
            vertical_arrows: true,
            ..Default::default()
        };

        let mut events = ui.input(|i| i.filtered_events(&event_filter));
        let items_for_display: Vec<_> = items
            .filter(|item| {
                let item_text = stringer(&item);
                item_text.to_lowercase().contains(&buf.to_lowercase())
            })
            .collect();

        if Popup::is_id_open(ui.ctx(), popup_id) {
            for event in events.drain(..) {
                match event {
                    eframe::egui::Event::Key {
                        key, pressed: true, ..
                    } => {
                        if key == Key::Enter {
                            if items_for_display.len() == 1 {
                                let item = items_for_display[0].clone();
                                *selected = item;
                                *buf = (stringer)(&selected);

                                #[allow(deprecated)]
                                ui.memory_mut(|m| m.close_popup(popup_id));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let close_behavior = PopupCloseBehavior::CloseOnClick;

        if edit_response.gained_focus() {
            buf.clear();
        }

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
                        for item in items_for_display {
                            let item_text = stringer(&item);

                            if !item_text.to_lowercase().contains(&buf.to_lowercase()) {
                                continue;
                            }

                            let item: V = item;
                            ui.selectable_value::<V>(&mut selected, item, item_text);
                        }
                    })
                    .inner
            });

        let is_popup_open = Popup::is_id_open(ui.ctx(), popup_id);
        let new_selected = selected.clone();
        if !(edit_response.has_focus() || is_popup_open) {
            *buf = (stringer)(&new_selected);
        }

        edit_response
    }
}
