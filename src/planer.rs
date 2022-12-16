pub mod calendar;
pub mod uuid_ref;

use std::{sync::{Mutex, Arc}, path::Path, cell::RefCell};

use chrono::{prelude::*, Duration};
use serde_with::{serde_as, DurationSeconds};
use uuid::Uuid;

use crate::solver::{Constraints, solve};

use self::{calendar::{Calendar, Event}, uuid_ref::{UuidRef, AsUuid}};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct PlanerData {
    pub students: Vec<Arc<Mutex<Student>>>,
    pub teachers: Vec<Arc<Mutex<Teacher>>>,

    pub unfinished_exams: Vec<Arc<Mutex<Exam>>>,
    pub finished_exams: Vec<Arc<Mutex<Exam>>>,

    pub rooms: Vec<Arc<Mutex<Room>>>,
    pub timetable: Timetable,

    #[serde(skip)]
    pub constraints: Constraints,

    #[serde(skip)]
    pub current_file_name: Option<String>,

    #[serde(skip)]
    needs_recompute: RefCell<bool>,
}

impl PlanerData {
    pub fn save(&mut self) {
        if let Some(file) = &self.current_file_name {
            let data = serde_json::to_string(self).expect("could not serialize data");
            std::fs::write(file, data).expect("could not write file");
        } else {
            self.save_as();
        }
    }

    pub fn save_as(&mut self) {
        let file = rfd::FileDialog::new()
            .add_filter("plans", &["plan"])
            .add_filter("planer templates", &["ptemplate"])
            .save_file();
        if let Some(path) = file {
            self.current_file_name = Some(path.to_str().unwrap().to_owned());
            self.save();
        }
    }

    pub fn load(path: impl AsRef<Path>) -> Self {
        let file_name = path.as_ref().to_str().unwrap().to_owned();
        let mut data = Self::load_template(path);
        data.current_file_name = Some(file_name);

        data
    }

    pub fn load_template(path: impl AsRef<Path>) -> Self {
        let file = std::fs::read_to_string(path).expect("could not open file");
        let mut data: PlanerData = serde_json::from_str(&file[..]).expect("could not deserialize data");
        data.revalidate();
        data.compute_conflicts();

        data
    }

    pub fn revalidate(&mut self) {
        for exam in &mut self.unfinished_exams {
            exam.lock().unwrap().revalidate(&self.students, &self.teachers);
        }

        for exam in &mut self.finished_exams {
            exam.lock().unwrap().revalidate(&self.students, &self.teachers);
        }

        for student in &mut self.students {
            student.lock().unwrap().revalidate(&self.finished_exams);
        }

        for teacher in &mut self.teachers {
            teacher.lock().unwrap().revalidate(&self.finished_exams);
        }

        for room in &mut self.rooms {
            room.lock().unwrap().revalidate(&self.finished_exams);
        }
    }

    pub fn add_student(&mut self, first: String, last: String, title: Option<String>) {
        self.students.push(Arc::new(Mutex::new(Student {
            name: Name { uuid: Uuid::new_v4(), first, last, title },
            calendar: Calendar::new(),
        })));
    }

    pub fn solve(&mut self) {
        let res = solve(
            &mut self.unfinished_exams,
            &mut self.rooms[..],
            &self.timetable,
            Utc::today(),
            |exam, (room, lesson, day)| {
                let room_ref = Arc::clone(room);
                Self::book_exam(UuidRef::new(exam), &room_ref, day.and_time(lesson.start).unwrap());
            },
            &self.constraints,
        );

        match res {
            Ok(mut v) => {
                self.finished_exams.append(&mut v.finished_exams);
            },
            Err(mut v) => {
                self.finished_exams.append(&mut v.finished_exams);
                println!("could not match all exams");
            },
        }

        self.compute_conflicts();
    }

    pub fn compute_conflicts(&mut self) {
        for exam in &self.finished_exams {
            let mut exam = exam.lock().unwrap();

            if let Some((room_ref, time)) = exam.pairing.as_ref() {
                let room_res = room_ref.get().unwrap();
                let room = room_res.lock().unwrap();
                let combination = (&*room, time);

                exam.error = self.constraints.apply_hard(&exam, &combination, true).err();
            } else {
                println!("no pairing: {exam:?}");
            }
        }
    }

    pub fn schedule_recompute(&self) {
        *self.needs_recompute.borrow_mut() = true;
    }

    pub fn recompute_if_scheduled(&mut self) {
        if *self.needs_recompute.borrow() {
            self.compute_conflicts();
            *self.needs_recompute.borrow_mut() = false;
        }
    }

    pub fn add_teacher(&mut self, first: String, last: String, title: Option<String>, shorthand: Option<String>, subjects: &[String]) {
        let shorthand = shorthand.unwrap_or((&last[0..(last.len().min(2))]).to_owned());
        self.teachers.push(Arc::new(Mutex::new(Teacher {
            name: Name { uuid: Uuid::new_v4(), first, last, title },
            shorthand,
            calendar: Calendar::new(),
            subjects: subjects.to_vec()
        })));
    }

    pub fn book_exam(exam_ref: UuidRef<Mutex<Exam>>, room: &Arc<Mutex<Room>>, start_time: DateTime<Utc>) {
        let room_ref = UuidRef::new(room);
        if let Some(exam) = exam_ref.get() {
            let mut exam = exam.lock().unwrap();
            let mut room = room.lock().unwrap();

            let ev = Event::new(start_time, exam.duration, exam_ref.clone());

            for student in &mut exam.examinees {
                student.get().map(|v| {
                    v.lock().unwrap().calendar.add_event(ev.clone());
                });
            }
            
            for teacher in &mut exam.examiners {
                teacher.as_ref().map(|v| v.get().map(|v| {
                    v.lock().unwrap().calendar.add_event(ev.clone());
                }));
            }

            room.calendar.add_event(ev);

            exam.pairing = Some((room_ref, start_time));
        }
    }

    pub fn unbook_exam(exam_ref: UuidRef<Mutex<Exam>>, room: &mut Room, start_time: DateTime<Utc>) {
        if let Some(exam) = exam_ref.get() {
            let mut exam = exam.lock().unwrap();

            let ev = Event::new(start_time, exam.duration, exam_ref);

            for student in &mut exam.examinees {
                student.get().map(|v| {
                    v.lock().unwrap().calendar.remove_event(&ev);
                });
            }
            
            for teacher in &mut exam.examiners {
                teacher.as_ref().map(|v| v.get().map(|v| {
                    v.lock().unwrap().calendar.remove_event(&ev);
                }));
            }

            room.calendar.remove_event(&ev);
        }
    }

    pub fn unfinish_exam(&mut self, exam: UuidRef<Mutex<Exam>>) {
        let idx = self.finished_exams.iter().position(|v| v.lock().unwrap().uuid == exam.uuid());
        idx.map(|idx| {
            let ex = self.finished_exams.remove(idx);
            self.unfinished_exams.push(ex);
        });
    }

    pub fn finish_exam(&mut self, exam: UuidRef<Mutex<Exam>>) {
        let idx = self.unfinished_exams.iter().position(|v| v.lock().unwrap().uuid == exam.uuid());
        idx.map(|idx| {
            let ex = self.unfinished_exams.remove(idx);
            self.finished_exams.push(ex);
        });
    }

    pub fn add_exam(&mut self, id: String, duration: Duration, subjects: Vec<String>, tags: Vec<Tag>) {
        self.unfinished_exams.push(Arc::new(Mutex::new(Exam {
            duration, id, subjects, tags,
            uuid: Uuid::new_v4(),
            examinees: Vec::new(),
            pinned: false,
            examiners: [None, None, None],
            pairing: None,
            error: None,
        })));
    }

    pub fn add_room(&mut self, number: String, tags: Vec<String>) {
        self.rooms.push(Arc::new(Mutex::new(Room {
            number, tags,
            calendar: Calendar::new(),
            uuid: Uuid::new_v4(),
        })));
    }
}


impl Default for PlanerData {
    fn default() -> Self {
        let v = Self {
            students: Vec::new(),
            teachers: Vec::new(),

            unfinished_exams: Vec::new(),
            finished_exams: Vec::new(),
            rooms: Vec::new(),
            timetable: Timetable::default(),

            constraints: Constraints::default(),
            current_file_name: None,
            needs_recompute: RefCell::new(false),
        };
        
        v
    }
}

// small components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Name {
    uuid: Uuid,
    pub first: String,
    pub last: String,
    pub title: Option<String>,
}

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}{} {}", self.first, self.title.as_ref().map(|v| format!(" {v}")).unwrap_or(String::new()), self.last))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub required: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum LessonType {
    Lesson,
    Break,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Timetable {
    pub times: Vec<TimetableLesson>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct TimetableLesson {
    pub start: NaiveTime,
    #[serde_as(as = "DurationSeconds<i64>")]
    pub duration: Duration,
    pub lesson_type: LessonType,
}

impl Default for Timetable {
    fn default() -> Self {
        use LessonType::*;
        let times = [
            (NaiveTime::from_hms(08, 00, 00), Duration::minutes(45), Lesson),
            (NaiveTime::from_hms(08, 50, 00), Duration::minutes(45), Lesson),
            (NaiveTime::from_hms(09, 40, 00), Duration::minutes(45), Lesson),
            // 20 min break
            (NaiveTime::from_hms(10, 40, 00), Duration::minutes(45), Lesson),
            (NaiveTime::from_hms(11, 30, 00), Duration::minutes(45), Lesson),

            (NaiveTime::from_hms(12, 15, 00), Duration::minutes(45), Break), // 45 min break

            (NaiveTime::from_hms(13, 05, 00), Duration::minutes(45), Lesson),
            (NaiveTime::from_hms(13, 55, 00), Duration::minutes(45), Lesson),
            (NaiveTime::from_hms(14, 45, 00), Duration::minutes(45), Lesson),
            (NaiveTime::from_hms(15, 30, 00), Duration::minutes(45), Lesson),
            (NaiveTime::from_hms(16, 15, 00), Duration::minutes(45), Lesson),
        ].into_iter().map(|(start, duration, lesson_type)| TimetableLesson { start, duration, lesson_type }).collect();

        Self { times }
    }
}

// facilities
#[derive(Debug, Serialize, Deserialize)]
pub struct Room {
    uuid: Uuid,
    pub calendar: Calendar<UuidRef<Mutex<Exam>>>,
    pub number: String,
    pub tags: Vec<String>,
}
impl AsUuid for Room { fn as_uuid(&self) -> Uuid { self.uuid } }

impl Room {
    fn revalidate(&mut self, exams: &[Arc<Mutex<Exam>>]) {
        self.calendar.revalidate(exams)
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct Exam {
    #[serde_as(as = "DurationSeconds<i64>")]
    pub duration: Duration,
    pub uuid: Uuid,
    pub id: String,
    pub pinned: bool,

    pub examinees: Vec<UuidRef<Mutex<Student>>>,
    pub examiners: [Option<UuidRef<Mutex<Teacher>>>; 3],

    pub subjects: Vec<String>,
    pub tags: Vec<Tag>,

    pub pairing: Option<(UuidRef<Mutex<Room>>, DateTime<Utc>)>,

    #[serde(skip)]
    pub error: Option<String>,
}
impl AsUuid for Exam { fn as_uuid(&self) -> Uuid { self.uuid } }

impl Exam {
    fn revalidate(&mut self, students: &[Arc<Mutex<Student>>], teachers: &[Arc<Mutex<Teacher>>]) {
        for student in &mut self.examinees {
            student.revalidate(students);
        }

        for teacher in &mut self.examiners {
            teacher.as_mut().map(|v| v.revalidate(teachers));
        }
    }
}


// people
#[derive(Debug, Serialize, Deserialize)]
pub struct Student {
    pub name: Name,
    pub calendar: Calendar<UuidRef<Mutex<Exam>>>,
}
impl AsUuid for Student { fn as_uuid(&self) -> Uuid { self.name.uuid } }

impl Student {
    fn revalidate(&mut self, data: &[Arc<Mutex<Exam>>]) {
        self.calendar.revalidate(data);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Teacher {
    pub name: Name,
    pub shorthand: String,
    pub calendar: Calendar<UuidRef<Mutex<Exam>>>,
    pub subjects: Vec<String>,
}
impl AsUuid for Teacher { fn as_uuid(&self) -> Uuid { self.name.uuid } }

impl Teacher {
    fn revalidate(&mut self, data: &[Arc<Mutex<Exam>>]) {
        self.calendar.revalidate(data);
    }
}

