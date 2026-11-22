//! Your school day: the junior high's bells and timetable (census::school, docs/school_plan.md),
//! seen from your seat in 1-A.

use census::school::{self, Lesson, Timetable};
use std::sync::OnceLock;

pub use census::school::{periods_on, slot_at, Slot, Subject, PERIODS};

/// Your class.
pub const MY_CLASS: (u8, char) = (1, 'A');

/// The town's census (the staff, the classes), read once.
pub fn census() -> &'static census::Census {
    static C: OnceLock<census::Census> = OnceLock::new();
    C.get_or_init(|| serde_json::from_str(include_str!("../../../assets/town/census.json")).expect("assets/town/census.json"))
}

/// The junior high's week.
pub fn timetable() -> &'static Timetable {
    census().timetable()
}

/// Your lesson in a period of a weekday (0 = Monday), if there is one.
pub fn my_lesson(weekday: u8, period: u8) -> Option<&'static Lesson> {
    timetable().lesson(MY_CLASS, weekday, period)
}

/// "2nd period: maths with Yamada-sensei" (in its room if it isn't your classroom).
pub fn describe(weekday: u8, period: u8) -> Option<String> {
    let l = my_lesson(weekday, period)?;
    let t = timetable().teacher(l.teacher);
    let room = match l.room {
        school::Room::Homeroom => String::new(),
        r => format!(", in {}", r.name(MY_CLASS)),
    };
    Some(format!("{} period: {} with {}{room}", school::ordinal(period), l.subject.name(), t.sensei()))
}

/// The day's lessons in order, as a line ("social studies, maths, class activities, ...").
pub fn day_line(weekday: u8, from: u8, to: u8) -> String {
    (from..=to.min(periods_on(weekday))).filter_map(|p| my_lesson(weekday, p)).map(|l| l.subject.name()).collect::<Vec<_>>().join(", ")
}

/// Whether you're in a lesson (a period, or one of the meetings) at `minute` of a school day.
pub fn in_lessons(weekday: u8, minute: u32) -> Option<Slot> {
    let s = slot_at(weekday, minute);
    matches!(s, Slot::Period(_) | Slot::MorningMeeting | Slot::EndMeeting).then_some(s)
}
