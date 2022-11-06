pub mod calendar;
pub mod uuid_ref;

use std::{sync::{Mutex, Arc}, path::Path};

use chrono::{prelude::*, Duration};
use serde_with::{serde_as, DurationSeconds};
use uuid::Uuid;

use self::{calendar::Calendar, uuid_ref::{UuidRef, AsUuid}};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct PlanerData {
    pub students: Vec<Arc<Mutex<Student>>>,
    pub teachers: Vec<Arc<Mutex<Teacher>>>,

    pub unfinished_exams: Vec<Arc<Mutex<Exam>>>,
    pub finished_exams: Vec<Arc<Mutex<Exam>>>,

    pub rooms: Vec<Room>,
    pub timetable: Timetable,
}

impl PlanerData {
    pub fn save(&self, path: impl AsRef<Path>) {
        let data = serde_json::to_string(self).expect("could not serialize data");
        std::fs::write(path, data).expect("could not write file");
    }

    pub fn load(path: impl AsRef<Path>) -> Self {
        let file = std::fs::read_to_string(path).expect("could not open file");
        let mut data: PlanerData = serde_json::from_str(&file[..]).expect("could not deserialize data");
        data.revalidate();
        data
    }

    pub fn revalidate(&mut self) {
        for exam in self.unfinished_exams.iter_mut() {
            exam.lock().unwrap().revalidate(&self.students, &self.teachers);
        }

        for exam in self.finished_exams.iter_mut() {
            exam.lock().unwrap().revalidate(&self.students, &self.teachers);
        }
    }

    pub fn add_student(&mut self, first: String, last: String, title: Option<String>) {
        self.students.push(Arc::new(Mutex::new(Student {
            name: Name { uuid: Uuid::new_v4(), first, last, title },
            calendar: Calendar::new(),
        })));
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

    pub fn add_exam(&mut self, id: String, duration: Duration, subjects: Vec<String>, tags: Vec<Tag>) {
        self.unfinished_exams.push(Arc::new(Mutex::new(Exam {
            duration, id, subjects, tags,
            uuid: Uuid::new_v4(),
            examinees: Vec::new(),
            pinned: false,
            examiners: [None, None, None],
        })));
    }

    pub fn add_room(&mut self, number: String, tags: Vec<String>) {
        self.rooms.push(Room {
            number, tags,
            calendar: Calendar::new(),
            uuid: Uuid::new_v4(),
        });
    }
}


impl Default for PlanerData {
    fn default() -> Self {
        let mut v = Self {
            students: Vec::new(),
            teachers: Vec::new(),

            unfinished_exams: Vec::new(),
            finished_exams: Vec::new(),
            rooms: Vec::new(),
            timetable: Timetable::default(),
        };
        
        v.add_teacher(format!("test"), format!("asdf"), None, None, &[format!("IT")]);
        v.add_room("101".to_string(), vec!["smartboard".to_string()]);
        v.add_room("104".to_string(), vec!["smartboard".to_string()]);
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
    start: NaiveTime,
    #[serde_as(as = "DurationSeconds<i64>")]
    duration: Duration,
    lesson_type: LessonType,
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
    calendar: Calendar<UuidRef<Mutex<Exam>>>,
    pub number: String,
    pub tags: Vec<String>,
}
impl AsUuid for Room { fn as_uuid(&self) -> Uuid { self.uuid } }

impl Room {
    fn revalidate(&mut self, exams: &[Arc<Mutex<Exam>>]) {
        // self.calendar.revalidate(exams)
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Teacher {
    pub name: Name,
    pub shorthand: String,
    pub calendar: Calendar<UuidRef<Mutex<Exam>>>,
    pub subjects: Vec<String>,
}
impl AsUuid for Teacher { fn as_uuid(&self) -> Uuid { self.name.uuid } }


