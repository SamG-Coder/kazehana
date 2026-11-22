//! Who is in a place now, from the census schedule (census::schedule): what the world draws
//! (game::crowd). Everyone's whereabouts come from the one schedule; a room only says where in it
//! they go.
//!
//! So far, a class's homeroom: each student at their own seat (by the class roll, 出席番号, in
//! name order), the teacher at the front. Students are seated for the lessons taught in the
//! room, morning reading, the meetings and school lunch; the teacher is at the front for those
//! lessons and the homeroom teacher for the meetings.

use crate::calendar::{is_school_day, Date};
use census::schedule::{self, Day};
use census::school::{slot_at, Room, Slot};
use census::{PersonId, Work};

/// Where in the room someone is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    /// at their desk (the seat number: their place on the class roll)
    Seat(u8),
    /// at the front: the teacher's place on the platform, by the board
    Front,
}

/// What they're doing there (it picks their animation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Doing {
    /// sitting and listening (meetings, morning reading)
    Sitting,
    /// in a lesson: writing, looking up
    Working,
    /// eating school lunch at their desk
    Eating,
    /// teaching at the board
    Teaching,
    /// taking the register, the day's notices
    Talking,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Present {
    pub person: PersonId,
    pub spot: Spot,
    pub doing: Doing,
}

/// A census `Day` for a date.
pub fn day(d: Date) -> Day {
    Day { weekday: d.weekday() as u8, month: d.month as u8, date: d.day as u8, school_day: is_school_day(d) }
}

/// A class's roll: its students in name order (出席番号 1, 2, ...). (You aren't on the census's
/// roll: your seat is your own.)
pub fn roll(class: (u8, char)) -> Vec<PersonId> {
    let c = crate::school::census();
    let mut ids: Vec<&census::Person> = c.people.iter().filter(|p| matches!(&p.work, Work::JuniorHigh { grade, class: k, .. } if *grade == class.0 && *k == class.1)).collect();
    ids.sort_by(|a, b| (&a.family_name, &a.given_name).cmp(&(&b.family_name, &b.given_name)));
    ids.into_iter().map(|p| p.id).collect()
}

/// Who is in a class's homeroom at a minute of a date.
pub fn in_homeroom(class: (u8, char), date: Date, minute: u32) -> Vec<Present> {
    let c = crate::school::census();
    let d = day(date);
    if !d.school_day {
        return Vec::new();
    }
    let slot = slot_at(d.weekday, minute);
    // (at school: a student at home getting ready, or late, is not in the room)
    let jh = c.place_of(census::PlaceKind::JuniorHigh).map(|p| p.id);
    let mut out = Vec::new();
    for (k, id) in roll(class).into_iter().enumerate() {
        let a = schedule::activity(c, id, d, minute as u16);
        let here = match (&a.lesson, slot) {
            (Some((l, _)), _) => (l.room == Room::Homeroom).then_some(Doing::Working),
            (None, Slot::MorningReading | Slot::MorningMeeting | Slot::EndMeeting) if a.place.is_some() && a.place == jh => Some(Doing::Sitting),
            // in before the bell: at their desks, as they arrive (each at their own time)
            (None, Slot::Arrival) if a.place.is_some() && a.place == jh => Some(Doing::Sitting),
            (None, Slot::Lunch) if a.place.is_some() && a.place == jh => Some(Doing::Eating),
            _ => None,
        };
        if let Some(doing) = here {
            out.push(Present { person: id, spot: Spot::Seat(k as u8 + 1), doing });
        }
    }
    // the teacher at the front: the lesson's, or the homeroom teacher for the meetings
    let tt = c.timetable();
    let teacher = match slot {
        Slot::Period(n) => tt.lesson(class, d.weekday, n).filter(|l| l.room == Room::Homeroom).map(|l| (l.teacher, Doing::Teaching)),
        Slot::MorningMeeting | Slot::EndMeeting => census::school::Timetable::class_index(class).map(|ci| (tt.homeroom[ci], Doing::Talking)),
        _ => None,
    };
    if let Some((t, doing)) = teacher {
        if let Some(person) = tt.teachers.get(t).and_then(|t| t.person) {
            out.push(Present { person, spot: Spot::Front, doing });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::school::MY_CLASS;

    #[test]
    fn a_morning_in_1a() {
        let roll = roll(MY_CLASS);
        assert_eq!(roll.len(), 16, "1-A's census students");
        // Monday 13 April, the 2nd period (maths, in the classroom): the class and the teacher
        let d = Date::new(2026, 4, 13);
        let now = in_homeroom(MY_CLASS, d, crate::school::PERIODS[1].0 + 10);
        let seated = now.iter().filter(|p| matches!(p.spot, Spot::Seat(_))).count();
        assert_eq!(seated, 16, "everyone at their desk");
        let front: Vec<_> = now.iter().filter(|p| p.spot == Spot::Front).collect();
        assert_eq!(front.len(), 1);
        let maths = crate::school::my_lesson(0, 2).unwrap();
        assert_eq!(Some(front[0].person), crate::school::timetable().teacher(maths.teacher).person);
        // seats follow the roll
        assert!(now.iter().any(|p| p.person == roll[0] && p.spot == Spot::Seat(1)));
        // PE (Monday's 5th) is in the gym: nobody in the classroom
        let pe = in_homeroom(MY_CLASS, d, crate::school::PERIODS[4].0 + 10);
        assert!(pe.is_empty(), "{pe:?}");
        // the morning meeting: the homeroom teacher talking at the front
        let mm = in_homeroom(MY_CLASS, d, census::school::MORNING_MEETING + 5);
        assert!(mm.iter().any(|p| p.spot == Spot::Front && p.doing == Doing::Talking));
        // a Saturday: nobody
        assert!(in_homeroom(MY_CLASS, Date::new(2026, 4, 18), 600).is_empty());
    }
}
