//! Dates, Japanese holidays, school terms and the town's event calendar.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

/// The game starts on the first day of the Japanese school year.
pub const START_DATE: Date = Date { year: 2026, month: 4, day: 6 };

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Weekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Weekday {
    pub fn index(self) -> u32 {
        self as u32
    }
    pub fn kanji(self) -> &'static str {
        ["月", "火", "水", "木", "金", "土", "日"][self as usize]
    }
    pub fn is_weekend(self) -> bool {
        matches!(self, Weekday::Sat | Weekday::Sun)
    }
}

impl Date {
    pub const fn new(year: i32, month: u32, day: u32) -> Self {
        Self { year, month, day }
    }

    /// Days since 1970-01-01 (Howard Hinnant's algorithm).
    pub fn to_days(self) -> i64 {
        let y = self.year as i64 - i64::from(self.month <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = self.month as i64;
        let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + self.day as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    pub fn from_days(z: i64) -> Self {
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
        Self { year, month, day }
    }

    pub fn add_days(self, n: i64) -> Self {
        Self::from_days(self.to_days() + n)
    }

    pub fn weekday(self) -> Weekday {
        // 1970-01-01 was a Thursday.
        const W: [Weekday; 7] = [Weekday::Thu, Weekday::Fri, Weekday::Sat, Weekday::Sun, Weekday::Mon, Weekday::Tue, Weekday::Wed];
        W[self.to_days().rem_euclid(7) as usize]
    }

    /// 1-based day of the year.
    pub fn day_of_year(self) -> u32 {
        (self.to_days() - Date::new(self.year, 1, 1).to_days()) as u32 + 1
    }

    pub fn month_name(self) -> &'static str {
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][self.month as usize - 1]
    }

    fn md(self) -> (u32, u32) {
        (self.month, self.day)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} {} {} {}", self.weekday(), self.day, self.month_name(), self.year)
    }
}

/// The `n`th (1-based) given weekday of a month.
pub fn nth_weekday(year: i32, month: u32, wd: Weekday, n: u32) -> Date {
    let first = Date::new(year, month, 1);
    let offset = (wd.index() + 7 - first.weekday().index()) % 7;
    Date::new(year, month, 1 + offset + 7 * (n - 1))
}

pub fn last_weekday(year: i32, month: u32, wd: Weekday) -> Date {
    let next = if month == 12 { Date::new(year + 1, 1, 1) } else { Date::new(year, month + 1, 1) };
    let last = next.add_days(-1);
    let back = (last.weekday().index() + 7 - wd.index()) % 7;
    last.add_days(-(back as i64))
}

fn equinox_day(year: i32, base: f64) -> u32 {
    let y = (year - 1980) as f64;
    (base + 0.242194 * y - (y / 4.0).floor()).floor() as u32
}

/// National holidays before substitute and "citizens'" holidays are applied.
fn base_holiday(d: Date) -> Option<&'static str> {
    let y = d.year;
    let named = match d.md() {
        (1, 1) => Some("New Year's Day"),
        (2, 11) => Some("National Foundation Day"),
        (2, 23) => Some("Emperor's Birthday"),
        (4, 29) => Some("Shōwa Day"),
        (5, 3) => Some("Constitution Day"),
        (5, 4) => Some("Greenery Day"),
        (5, 5) => Some("Children's Day"),
        (8, 11) => Some("Mountain Day"),
        (11, 3) => Some("Culture Day"),
        (11, 23) => Some("Labour Thanksgiving Day"),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    if d.month == 3 && d.day == equinox_day(y, 20.8431) {
        return Some("Vernal Equinox Day");
    }
    if d.month == 9 && d.day == equinox_day(y, 23.2488) {
        return Some("Autumnal Equinox Day");
    }
    let happy_monday = [
        (1, 2, "Coming of Age Day"),
        (7, 3, "Marine Day"),
        (9, 3, "Respect for the Aged Day"),
        (10, 2, "Sports Day"),
    ];
    happy_monday
        .iter()
        .find(|(m, n, _)| d.month == *m && d == nth_weekday(y, *m, Weekday::Mon, *n))
        .map(|x| x.2)
}

pub fn holiday(d: Date) -> Option<&'static str> {
    if let Some(h) = base_holiday(d) {
        return Some(h);
    }
    if d.weekday() != Weekday::Sun {
        // Substitute holiday: the first non-holiday after a holiday that fell on a Sunday.
        let mut k = 1;
        while base_holiday(d.add_days(-k)).is_some() {
            if d.add_days(-k).weekday() == Weekday::Sun {
                return Some("Substitute Holiday");
            }
            k += 1;
        }
        // Citizens' holiday: a day sandwiched between two holidays.
        if base_holiday(d.add_days(-1)).is_some() && base_holiday(d.add_days(1)).is_some() {
            return Some("Citizens' Holiday");
        }
    }
    None
}

pub fn school_break(d: Date) -> Option<&'static str> {
    match d.md() {
        (7, 21..) | (8, _) => Some("Summer Break"),
        (12, 26..) | (1, ..=7) => Some("Winter Break"),
        (3, 25..) | (4, ..=5) => Some("Spring Break"),
        _ => None,
    }
}

pub fn is_school_day(d: Date) -> bool {
    !d.weekday().is_weekend() && holiday(d).is_none() && school_break(d).is_none()
}

/// Which card season has been released by this date (1 = Dawn of Dragons, April 2026).
/// Each season comes out on the first Saturday of its month.
pub fn released_season(d: Date) -> u16 {
    let months = (d.year - START_DATE.year) * 12 + d.month as i32 - START_DATE.month as i32;
    let release = nth_weekday(d.year, d.month, Weekday::Sat, 1);
    let n = months + 1 - i32::from(d < release);
    n.max(1) as u16
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    EntranceCeremony,
    SetRelease(u16),
    ShopTournament,
    Exams(&'static str),
    SportsDay,
    SummerFestival,
    CultureFestival,
    ClosingCeremony(&'static str),
    Holiday(&'static str),
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Event::EntranceCeremony => write!(f, "Entrance Ceremony"),
            Event::SetRelease(n) => write!(f, "New season out: {}", tcg::season::theme(*n).name),
            Event::ShopTournament => write!(f, "Monthly tournament at the shop"),
            Event::Exams(t) => write!(f, "{t} exams"),
            Event::SportsDay => write!(f, "Sports Day"),
            Event::SummerFestival => write!(f, "Obon Summer Festival"),
            Event::CultureFestival => write!(f, "Culture Festival"),
            Event::ClosingCeremony(t) => write!(f, "End of {t} term"),
            Event::Holiday(h) => write!(f, "{h}"),
        }
    }
}

pub fn events_on(d: Date) -> Vec<Event> {
    let mut ev = Vec::new();
    if d == START_DATE {
        ev.push(Event::EntranceCeremony);
    }
    if d >= START_DATE && d == nth_weekday(d.year, d.month, Weekday::Sat, 1) {
        ev.push(Event::SetRelease(released_season(d)));
    }
    if d == last_weekday(d.year, d.month, Weekday::Sun) {
        ev.push(Event::ShopTournament);
    }
    if let Some(h) = holiday(d) {
        ev.push(Event::Holiday(h));
    }
    let y = d.year;
    let exams = [
        (5, 3, "Mid-term"),
        (7, 1, "First-term final"),
        (10, 3, "Mid-term"),
        (12, 1, "Second-term final"),
        (2, 4, "Year-end"),
    ];
    for (m, week, name) in exams {
        let start = nth_weekday(y, m, Weekday::Tue, week);
        if d >= start && d <= start.add_days(2) {
            ev.push(Event::Exams(name));
        }
    }
    if d == last_weekday(y, 9, Weekday::Sat) {
        ev.push(Event::SportsDay);
    }
    if d.month == 8 && (13..=15).contains(&d.day) {
        ev.push(Event::SummerFestival);
    }
    if d == nth_weekday(y, 11, Weekday::Sat, 1) {
        ev.push(Event::CultureFestival);
    }
    match d.md() {
        (7, 20) => ev.push(Event::ClosingCeremony("first")),
        (12, 25) => ev.push(Event::ClosingCeremony("second")),
        (3, 24) => ev.push(Event::ClosingCeremony("school year")),
        _ => {}
    }
    ev
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_roundtrip_and_weekday() {
        assert_eq!(START_DATE.weekday(), Weekday::Mon);
        assert_eq!(Date::new(1970, 1, 1).to_days(), 0);
        for z in -1000..100_000 {
            assert_eq!(Date::from_days(z).to_days(), z);
        }
        assert_eq!(Date::new(2026, 12, 31).add_days(1), Date::new(2027, 1, 1));
        assert_eq!(Date::new(2028, 2, 28).add_days(1), Date::new(2028, 2, 29));
    }

    #[test]
    fn japanese_holidays_2026() {
        assert_eq!(holiday(Date::new(2026, 5, 6)), Some("Substitute Holiday")); // May 3 is a Sunday
        assert_eq!(holiday(Date::new(2026, 7, 20)), Some("Marine Day"));
        assert_eq!(holiday(Date::new(2026, 9, 21)), Some("Respect for the Aged Day"));
        assert_eq!(holiday(Date::new(2026, 9, 22)), Some("Citizens' Holiday"));
        assert_eq!(holiday(Date::new(2026, 9, 23)), Some("Autumnal Equinox Day"));
        assert_eq!(holiday(Date::new(2026, 10, 12)), Some("Sports Day"));
        assert_eq!(holiday(Date::new(2027, 3, 21)), Some("Vernal Equinox Day"));
        assert_eq!(holiday(Date::new(2027, 3, 22)), Some("Substitute Holiday"));
        assert_eq!(holiday(Date::new(2026, 6, 10)), None);
    }

    #[test]
    fn school_days_and_seasons() {
        assert!(is_school_day(START_DATE));
        assert!(!is_school_day(Date::new(2026, 4, 11))); // Saturday
        assert!(!is_school_day(Date::new(2026, 8, 3))); // summer break
        // First Saturday of May 2026 is the 2nd.
        assert_eq!(released_season(Date::new(2026, 5, 1)), 1);
        assert_eq!(released_season(Date::new(2026, 5, 2)), 2);
        assert_eq!(released_season(Date::new(2027, 3, 31)), 12);
        assert!(events_on(Date::new(2026, 5, 2)).contains(&Event::SetRelease(2)));
    }
}
