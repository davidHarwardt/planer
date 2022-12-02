use std::sync::{Mutex, Arc};

use chrono::prelude::*;

use crate::planer::{Exam, Room, Timetable, TimetableLesson};


pub struct HardConstraint {
    pub func: Box<dyn Fn(&Exam, &(&Room, &TimetableLesson, &Date<Utc>)) -> Result<(), String>>,
}

pub struct SoftConstraint {
    pub func: Box<dyn Fn(&Exam, &(&Room, &TimetableLesson, &Date<Utc>)) -> i32>,
}

pub struct Constraints {
    pub hard: Vec<HardConstraint>,
    pub soft: Vec<SoftConstraint>,
}

macro_rules! constraint {
    (hard: $fn:tt) => {
        #[allow(unused_parens)]
        HardConstraint { func: Box::new($fn) }
    };
    (soft: $fn:tt) => {
        #[allow(unused_parens)]
        SoftConstraint { func: Box::new($fn) }
    };
}

impl Default for Constraints {
    fn default() -> Self {
        Constraints {
            hard: vec![
                // check if the room is already booked
                constraint!(hard: (|_exam, (room, lesson, day)| {
                    let time = day.and_time(lesson.start).unwrap();
                    // check in participants calendars
                    if room.calendar.is_booked_from_to(&time, lesson.duration) {
                        Err(format!("the room {} is already booked at {}", room.number, time))
                    } else { Ok(()) }
                })),

                constraint!(hard: (|exam, (room, _lesson, _day)| {
                    if exam.tags.iter()
                        .filter(|tag| tag.required)
                    .all(|tag| room.tags.contains(&tag.name)) {
                        Ok(())
                    } else {
                        Err(format!("some required tags are missing from the room"))
                    }
                })),
            ],

            soft: vec![
                // rank rooms with matching tags heigher
                constraint!(soft: (|exam, (room, _lesson, _day)| {
                    exam.tags.iter()
                        .filter_map(|tag| {
                            if room.tags.contains(&tag.name) {
                                Some(if tag.required { 2 } else { 1 })
                            } else { None }
                        })
                    .sum()
                })),
            ],
        }
    }
}

impl Constraints {
    fn apply_hard(&self, value: &Exam, candidate: &(&Room, &TimetableLesson, &Date<Utc>)) -> Result<(), String> {
        match self.hard.iter().find_map(|v| (v.func)(value, candidate).err()) {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    fn apply_soft(&self, value: &Exam, candidate: &(&Room, &TimetableLesson, &Date<Utc>)) -> i32 {
        self.soft.iter().fold(0, |acc, v| { acc + (v.func)(value, candidate) })
    }
}

struct Indexed<T>(usize, T);
struct Ranked<T>(i32, T);

pub struct SolveResult {
    pub finished_exams: Vec<Arc<Mutex<Exam>>>,
}

pub fn solve(
    values: &mut Vec<Arc<Mutex<Exam>>>,
    rooms: &mut [Room],
    timetable: &Timetable,
    day: Date<Utc>,
    mut mutator: impl FnMut(&Arc<Mutex<Exam>>, (&mut Room, &TimetableLesson, &Date<Utc>)),

    constraints: &Constraints,
) -> Result<SolveResult, SolveResult> {
    let mut finished_exams = Vec::new();

    while values.len() > 0 {
        let found = values.iter().enumerate()
            .flat_map(|(i, value)| {
                rooms.iter().enumerate().zip(std::iter::repeat(i)).flat_map(move |((j, room), i)| {
                    timetable.times.iter().zip(std::iter::repeat((i, j))).map(move |(t, (i, j))| Indexed(i, (value, (Indexed(j, room), t))))
                })
            })
            .filter(|Indexed(_, (exam, (Indexed(_, room), t)))| {
                constraints.apply_hard(&exam.lock().unwrap(), &(room, t, &day))
                    // .map_err(|err| println!("err: {err}"))
                    // .map(|_| println!("ok"))
                .is_ok()
            })
        .fold(Ranked(i32::MIN, Indexed(0, None)), |Ranked(acc_score, Indexed(i, item)), Indexed(j, (exam, (Indexed(k, room), t)))| {
            let combination = &(room, t, &day);
            let score = constraints.apply_soft(&exam.lock().unwrap(), combination);

            // println!("score: {score}, acc_score: {acc_score}");
            if score > acc_score {
                // println!("new ranked");
                Ranked(score, Indexed(j, Some((exam, Indexed(k, room), t))))
            } else { Ranked(acc_score, Indexed(i, item)) }
        });

        let Ranked(_score, Indexed(i, combination)) = found;
        if let Some((_exam, Indexed(j, _room), lesson)) = combination {
            let value = values.remove(i);
            let room = &mut rooms[j];
            mutator(&value, (room, lesson, &day));
            finished_exams.push(value);
        } else {
            return Err(SolveResult { finished_exams });
        }
    }

    Ok(SolveResult {
        finished_exams,
    })
}

