use eframe::{egui::{self, epaint}, epaint::vec2};

#[derive(Clone, Copy)]
struct DraggingActive;

pub trait DragResponse: Sized {
    fn get_drag_response(self) -> Option<egui::Response>;
}

impl DragResponse for () {
    fn get_drag_response(self) -> Option<egui::Response> { None }
}
impl DragResponse for egui::Response {
    fn get_drag_response(self) -> Option<egui::Response> {
        Some(self)
    }
}

impl DragResponse for Option<egui::Response> {
    fn get_drag_response(self) -> Option<egui::Response> { self }
}

#[derive(Clone)]
struct DraggingData<T: Clone + Send + Sync> {
    data: T,
    id: egui::Id,
}

#[derive(Clone)]
struct WasDropped;

pub fn drag_source<T, R: DragResponse + std::fmt::Debug>(
    ui: &mut egui::Ui,
    id: egui::Id,
    body: impl FnOnce(&mut egui::Ui) -> R,
    data: impl FnOnce() -> T,
    drop_fn: impl FnOnce(),
)
where
    T: 'static + std::any::Any + Clone + Send + Sync,
{
    let is_being_dragged = ui.memory().is_being_dragged(id);

    if !is_being_dragged {
        let body_res = ui.scope(|ui| {
            let res = body(ui);
            res
        });
        
        let res = if let Some(res) = body_res.inner.get_drag_response() { res }
                  else { body_res.response };

        let res = ui.interact(res.rect, id, egui::Sense::drag());
        // ui.ctx().move_to_top(body_res.layer_id);

        if res.drag_started() {
            let data = data();
            let dragging_data = DraggingData {
                data, id,
            };

            ui.memory().data.insert_temp(egui::Id::null(), dragging_data);
            ui.memory().data.insert_temp(egui::Id::null(), DraggingActive);
            // println!("drag_start");
        }


        if !ui.memory().is_being_dragged(id) {
            let mut mem = ui.memory();
            if mem.data.get_temp::<WasDropped>(id).is_some() {
                mem.data.remove::<WasDropped>(id);
                drop_fn();
            }
        }

        if res.hovered() {
            ui.output().cursor_icon = egui::CursorIcon::Grab;
        }
    } else {
        ui.output().cursor_icon = egui::CursorIcon::Grabbing;

        let layer_id = egui::LayerId::new(egui::Order::Tooltip, id);
        let res = ui.with_layer_id(layer_id, body).response;


        if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
            let delta = pointer_pos - res.rect.center();
            ui.ctx().translate_layer(layer_id, delta);
        }
    }
}

pub fn drop_target<T, R>(
    ui: &mut egui::Ui,
    body: impl FnOnce(&mut egui::Ui) -> R,
    on_drop: impl FnOnce(T),
) -> egui::InnerResponse<R>
where
    T: 'static + std::any::Any + Clone + Send + Sync,
{
    let is_being_dragged = {
        let mut mem = ui.memory();
        mem.is_anything_being_dragged() && mem.data.get_temp::<DraggingActive>(egui::Id::null()).is_some()
    };

    let mut drop_data = if is_being_dragged {
        ui.memory().data.get_temp::<DraggingData<T>>(egui::Id::null())
    } else { None };
    let can_accept_drag = drop_data.is_some();

    let margin = egui::Vec2::splat(4.0);

    let outer_rect_bounds = ui.available_rect_before_wrap();
    let inner_rect = outer_rect_bounds.shrink2(margin);

    let bg_target = ui.painter().add(epaint::Shape::Noop);

    let mut content_ui = ui.child_ui(inner_rect, *ui.layout());
    let ret = body(&mut content_ui);
    let outer_rect = egui::Rect::from_min_max(outer_rect_bounds.min, content_ui.min_rect().max + margin);
    let (rect, res) = ui.allocate_at_least(outer_rect.size(), egui::Sense::hover());

    // ui.painter().debug_rect(res.rect, egui::Color32::DARK_RED, "drop-area");


    let style = if is_being_dragged && can_accept_drag && res.hovered() {
        ui.visuals().widgets.active
    } else {
        ui.visuals().widgets.inactive
    };

    // println!("{}", res.hovered());

    // println!("{can_accept_drag}, {is_being_dragged}, {}, {}", ui.input().pointer.any_released(), res.hovered());
    {
        let mut mem = ui.memory();
        let data = mem.data.get_temp::<DraggingActive>(egui::Id::null());
        if !mem.is_anything_being_dragged() && data.is_some()  {
            // println!("remove");
            mem.data.remove::<T>(egui::Id::null());
            mem.data.remove::<DraggingActive>(egui::Id::null());
        }
    }

    if is_being_dragged && can_accept_drag && ui.input().pointer.any_released() {
        // println!("{}", res.hovered());
        if res.hovered() {
            // println!("drop");
            let data = drop_data.take().unwrap();
            on_drop(data.data);

            // println!("remove");
            ui.memory().data.remove::<T>(egui::Id::null());
            ui.memory().data.insert_temp(data.id, WasDropped);
            ui.memory().data.remove::<DraggingActive>(egui::Id::null());
            // println!("drop");
        } else {
            // println!("no-drop");
        }
        // as of now multiple removals
        // println!("remove");
    }

    let mut fill = style.bg_fill;
    let mut stroke = style.bg_stroke;
    if is_being_dragged && !can_accept_drag {
        let window_fill = ui.visuals().window_fill();
        fill = epaint::color::tint_color_towards(fill, window_fill);
        stroke.color = epaint::color::tint_color_towards(stroke.color, window_fill);
    }

    ui.painter().set(bg_target, epaint::RectShape {
        rounding: style.rounding,
        fill,
        stroke,
        rect,
    });

    egui::InnerResponse::new(ret, res)
}

