use chrono::{prelude::*, Duration, serde::ts_seconds}; 
use serde::{Serialize, Deserialize};
use serde_with::{serde_as, DurationSeconds};

use super::uuid_ref::AsUuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Calendar<E> {
    events: Vec<Event<E>>,
}

impl<E> Calendar<E> {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
        }
    }

    pub fn add_event(&mut self, ev: Event<E>) {
        self.events.push(ev);
    }

    pub fn is_booked_at(&self, time: &DateTime<Utc>) -> bool {
        self.events.iter().find(|v| v.includes_time(time)).is_some()
    }

    pub fn get_events_at(&self, time: &DateTime<Utc>) -> Vec<&Event<E>> {
        self.events.iter().filter(|v| v.includes_time(time)).collect()
    }

    pub fn get_events_at_mut(&mut self, time: &DateTime<Utc>) -> Vec<&mut Event<E>> {
        self.events.iter_mut().filter(|v| v.includes_time(time)).collect()
    }


    pub fn is_booked_from_to(&self, time: &DateTime<Utc>, duration: Duration) -> bool {
        self.events.iter().find(|v| v.includes(time, duration)).is_some()
    }

    pub fn get_booked_from_to(&self, time: &DateTime<Utc>, duration: Duration) -> Vec<&Event<E>> {
        self.events.iter().filter(|v| v.includes(time, duration)).collect()
    }

    pub fn get_booked_from_to_mut(&mut self, time: &DateTime<Utc>, duration: Duration) -> Vec<&mut Event<E>> {
        self.events.iter_mut().filter(|v| v.includes(time, duration)).collect()
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct Event<T> {
    #[serde(with = "ts_seconds")]
    pub start: DateTime<Utc>,
    #[serde_as(as = "DurationSeconds<i64>")]
    pub duration: Duration,
    pub data: T,
}

impl<T> Event<T> {
    pub fn includes_time(&self, time: &DateTime<Utc>) -> bool {
        &self.start <= time && &(self.start.clone() + self.duration.clone()) >= time
    }
    
    pub fn includes(&self, start: &DateTime<Utc>, duration: Duration) -> bool {
        start <= &(self.start.clone() + self.duration) && &(start.clone() + duration) >= &self.start
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_includes_time() {
        let event = Event {
            start: Utc.ymd(2022, Month::July.number_from_month(), 2).and_hms(08, 00, 00),
            duration: Duration::hours(1),
            data: (),
        };

        let time_1 = Utc.ymd(2022, Month::July.number_from_month(), 2).and_hms(08, 30, 00);
        let time_2 = Utc.ymd(2022, Month::July.number_from_month(), 2).and_hms(09, 00, 00);
        let time_3 = Utc.ymd(2022, Month::July.number_from_month(), 2).and_hms(08, 00, 00);
        let time_4 = Utc.ymd(2022, Month::July.number_from_month(), 2).and_hms(10, 00, 00);
        let time_5 = Utc.ymd(2022, Month::July.number_from_month(), 2).and_hms(06, 00, 00);

        assert!( event.includes_time(&time_1), "the event should include {time_1:?}");
        assert!( event.includes_time(&time_2), "the event should include {time_2:?} (inclusive at the end)");
        assert!( event.includes_time(&time_3), "the event should include {time_3:?} (inclusive at the start)");
        assert!(!event.includes_time(&time_4), "the event should not include {time_4:?} (after end)");
        assert!(!event.includes_time(&time_5), "the event should not include {time_5:?} (before start)");
    }

    #[test]
    fn event_includes() {
        let event = Event {
            start: Utc.ymd(2022, Month::July.number_from_month(), 2).and_hms(08, 00, 00),
            duration: Duration::hours(1),
            data: (),
        };

        let time_1 = Utc.ymd(2022, Month::July.number_from_month(), 2).and_hms(08, 30, 00);
        let time_2 = Utc.ymd(2022, Month::July.number_from_month(), 2).and_hms(07, 30, 00);
        let time_3 = Utc.ymd(2022, Month::July.number_from_month(), 2).and_hms(07, 29, 00);
        let time_4 = Utc.ymd(2022, Month::July.number_from_month(), 2).and_hms(09, 01, 00);

        let duration = Duration::minutes(30);

        assert!( event.includes(&time_1, duration), "the event should include the 30min range from {time_1:?}");
        assert!( event.includes(&time_2, duration), "the event should include the 30min range from {time_2:?}");
        assert!(!event.includes(&time_3, duration), "the event should not include the 30min range from {time_3:?}");
        assert!(!event.includes(&time_4, duration), "the event should not include the 30min range from {time_4:?}");
    }
}


