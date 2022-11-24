use std::{sync::{Mutex, Arc}, cell::RefCell};

use eframe::egui;

pub struct Modal<'a, T> {
    data_type: std::marker::PhantomData<T>,
    ctx: egui::Context,
    on_submit: RefCell<Option<Box<dyn FnOnce(T) + 'a>>>,
    id: egui::Id,
    action: RefCell<Option<ModalAction>>,
}

enum ModalAction {
    Close,
    Submit,
}

pub struct ModalData<T> {
    data: Arc<Mutex<Option<T>>>,
}

impl<T> Clone for ModalData<T> {
    fn clone(&self) -> Self {
        Self { data: self.data.clone() }
    }
}

const OVERLAY_COLOR: egui::Color32 = egui::Color32::from_rgba_premultiplied(0, 0, 0, 200);

impl<'a, T: Send + 'static> Modal<'a, T> {
    pub fn new(ctx: &egui::Context, id: egui::Id, on_submit: impl FnOnce(T) + 'a) -> Self {
        Self {
            data_type: std::marker::PhantomData,
            ctx: ctx.clone(),
            on_submit: RefCell::new(Some(Box::new(on_submit))),
            action: RefCell::new(None),
            id,
        }
    }

    // pub fn show_if(
    //     ctx: &egui::Context,
    //     v: bool,
    //     id: egui::Id,
    //     on_submit: impl FnOnce(T) + 'a,
    //     view: impl FnOnce(&mut egui::Ui, &Self, &mut T),
    //     data: impl FnOnce() -> T,
    // ) {
    //     let modal = Self::new(ctx, id, on_submit);
    //     modal.show(|ui, data| {
    //         view(ui, &modal, data);
    //     });
    //     if v { modal.open(data()) }
    // }

    pub fn show_close_submit(&self, ui: &mut egui::Ui, can_submit: bool) {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            if ui.button("cancel").clicked() { self.close() }
            if ui.add_enabled(can_submit, egui::Button::new("submit")).clicked() { self.submit() }
        });
    }

    pub fn show(&self, body: impl FnOnce(&mut egui::Ui, &mut T)) {
        let data = self.ctx.data().get_temp::<ModalData<T>>(self.id);
        if let Some(data) = data {
            egui::Area::new(self.id)
                .interactable(true)
                .fixed_pos(egui::Pos2::ZERO)
            .show(&self.ctx, |ui| {
                let screen_rect = ui.ctx().input().screen_rect;
                let area_res = ui.allocate_response(screen_rect.size(), egui::Sense::click());

                if area_res.clicked() {
                    self.close();
                }

                ui.painter().rect_filled(screen_rect, egui::Rounding::none(), OVERLAY_COLOR);
            });

            let window = egui::Window::new("modal")
                .id(self.id.with("modal_window"))
                .title_bar(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0; 2])
                .resizable(false);

            let window_res = window.show(&self.ctx, |ui| {
                let mut data = data.data.lock().unwrap();
                let edit_data = data.as_mut().expect("could not get editable data from modal (maybe the modal was improperly closed)");
                body(ui, edit_data);
            });

            if let Some(res) = window_res {
                self.ctx.move_to_top(res.response.layer_id);
            }
        }

        if let Some(action) = &*self.action.borrow() {
            match action {
                ModalAction::Close => {
                    self.ctx.data().remove::<ModalData<T>>(self.id);
                },
                ModalAction::Submit => {
                    let data: ModalData<T> = self.ctx.data().get_temp(self.id).expect("the data of a modal could not be found");
                    let data = data.data.lock().unwrap().take().expect("the modal was already closed");

                    let func = self.on_submit.borrow_mut().take().expect("the submit function was already called (maybe the modal was improperly closed)");
                    func(data);

                    self.ctx.data().remove::<ModalData<T>>(self.id);
                },
            }
        }
    }

    pub fn close(&self) { *self.action.borrow_mut() = Some(ModalAction::Close) }
    pub fn submit(&self) { *self.action.borrow_mut() = Some(ModalAction::Submit) }

    pub fn open(&self, data: T) {
        self.ctx.data().insert_temp(self.id, ModalData {
            data: Arc::new(Mutex::new(Some(data))),
        });
    }
}

