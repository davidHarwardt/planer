use crate::app::PlanerApp;

mod app;
mod drag_and_drop;
mod planer;
mod modal;
mod search;
mod solver;

fn main() {
    let native_options = eframe::NativeOptions {
        decorated: true,
        resizable: true,

        ..Default::default()
    };
    eframe::run_native("planer", native_options, Box::new(|cc| Box::new(PlanerApp::new(cc))));
}

