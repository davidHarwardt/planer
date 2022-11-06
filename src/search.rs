use eframe::egui;

pub enum SearchKind {
    Normal,
    Name,
    Tag,
}

pub struct SearchData<T> {
    search_string: String,
    search_type: T,
    types: Vec<(String, T)>,
}

const TEXT_CLOSE_ICON: &str = "🗙";

impl<T: PartialEq + Default + Copy + std::fmt::Display> SearchData<T> {
    pub fn new(types: &[(&str, T)]) -> Self {
        Self {
            search_string: String::new(),
            search_type: T::default(),
            types: types.iter().map(|(s, t)| ((*s).to_owned(), *t)).collect(),
        }
    }

    fn get_search_type(&self) -> (&str, T) {
        self.types.iter().find_map(|v| if self.search_string.starts_with(&v.0) { Some((&v.0[..], v.1)) } else { None }).unwrap_or(("", T::default()))
    }

    pub fn search(&self) -> (&str, T) {
        let (prefix, s_type) = self.get_search_type();
        (&self.search_string[prefix.len()..], s_type)
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let padding = 10.0;
        let btn_width = 20.0;
        ui.horizontal_top(|ui| {
            ui.add_sized((ui.available_width() - (padding + btn_width), 0.0), egui::TextEdit::singleline(&mut self.search_string));
            if ui.add_sized((btn_width, 0.0), egui::Button::new(TEXT_CLOSE_ICON)).clicked() { self.search_string.clear() }
        });

        let col_width = 75.0;
        let n_cols = ((ui.available_width() / col_width).ceil() as usize).min(self.types.len());

        let (s_str, s_type) = self.search();
        let mut res_str = None;
        for line in self.types.chunks(n_cols) {
            ui.columns(n_cols, |col| {
                for ((item_prefix, item_type), ui) in line.iter().zip(col.iter_mut()) {
                    if ui.selectable_label(*item_type == s_type, format!("{item_type}")).clicked() {
                        res_str = Some(format!("{item_prefix}{}", s_str));
                    }
                }
            });
        }

        if let Some(res_str) = res_str { self.search_string = res_str }
    }
}


