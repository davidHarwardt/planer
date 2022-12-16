use std::sync::{Mutex, Arc};

use chrono::prelude::*;

use crate::planer::{Exam, Room, Timetable, TimetableLesson};


pub struct HardConstraint {
    pub func: Box<dyn Fn(&Exam, &(&Room, &DateTime<Utc>), bool) -> Result<(), String>>,
}

pub struct SoftConstraint {
    pub func: Box<dyn Fn(&Exam, &(&Room, &DateTime<Utc>)) -> i32>,
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
                constraint!(hard: (|exam, (room, start), is_check| {
                    // check in participants calendars
                    if is_check { return Ok(()) }
                    if room.calendar.is_booked_from_to(&start, exam.duration) {
                        Err(format!("the room {} is already booked at {}", room.number, start))
                    } else { Ok(()) }
                })),

                constraint!(hard: (|exam, (room, _start), _is_check| {
                    let missing: Vec<_> = exam.tags.iter()
                        .filter_map(|tag| if tag.required && !room.tags.contains(&tag.name) {
                            Some(format!("\n - {}", tag.name.clone()))
                        } else { None })
                    .collect();

                    if missing.len() == 0 {
                        Ok(())
                    } else {
                        let missing = missing.join("");
                        Err(format!("the following required tags are missing from the room:{missing}"))
                    }
                })),

                constraint!(hard: (|exam, (_room, start), _is_check| {
                    let duration = exam.duration;
                    let booked: Vec<_> = exam.examiners.iter()
                        .filter_map(|examiner| {
                            if let Some(examiner) = examiner {
                                if let Some(examiner) = examiner.get() {
                                    let examinerp = examiner.lock().unwrap();
                                    let bookings = examinerp.calendar.get_booked_from_to(start, duration);
                                    // if bookings.len() == 0 { return None }
                                    // if bookings.len() == 1 && bookings[0].data.uuid() == exam.uuid { return None }

                                    let bookings_string = bookings.iter()
                                        .filter_map(|b| {
                                            if b.data.uuid() == exam.uuid { None }
                                            else { Some(b.data.get().map(|v| {
                                                let v = v.lock().unwrap();
                                                format!("{}", v.id)
                                            })) }
                                        })
                                        .filter_map(|v| v)
                                    .collect::<Vec<_>>();

                                    drop(examinerp);
                                    if bookings_string.len() != 0 {
                                        Some(format!("\n{}: {}", examiner.lock().unwrap().name, bookings_string.join(", ")))
                                    } else { None }
                                } else { None }
                            } else { None }
                        })
                    .collect();

                    if booked.len() != 0 {
                        Err(format!("the following people are already booked:{}", booked.join("")))
                    } else { Ok(()) }
                })),
            ],

            soft: vec![
                // rank rooms with matching tags heigher
                constraint!(soft: (|exam, (room, _start)| {
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
    pub fn apply_hard(&self, value: &Exam, candidate: &(&Room, &DateTime<Utc>), is_check: bool) -> Result<(), String> {
        match self.hard.iter().find_map(|v| (v.func)(value, candidate, is_check).err()) {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    pub fn apply_soft(&self, value: &Exam, candidate: &(&Room, &DateTime<Utc>)) -> i32 {
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
    rooms: &mut [Arc<Mutex<Room>>],
    timetable: &Timetable,
    day: Date<Utc>,
    mut mutator: impl FnMut(&Arc<Mutex<Exam>>, (&Arc<Mutex<Room>>, &TimetableLesson, &Date<Utc>)),

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
                let room = room.lock().unwrap();
                constraints.apply_hard(&exam.lock().unwrap(), &(&*room, &day.and_time(t.start).unwrap()), false)
                    // .map_err(|err| println!("err: {err}"))
                    // .map(|_| println!("ok"))
                .is_ok()
            })
        .fold(Ranked(i32::MIN, Indexed(0, None)), |Ranked(acc_score, Indexed(i, item)), Indexed(j, (exam, (Indexed(k, room), t)))| {
            let combination = &(&*room.lock().unwrap(), &day.and_time(t.start).unwrap());
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

