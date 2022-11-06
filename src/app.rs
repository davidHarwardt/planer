use std::{cell::RefCell, sync::Mutex};

use chrono::Duration;
use eframe::{egui::{self, emath}, epaint::{vec2, pos2}};
use uuid::Uuid;

use crate::{drag_and_drop::drop_target, planer::{PlanerData, Exam, Teacher, Student, uuid_ref::UuidRef, Tag, Name}, modal::Modal, search::SearchData};

use super::drag_and_drop::drag_source;

pub struct PlanerApp {
    maximized: bool,
    tab: Tab,

    settings: Settings,
    data: PlanerData,
    person_tab: PersonTab,


    search_data: SearchData<SearchType>,
    dummy_string: String,
}


#[derive(Debug)]
struct AddExamData {
    id: String,
    duration: Duration,
    subjects: Vec<String>,
    tags: Vec<Tag>,
}

impl Default for AddExamData {
    fn default() -> Self {
        Self {
            id: String::new(),
            duration: Duration::minutes(30),
            subjects: Vec::new(),
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
struct SubjectModalData {
    name: String,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum SearchType {
    #[default]
    Normal,
    Name,
    Id,
    Subject,
    Tag,
}

impl std::fmt::Display for SearchType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SearchType::Normal => "normal",
            SearchType::Name => "@name",
            SearchType::Id => "$id",
            SearchType::Subject => ":subject",
            SearchType::Tag => "#tag",
        })
    }
}


impl PlanerApp {
    pub fn new(_cc: &eframe::CreationContext) -> Self {
        use SearchType::*;
        Self {
            maximized: false,
            tab: Tab::Calendar,

            settings: Settings::new(),
            data: PlanerData::default(),
            person_tab: PersonTab::Teachers,

            search_data: SearchData::new(&[
                ("@", Name),
                ("$", Id),
                (":", Subject),
                ("#", Tag),
            ]),
            dummy_string: "bio-2".to_string(),
        }
    }
}

const CLOSE_WINDOW_ICON: &str       = "🗙";
const MAXIMIZE_WINDOW_ICON: &str    = "🗖";
const MINIMIZE_WINDOW_ICON: &str    = "🗕";
const PIN_ICON: &str                = "📌";
const ADD_ICON: &str                = "➕";

#[derive(Eq, PartialEq)]
enum Tab {
    Calendar,
    Exams,
}

#[derive(Eq, PartialEq)]
enum PersonTab {
    Teachers,
    Students,
}

impl eframe::App for PlanerApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_status_bar").show(ctx, |ui| {

            egui::Frame::none().inner_margin(2.0).show(ui, |ui| {
                ui.columns(3, |col| {
                    egui::menu::bar(&mut col[0], |ui| {
                        ui.menu_button("file", |ui| {
                            if ui.button("save").clicked() {
                                let file = rfd::FileDialog::new()
                                    .add_filter("json", &["json"])
                                    .save_file();

                                if let Some(path) = file {
                                    self.data.save(path);
                                }
                            }
                            if ui.button("load").clicked() {
                                let file = rfd::FileDialog::new()
                                    .add_filter("json", &["json"])
                                    .pick_file();

                                if let Some(path) = file {
                                    self.data = PlanerData::load(path);
                                }
                            }

                            if ui.button("settings").clicked() { self.settings.visible = !self.settings.visible }
                        });
                    });

                    col[1].columns(2, |col| {
                        let tab = |ui: &mut egui::Ui, is_selected: bool, name: &str| -> egui::Response {
                            ui.add_sized(ui.available_size(), egui::SelectableLabel::new(is_selected, name))
                        };

                        if tab(&mut col[0], self.tab == Tab::Calendar, "calendar").clicked() { self.tab = Tab::Calendar }
                        if tab(&mut col[1], self.tab == Tab::Exams, "exams").clicked() { self.tab = Tab::Exams }
                    });

                    col[2].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(CLOSE_WINDOW_ICON).clicked() { frame.close() }
                        if ui.button(MAXIMIZE_WINDOW_ICON).clicked() { self.maximized = !self.maximized; frame.set_fullscreen(self.maximized) }
                    });
                });
            });


            {
                let res = ui.interact(ui.min_rect(), ui.id().with("title_bar_drag"), egui::Sense::click());
                
                if res.is_pointer_button_down_on() {
                    frame.drag_window();
                }
            }
        });

        self.settings.ui(ctx);

        match self.tab {
            Tab::Calendar => self.show_calendar_tab(ctx),
            Tab::Exams => self.show_exams_tab(ctx),
        }
    }
}

#[derive(Debug, Clone)]
struct DraggingExam(UuidRef<Mutex<Exam>>);

#[derive(Debug, Clone)]
struct DraggingTeacher(UuidRef<Mutex<Teacher>>);

#[derive(Debug, Clone)]
struct DraggingStudent(UuidRef<Mutex<Student>>);

impl PlanerApp {
    fn show_calendar_tab(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("exam_select_panel").resizable(true).show(ctx, |ui| {

            ui.add_space(5.0);
            self.search_data.show(ui);
            ui.separator();

            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {

                for (i, exam) in self.data.unfinished_exams.iter().enumerate() {
                    let uuid = { UuidRef::new(exam) };
                    drag_source(ui, ui.id().with((i, "exam_drag_calendar")), |ui| {
                        let mut exam = exam.lock().unwrap();

                        self.show_exam(ui, &mut exam, false, || {})
                    }, || DraggingExam(uuid));
                }

                ui.add_space(5.0);
                ui.vertical_centered_justified(|ui| {
                // ui.vertical_centered(|ui| {
                    if ui.button(egui::RichText::new(ADD_ICON).heading()).clicked() { self.tab = Tab::Exams }
                    // ui.label("end");
                });
                ui.add_space(5.0);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // <marker>
            let time_width = 50.0;
            let room_width = 200.0;
            let header_height = 100.0;
            let padding = 5.0;
            egui::ScrollArea::horizontal().show(ui, |ui| {
                // manualy set dims
                ui.set_width(room_width * (self.data.rooms.len() as f32));

                let mut delete_idx = None;
                for (i, room) in self.data.rooms.iter_mut().enumerate() {
                    let idx = i as f32;
                    let rect = egui::Rect::from_min_size(
                        ui.min_rect().left_top() + vec2(idx * (room_width + padding) + time_width, 0.0),
                        vec2(room_width, header_height));

                    let mut ui = ui.child_ui(rect, egui::Layout::left_to_right(egui::Align::TOP));

                    ui.group(|ui| {
                        ui.vertical_centered_justified(|ui| {
                            ui.add(egui::TextEdit::singleline(&mut room.number).font(egui::TextStyle::Heading));
                            // ui.add_sized((ui.min_rect(), 0.0), egui::TextEdit::singleline(&mut room.number).font(egui::TextStyle::Heading));

                            ui.add_space(2.5);

                            ui.horizontal_wrapped(|ui| {
                                room.tags.retain(|tag| {
                                    let res = ui.button(format!("{tag}"))
                                        .on_hover_text_at_pointer("click to edit, right-click to remove");

                                    !res.secondary_clicked()
                                });

                                if ui.button(ADD_ICON)
                                    .on_hover_text_at_pointer("click to add tag")
                                .clicked() {
                                    println!("add");
                                }
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::BOTTOM), |ui| {
                                if ui.button(CLOSE_WINDOW_ICON)
                                    .on_hover_text_at_pointer("click to delete")
                                .clicked() {
                                    delete_idx = Some(i);
                                }
                            });
                        });
                    });
                }

                if let Some(idx) = delete_idx { self.data.rooms.remove(idx); }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    // self.data.timetable.times
                });
            });
        });
    }

    fn show_exams_tab(&mut self, ctx: &egui::Context) {
        let min_width = 200.0;
        egui::SidePanel::right("participant_select_panel")
            .resizable(true)
            .max_width(ctx.available_rect().width() - (min_width + 20.0))
        .show(ctx, |ui| {
            ui.add_space(2.0);
            ui.columns(2, |col| {
                col[0].selectable_value(&mut self.person_tab, PersonTab::Teachers, "teachers");
                col[1].selectable_value(&mut self.person_tab, PersonTab::Students, "students");
            });
            ui.separator();

            match self.person_tab {
                PersonTab::Teachers => {
                    self.search_data.show(ui);
                    ui.separator();

                    egui::TopBottomPanel::bottom("add_teacher_panel").frame(egui::Frame::none()).show_inside(ui, |ui| {
                        egui::Frame::none().inner_margin(5.0).show(ui, |ui| {
                            ui.centered_and_justified(|ui| {
                                struct AddTeacherData {
                                    first_name: String,
                                    last_name: String,
                                    title: Option<String>,
                                    shorthand: Option<String>,
                                    subj_string: String,
                                }

                                let add_teacher_modal = Modal::new(ui.ctx(), ui.id().with("add_teacher_modal"), |v: AddTeacherData| {
                                    let subjects: Vec<String> = v.subj_string.replace("\n", ",").split(",")
                                        .map(|v| v.trim().to_owned())
                                        .filter(|v| !v.is_empty())
                                        .collect();

                                    self.data.add_teacher(
                                        v.first_name,
                                        v.last_name,
                                        v.title,
                                        v.shorthand,
                                        &subjects[..],
                                    );
                                });
                                add_teacher_modal.show(|ui, data| {
                                    ui.set_max_width(200.0);
                                    ui.columns(3, |col| {
                                        egui::TextEdit::singleline(&mut data.first_name).hint_text("first").show(&mut col[0]);
                                        
                                        let mut title = data.title.take().unwrap_or_default();
                                        egui::TextEdit::singleline(&mut title).hint_text("[title]").show(&mut col[1]);
                                        if !title.is_empty() { data.title = Some(title) }

                                        egui::TextEdit::singleline(&mut data.last_name).hint_text("last").show(&mut col[2]);
                                    });

                                    let mut shorthand = data.shorthand.take().unwrap_or_default();
                                    egui::TextEdit::singleline(&mut shorthand)
                                        .hint_text(format!("{} (shorthand)", &data.last_name[0..(data.last_name.len().min(2))]))
                                    .show(ui);
                                    if !shorthand.is_empty() { data.shorthand = Some(shorthand) }

                                    egui::TextEdit::multiline(&mut data.subj_string).hint_text("subjects (comma seperated)").show(ui);

                                    let can_submit = !data.first_name.is_empty() && !data.last_name.is_empty();
                                    add_teacher_modal.show_close_submit(ui, can_submit);
                                });

                                if ui.button(egui::RichText::new(ADD_ICON).heading()).on_hover_text_at_pointer("click to add teacher").clicked() {
                                    add_teacher_modal.open(AddTeacherData {
                                        first_name: String::new(),
                                        last_name: String::new(),
                                        subj_string: String::new(),
                                        title: None,
                                        shorthand: None,
                                    });
                                }
                            });
                        });
                    });

                    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                        let mut delete_idx = None;
                        for (i, teacher) in self.data.teachers.iter()
                        .filter(|v| {
                            let (s_str, s_type) = self.search_data.search();
                            let teacher = v.lock().unwrap();
                            match s_type {
                                SearchType::Normal | SearchType::Name => { format!("{}", teacher.name).to_uppercase().contains(&s_str.to_uppercase()) },
                                SearchType::Subject => { teacher.subjects.iter().find(|v| v.to_uppercase().contains(&s_str.to_uppercase())).is_some() },
                                _ => false,
                            }
                        }).enumerate() {
                            {
                                let dragging_teacher = DraggingTeacher(UuidRef::new(teacher));
                                let mut t = teacher.lock().unwrap();
                                let name = t.name.clone();
                                let mut set_name = None;
                                egui::Frame::default().fill(ui.style().noninteractive().bg_fill).show(ui, |ui| {
                                    ui.group(|ui| {
                                        let change_name_modal = Modal::new(ui.ctx(), ui.id().with((i, "change_name_modal")), |v: Name| set_name = Some(v));
                                        change_name_modal.show(|ui, data| {
                                            ui.set_max_width(200.0);
                                            ui.columns(3, |col| {
                                                egui::TextEdit::singleline(&mut data.first).hint_text("first").show(&mut col[0]);

                                                let mut title = data.title.take().unwrap_or_else(|| String::new());
                                                egui::TextEdit::singleline(&mut title).hint_text("[title]").show(&mut col[1]);
                                                if !title.is_empty() { data.title = Some(title) }

                                                egui::TextEdit::singleline(&mut data.last).hint_text("last").show(&mut col[2]);
                                            });

                                            let can_submit = !data.last.is_empty() && !data.first.is_empty();
                                            change_name_modal.show_close_submit(ui, can_submit);
                                        });

                                        ui.horizontal_top(|ui| {
                                            let drag_width = 37.5_f32.max(t.shorthand.len() as f32 * 12.5);
                                            if ui.add_sized(
                                                (ui.available_width() - drag_width - 10.0, 0.0),
                                                egui::Button::new(egui::RichText::new(format!("{}", t.name)).heading()))
                                                .on_hover_text_at_pointer("click to edit")
                                            .clicked() {
                                                change_name_modal.open(name);
                                            }
                                            
                                            drag_source(ui, ui.id().with((i, "teacher_card_drag")), |ui| {
                                                ui.add_sized(
                                                    (ui.available_width(), 0.0),
                                                    egui::Button::new(egui::RichText::new(format!("{}", t.shorthand)).heading()))
                                               .on_hover_text_at_pointer("drag to insert");
                                            }, || dragging_teacher);
                                        });
                                        ui.allocate_space(vec2(ui.available_width(), 0.0));
                                        // if ui.add_sized((ui.available_width(), 0.0), egui::Button::new(egui::RichText::new(format!("{}", t.name)).heading()))

                                        ui.add_sized((ui.available_width(), 0.0), egui::TextEdit::singleline(&mut t.shorthand).hint_text("shorthand"));

                                        ui.separator();
                                        ui.allocate_space(vec2(ui.available_width(), 0.0));
                                        ui.horizontal_wrapped(|ui| {
                                            t.subjects.retain(|v| {
                                                let res = ui.button(format!("{v}"))
                                                    .on_hover_text_at_pointer("click to edit, right-click to remove");

                                                if res.clicked() {
                                                    println!("edit");
                                                }

                                                !res.secondary_clicked()
                                            });

                                            struct SubjectName(String);
                                            let add_subject_modal = Modal::new(ui.ctx(), ui.id().with("add_subject_modal"), |v: SubjectName| {
                                                t.subjects.push(v.0);
                                            });
                                            add_subject_modal.show(|ui, data| {
                                                ui.set_max_width(200.0);
                                                ui.text_edit_singleline(&mut data.0);
                                                add_subject_modal.show_close_submit(ui, !data.0.is_empty());
                                            });

                                            if ui.button(ADD_ICON)
                                                .on_hover_text_at_pointer("click to add subject")
                                            .clicked() {
                                                add_subject_modal.open(SubjectName("".to_owned()));
                                            }
                                        });

                                        ui.columns(3, |col| {
                                            if col[2].add_sized(col[2].min_size(), egui::Button::new(CLOSE_WINDOW_ICON))
                                                .on_hover_text_at_pointer("click to remove")
                                            .clicked() {
                                                delete_idx = Some(i);
                                            }
                                        });
                                    });
                                });

                                if let Some(name) = set_name { t.name = name }
                            }
                        }

                        if let Some(idx) = delete_idx {
                            self.data.teachers.remove(idx);
                        }
                    });

                },
                PersonTab::Students => {
                    self.search_data.show(ui);
                    ui.separator();

                    egui::TopBottomPanel::bottom("add_student_panel").frame(egui::Frame::none()).show_inside(ui, |ui| {
                        egui::Frame::none().inner_margin(5.0).show(ui, |ui| {
                            ui.centered_and_justified(|ui| {
                                struct AddStudentData {
                                    first_name: String,
                                    last_name: String,
                                    title: Option<String>,
                                }

                                let add_student_modal = Modal::new(ui.ctx(), ui.id().with("add_student_modal"), |v: AddStudentData| {
                                    self.data.add_student(
                                        v.first_name,
                                        v.last_name,
                                        v.title,
                                    );
                                });
                                add_student_modal.show(|ui, data| {
                                    ui.set_max_width(200.0);
                                    ui.columns(3, |col| {
                                        egui::TextEdit::singleline(&mut data.first_name).hint_text("first").show(&mut col[0]);
                                        
                                        let mut title = data.title.take().unwrap_or_default();
                                        egui::TextEdit::singleline(&mut title).hint_text("[title]").show(&mut col[1]);
                                        if !title.is_empty() { data.title = Some(title) }

                                        egui::TextEdit::singleline(&mut data.last_name).hint_text("last").show(&mut col[2]);
                                    });

                                    let can_submit = !data.first_name.is_empty() && !data.last_name.is_empty();
                                    add_student_modal.show_close_submit(ui, can_submit);
                                });

                                if ui.button(egui::RichText::new(ADD_ICON).heading()).on_hover_text_at_pointer("click to add teacher").clicked() {
                                    add_student_modal.open(AddStudentData {
                                        first_name: String::new(),
                                        last_name: String::new(),
                                        title: None,
                                    });
                                }
                            });
                        });
                    });

                    {
                        let mut delete_idx = None;
                        for (i, student) in self.data.students.iter()
                        .filter(|v| {
                            let (s_str, s_type) = self.search_data.search();
                            let student = v.lock().unwrap();
                            match s_type {
                                SearchType::Normal | SearchType::Name => { format!("{}", student.name).to_uppercase().contains(&s_str.to_uppercase()) },
                                _ => false,
                            }
                        }).enumerate() {
                            {
                                let dragging_student = DraggingStudent(UuidRef::new(student));
                                let mut t = student.lock().unwrap();
                                let name = t.name.clone();
                                let mut set_name = None;
                                egui::Frame::default().fill(ui.style().noninteractive().bg_fill).show(ui, |ui| {
                                    ui.group(|ui| {
                                        let change_name_modal = Modal::new(ui.ctx(), ui.id().with((i, "change_name_modal")), |v: Name| set_name = Some(v));
                                        change_name_modal.show(|ui, data| {
                                            ui.set_max_width(200.0);
                                            ui.columns(3, |col| {
                                                egui::TextEdit::singleline(&mut data.first).hint_text("first").show(&mut col[0]);

                                                let mut title = data.title.take().unwrap_or_else(|| String::new());
                                                egui::TextEdit::singleline(&mut title).hint_text("[title]").show(&mut col[1]);
                                                if !title.is_empty() { data.title = Some(title) }

                                                egui::TextEdit::singleline(&mut data.last).hint_text("last").show(&mut col[2]);
                                            });

                                            let can_submit = !data.last.is_empty() && !data.first.is_empty();
                                            change_name_modal.show_close_submit(ui, can_submit);
                                        });

                                        ui.horizontal_top(|ui| {
                                            let drag_width = 37.5_f32;
                                            if ui.add_sized(
                                                (ui.available_width() - drag_width - 10.0, 0.0),
                                                egui::Button::new(egui::RichText::new(format!("{}", t.name)).heading()))
                                                .on_hover_text_at_pointer("click to edit")
                                            .clicked() {
                                                change_name_modal.open(name);
                                            }
                                            
                                            drag_source(ui, ui.id().with((i, "student_drag_card")), |ui| {
                                                ui.add_sized(
                                                    (ui.available_width(), 0.0),
                                                    egui::Button::new(egui::RichText::new("").heading()))
                                               .on_hover_text_at_pointer("drag to insert");
                                            }, || dragging_student);
                                        });
                                        ui.allocate_space(vec2(ui.available_width(), 0.0));
                                        // if ui.add_sized((ui.available_width(), 0.0), egui::Button::new(egui::RichText::new(format!("{}", t.name)).heading()))


                                        ui.separator();
                                        ui.allocate_space(vec2(ui.available_width(), 0.0));

                                        ui.columns(3, |col| {
                                            if col[2].add_sized(col[2].min_size(), egui::Button::new(CLOSE_WINDOW_ICON))
                                                .on_hover_text_at_pointer("click to remove")
                                            .clicked() {
                                                delete_idx = Some(i);
                                            }
                                        });
                                    });
                                });

                                if let Some(name) = set_name { t.name = name }
                            }
                        }

                        if let Some(idx) = delete_idx {
                            self.data.students.remove(idx);
                        }
                    }
                },
            }
        });

        egui::TopBottomPanel::bottom("exam_add_panel").show(ctx, |ui| {
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::Center), |ui| {
                // let modal = Modal::new(ui.ctx(), ui.id().with("exam_add_modal"), |v: AddExamData| {
                //     self.data.add_exam(v.id, v.duration, v.subjects, v.tags);
                // });

                // modal.show(|ui, data| {
                //     egui::TextEdit::singleline(&mut data.id)
                //         .hint_text("id / name")
                //     .show(ui);

                //     if ui.button("submit").clicked() { modal.submit() }
                //     if ui.button("close").clicked() { modal.close() }
                // });

                ui.add_space(5.0);
                if ui.add_sized((ui.available_width(), 0.0), egui::Button::new(egui::RichText::new(ADD_ICON).heading())).clicked() {
                // if ui.button(egui::RichText::new("+").heading()).clicked()
                    // modal.open(AddExamData::default());
                    self.data.add_exam("".to_string(), Duration::minutes(30), Vec::new(), Vec::new());
                }
                ui.add_space(2.0);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {

            if self.data.unfinished_exams.len() > 0 {
                let mut remove_exam = None;
                egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                    ui.set_min_size(vec2(min_width, 0.0));
                    let num_cols = ((ui.available_size().x / min_width) as usize).max(1);

                    for (i, exams) in self.data.unfinished_exams.chunks(num_cols).enumerate() {
                        ui.columns(num_cols, |col| {
                            for (j, (exam, ui)) in exams.iter().zip(col.iter_mut()).enumerate() {
                                let mut exam = exam.lock().unwrap();
                                self.show_exam(ui, &mut exam, true, || remove_exam = Some(i * num_cols + j));
                            }
                        });
                    }

                    // could also show finished exams in gray with button to remove them for
                    // editing
                });

                if let Some(idx) = remove_exam {
                    let mut i = 0;
                    self.data.unfinished_exams.retain(|_| {
                        let remove = i != idx;
                        i += 1;
                        remove
                    });
                }

            } else {
                ui.vertical_centered(|ui| {
                    ui.heading("add exams using the \"+\" button");
                });
            }
            
        });
    }

    fn show_exam(&self, ui: &mut egui::Ui, exam: &mut Exam, edit_view: bool, on_remove: impl FnOnce()) -> Option<egui::Response> {
        let res = egui::Frame::group(ui.style()).fill(ui.style().noninteractive().bg_fill).show(ui, |ui| {
            if edit_view {

                egui::TextEdit::singleline(&mut exam.id)
                    .hint_text("id")
                    .font(egui::TextStyle::Heading)
                .show(ui);

                ui.label(egui::RichText::new(format!("{}", exam.uuid)).weak().size(10.0)); // could use tooltip

                {
                    let mut minutes = exam.duration.num_minutes();
                    ui.horizontal(|ui| {
                        ui.label("duration: ");
                        ui.add(egui::DragValue::new(&mut minutes).speed(2.0).suffix("min"));
                    });
                    exam.duration = Duration::minutes(minutes);
                }

                ui.separator();

                ui.group(|ui| {
                    ui.weak("subjects");
                    ui.horizontal_wrapped(|ui| {
                        exam.subjects.retain(|v| {
                            let res = ui.button(format!("{v}")).on_hover_text_at_pointer("click to rename, right-click to remove");

                            if res.clicked() {
                                println!("rename");
                                true
                            } else if res.secondary_clicked() { false }
                              else { true }
                        });

                        let modal = Modal::new(ui.ctx(), ui.id().with("add_subject_modal"), |v: SubjectModalData| {
                            exam.subjects.push(v.name);
                        });
                        modal.show(|ui, data| {
                            ui.set_max_width(200.0);
                            egui::TextEdit::singleline(&mut data.name)
                                .hint_text("subject")
                            .show(ui);

                            let can_submit = !data.name.is_empty();
                            modal.show_close_submit(ui, can_submit);
                        });

                        if ui.button(ADD_ICON).on_hover_text_at_pointer("add subject").clicked() { modal.open(Default::default()) }
                    });
                });
                ui.group(|ui| {
                    ui.weak("examiners");
                    ui.columns(exam.examiners.len(), |col| {
                        for (examiner, ui) in exam.examiners.iter_mut().zip(col.iter_mut()) {
                            if let Some(v) = examiner {
                                if let Some(v) = v.get() {
                                    let v = v.lock().unwrap();
                                    ui.add_space(2.0);
                                    let res = ui.button(format!("{}", v.shorthand))
                                        .on_hover_text_at_pointer(format!("{}", v.name))
                                        .on_hover_text_at_pointer("click to jump to, right-click to remove");

                                    // todo: implement click to jump to

                                    if res.secondary_clicked() {
                                        *examiner = None;
                                    }
                                } else {
                                    ui.add_space(2.0);
                                    let res = ui.button(egui::RichText::new("<invalid>").color(egui::Color32::RED))
                                        .on_hover_text_at_pointer(format!("uuid \"{}\" is invalid", v.uuid()))
                                        .on_hover_text_at_pointer(format!("click to revalidate, right-click to remove"));
                                    // todo add click to revalidate func
                                    if res.secondary_clicked() { *examiner = None }
                                }

                            } else {
                                drop_target(ui, |ui| {
                                    // ui.allocate_space(ui.min_size());
                                    ui.add_sized(ui.min_size(), egui::Label::new(""));
                                }, |v: DraggingTeacher| {
                                    *examiner = Some(v.0);
                                });
                            }
                        }

                        // if ui.button("+").on_hover_text("add examiner").clicked() { exam.examiners.push() };
                    });
                });
                ui.group(|ui| {
                    let mut add_examiees = Vec::new();
                    ui.weak("examinees");
                    drop_target(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            exam.examinees.retain(|v| {
                                if let Some(v) = v.get() {
                                    let v = v.lock().unwrap();
                                    let res = ui.button(format!("{}", v.name)).on_hover_text_at_pointer("click to jump to, right-click to remove");

                                    // todo: implement click to jump to

                                    !res.secondary_clicked()
                                } else {
                                    let res = ui.button(egui::RichText::new("<invalid>").color(egui::Color32::RED))
                                        .on_hover_text_at_pointer(format!("uuid: \"{}\" is invalid", v.uuid()))
                                        .on_hover_text_at_pointer("click to revalidate, right-click to remove");

                                    // todo add click to revalidate fn

                                    !res.secondary_clicked()
                                }
                            });
                        });
                    }, |v: DraggingStudent| add_examiees.push(v.0));

                    exam.examinees.extend(add_examiees);
                });
                ui.group(|ui| {
                    ui.weak("tags");
                    ui.horizontal_wrapped(|ui| {
                        exam.tags.retain_mut(|v| {
                            let mut res = ui.selectable_label(v.required, format!("{}", v.name));

                            if v.required { res = res.on_hover_text_at_pointer("required") }

                            let res = res.on_hover_text_at_pointer("click to edit, right-click to remove")
                                         .on_hover_text_at_pointer("double-click to toggle required");

                            if res.double_clicked() { v.required = !v.required }

                            !res.secondary_clicked()
                        });

                        let modal = Modal::new(ui.ctx(), ui.id().with("add_tag_modal"), |v: Tag| { exam.tags.push(v) });
                        modal.show(|ui, data| {
                            ui.set_max_width(200.0);
                            egui::TextEdit::singleline(&mut data.name)
                                .hint_text("tag name")
                            .show(ui);

                            ui.checkbox(&mut data.required, "required")
                                .on_hover_text_at_pointer("if the tag is not required it is treated as a hint for the solver");

                            let can_submit = !data.name.is_empty();
                            modal.show_close_submit(ui, can_submit);
                        });

                        if ui.button(ADD_ICON).on_hover_text_at_pointer("add tag").clicked() { modal.open(Tag {
                            name: String::new(),
                            required: false,
                        }) }
                    });
                });

                ui.columns(3, |col| {
                    if col[2].add_sized(col[2].min_size(), egui::Button::new(CLOSE_WINDOW_ICON)).on_hover_text_at_pointer("delete exam").clicked() {
                        on_remove()
                    }
                });

                None
            } else {
                let title_res = ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.heading(if exam.id.is_empty() { "[unnamed]" } else { &exam.id[..] });
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(CLOSE_WINDOW_ICON).on_hover_text_at_pointer("remove item from the room").clicked() { on_remove() }
                            if ui.selectable_label(exam.pinned, PIN_ICON)
                                .on_hover_text_at_pointer("pin item")
                            .clicked() {
                                exam.pinned = !exam.pinned;
                            }
                    });
                });

                let res = ui.scope(|ui| {
                    ui.weak(format!("duration: {}min", exam.duration.num_minutes()));
                    ui.horizontal_wrapped(|ui| {
                        for subject in &exam.subjects {
                            ui.label(format!("{subject}"));
                        }

                        if exam.subjects.len() == 0 { ui.weak("<no subjects>"); }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.columns(exam.examiners.len(), |col| {
                            for (examiner, ui) in exam.examiners.iter().zip(col.iter_mut()) {
                                ui.centered_and_justified(|ui| {
                                    if let Some(v) = examiner {
                                        if let Some(v) = v.get() {
                                            let v = v.lock().unwrap();
                                            ui.label(format!("{}", v.shorthand)).on_hover_text_at_pointer(format!("{}", v.name));
                                        } else {
                                            ui.label(egui::RichText::new("<invalid>").color(egui::Color32::RED));
                                        }
                                    } else {
                                        ui.weak("―");
                                    }
                                });
                            }
                        });
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            for (i, examinee) in exam.examinees.iter().enumerate() {
                                let comma = if (i + 1) < exam.examinees.len() { ", " } else { "" };
                                if let Some(examinee) = examinee.get() {
                                    let examinee = examinee.lock().unwrap();
                                    ui.label(format!("{}{}", examinee.name, comma));
                                } else {
                                    ui.label(egui::RichText::new(format!("<invalid>{}", comma)).color(egui::Color32::RED));
                                }
                            }

                            if exam.examinees.is_empty() { ui.weak("<no examinees>"); }
                        });
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            for (i, tag) in exam.tags.iter().filter(|v| v.required).enumerate() {
                                let comma = if (i + 1) < exam.examinees.len() { ", " } else { "" };
                                ui.label(format!("{}{comma}", tag.name));
                            }
                            if exam.tags.is_empty() { ui.weak("<no tags>"); }
                        });

                    });

                }).response;
                Some(res)
            }
        }).inner;
        // println!("{res:?}");
        res
    }
}


struct Settings {
    visible: bool,
}

impl Settings {
    fn new() -> Self {
        Self {
            visible: false,
        }
    }

    fn ui(&mut self, ctx: &egui::Context) {
        egui::Window::new("settings")
            .open(&mut self.visible)
            .collapsible(false)
            .resizable(false)
            // .title_bar(false)
            .anchor(egui::Align2::CENTER_CENTER, (0.0, 0.0))
        .show(ctx, |ui| {
            // ui.heading("settings");

            // scale
            {
                let mut scale = ctx.pixels_per_point();
                egui::ComboBox::from_label("scale")
                    .selected_text(format!("{scale}x"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut scale, 0.25, "0.25x");
                    ui.selectable_value(&mut scale, 0.5, "0.5x");
                    ui.selectable_value(&mut scale, 1.0, "1x");
                    ui.selectable_value(&mut scale, 1.25, "1.25x");
                    ui.selectable_value(&mut scale, 1.5, "1.5x");
                    ui.selectable_value(&mut scale, 1.75, "1.75x");
                    ui.selectable_value(&mut scale, 2.0, "2x");
                });
                ctx.set_pixels_per_point(scale);
            }
            
            // dark / ligth
            ui.horizontal(|ui| {
                egui::widgets::global_dark_light_mode_buttons(ui);
            });

        });
    }
}

