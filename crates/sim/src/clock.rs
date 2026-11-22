//! Game time, day phases, and the real sun and moon over Kazehana (36°N).

use crate::calendar::{is_school_day, Date};
use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};
use std::fmt;

pub const MINUTES_PER_DAY: u32 = 24 * 60;
pub const LATITUDE_DEG: f32 = 36.0;

/// Clock times (minutes after midnight) the day is organised around.
pub mod at {
    pub const fn hm(h: u32, m: u32) -> u32 {
        h * 60 + m
    }
    pub const WAKE: u32 = hm(6, 30);
    /// The junior high's bells (census::school, docs/school_plan.md): the morning meeting, the
    /// end of the 4th period (lunch), the 5th period, and the end-of-day meeting's end on a
    /// six-period day (Wednesdays end an hour earlier: `school_out`).
    pub const HOMEROOM: u32 = hm(8, 30);
    pub const LUNCH: u32 = hm(12, 40);
    pub const AFTERNOON: u32 = hm(13, 50);
    pub const SCHOOL_OUT: u32 = hm(15, 50);
    /// When school's out on a weekday (0 = Monday).
    pub fn school_out(weekday: u8) -> u32 {
        census::school::end_meeting(weekday).1
    }
    /// Tsukumodō opens early: the kids drop in on the way to school.
    pub const SHOP_OPEN: u32 = hm(7, 0);
    pub const SHOP_CLOSE: u32 = hm(20, 0);
    pub const EVENING: u32 = hm(18, 0);
    pub const NIGHT: u32 = hm(21, 0);
    pub const CURFEW: u32 = hm(22, 0);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GameTime {
    pub date: Date,
    /// Minutes after midnight, 0..1440.
    pub minute: u32,
}

impl GameTime {
    pub fn new(date: Date, minute: u32) -> Self {
        Self { date, minute }
    }

    pub fn hour(&self) -> u32 {
        self.minute / 60
    }

    /// Advance; returns how many midnights were crossed.
    pub fn advance(&mut self, minutes: u32) -> u32 {
        let total = self.minute + minutes;
        let days = total / MINUTES_PER_DAY;
        self.minute = total % MINUTES_PER_DAY;
        self.date = self.date.add_days(days as i64);
        days
    }

    /// Minutes until the next time the clock reads `minute` (0 if now).
    pub fn until(&self, minute: u32) -> u32 {
        (minute + MINUTES_PER_DAY - self.minute) % MINUTES_PER_DAY
    }

    pub fn school_day(&self) -> bool {
        is_school_day(self.date)
    }

    pub fn phase(&self) -> Phase {
        use at::*;
        let m = self.minute;
        let school = self.school_day();
        match m {
            _ if m < hm(5, 0) => Phase::LateNight,
            _ if m < hm(7, 0) => Phase::EarlyMorning,
            _ if m < HOMEROOM => Phase::Morning,
            _ if m < EVENING && !school => Phase::Daytime,
            _ if m < LUNCH => Phase::MorningClass,
            _ if m < AFTERNOON => Phase::Lunch,
            _ if m < school_out(self.date.weekday() as u8) => Phase::AfternoonClass,
            _ if m < EVENING => Phase::AfterSchool,
            _ if m < NIGHT => Phase::Evening,
            _ => Phase::Night,
        }
    }
}

impl fmt::Display for GameTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {:02}:{:02}", self.date, self.minute / 60, self.minute % 60)
    }
}

/// Time-of-day blocks shown in the HUD, as in Persona.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Phase {
    LateNight,
    EarlyMorning,
    Morning,
    MorningClass,
    Lunch,
    AfternoonClass,
    AfterSchool,
    /// Daytime on weekends and holidays.
    Daytime,
    Evening,
    Night,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::LateNight => "Late Night",
            Phase::EarlyMorning => "Early Morning",
            Phase::Morning => "Morning",
            Phase::MorningClass => "Class (AM)",
            Phase::Lunch => "Lunch",
            Phase::AfternoonClass => "Class (PM)",
            Phase::AfterSchool => "After School",
            Phase::Daytime => "Daytime",
            Phase::Evening => "Evening",
            Phase::Night => "Night",
        }
    }
    pub fn in_class(self) -> bool {
        matches!(self, Phase::MorningClass | Phase::AfternoonClass)
    }
}

/// Sun and moon for lighting. Directions point *towards* the body, in world space:
/// +X east, +Y up, -Z north.
#[derive(Clone, Copy, Debug)]
pub struct Sky {
    pub sun_dir: [f32; 3],
    /// Sun elevation in radians (negative below the horizon).
    pub sun_elevation: f32,
    pub moon_dir: [f32; 3],
    pub moon_elevation: f32,
    /// 0 = new moon, 0.5 = full moon.
    pub moon_phase: f32,
    /// Moon illumination 0..1.
    pub moon_light: f32,
}

fn body_direction(hour_angle: f32, decl: f32, lat: f32) -> ([f32; 3], f32) {
    let east = -decl.cos() * hour_angle.sin();
    let north = lat.cos() * decl.sin() - lat.sin() * decl.cos() * hour_angle.cos();
    let up = lat.sin() * decl.sin() + lat.cos() * decl.cos() * hour_angle.cos();
    ([east, up, -north], up.clamp(-1.0, 1.0).asin())
}

pub fn sky(t: &GameTime) -> Sky {
    sky_at(t, 0.0)
}

/// The sky `extra` minutes (a fraction of one, usually) after `t`: the sun and moon move
/// smoothly between the clock's whole minutes.
pub fn sky_at(t: &GameTime, extra: f32) -> Sky {
    let minute = t.minute as f32 + extra;
    let lat = LATITUDE_DEG.to_radians();
    let tilt = 23.44f32.to_radians();
    let day = t.date.day_of_year() as f32 + minute / MINUTES_PER_DAY as f32;
    // Sun's ecliptic longitude (0 at the March equinox, ~day 80).
    let sun_lon = TAU * (day - 80.0) / 365.25;
    let sun_decl = (tilt.sin() * sun_lon.sin()).asin();
    let hour_angle = (minute / 60.0 - 12.0) * PI / 12.0;
    let (sun_dir, sun_elevation) = body_direction(hour_angle, sun_decl, lat);

    // Moon: phase from a known new moon (2000-01-06 18:14 UTC), synodic month 29.53 days.
    let days = t.date.to_days() as f32 + minute / MINUTES_PER_DAY as f32 - 9 as f32 / 24.0;
    let new_moon = Date::new(2000, 1, 6).to_days() as f32 + 18.23 / 24.0;
    let moon_phase = ((days - new_moon) / 29.530_588).rem_euclid(1.0);
    // The moon trails the sun by phase × 24h and sits phase × 360° further along the ecliptic.
    let moon_lon = sun_lon + moon_phase * TAU;
    let moon_decl = (tilt.sin() * moon_lon.sin()).asin();
    let (moon_dir, moon_elevation) = body_direction(hour_angle - moon_phase * TAU, moon_decl, lat);
    let moon_light = 0.5 - 0.5 * (moon_phase * TAU).cos();

    Sky { sun_dir, sun_elevation, moon_dir, moon_elevation, moon_phase, moon_light }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sun_rises_in_east_sets_in_west() {
        let d = Date::new(2026, 6, 21);
        let morning = sky(&GameTime::new(d, at::hm(7, 0)));
        let noon = sky(&GameTime::new(d, at::hm(12, 0)));
        let evening = sky(&GameTime::new(d, at::hm(17, 0)));
        let midnight = sky(&GameTime::new(d, 0));
        assert!(morning.sun_dir[0] > 0.3 && evening.sun_dir[0] < -0.3);
        assert!(noon.sun_dir[2] > 0.0, "noon sun is to the south (+Z)");
        // Midsummer noon at 36°N is about 77° up.
        assert!((noon.sun_elevation.to_degrees() - 77.4).abs() < 1.0);
        assert!(midnight.sun_elevation < 0.0);
        // Winter noon is low (~31°).
        let winter = sky(&GameTime::new(Date::new(2026, 12, 21), at::hm(12, 0)));
        assert!((winter.sun_elevation.to_degrees() - 30.6).abs() < 1.5);
    }

    #[test]
    fn moon_phase_matches_a_known_full_moon() {
        // Full moon on 2026-05-01 ~17:23 UTC (02:23 JST on the 2nd).
        let s = sky(&GameTime::new(Date::new(2026, 5, 2), at::hm(2, 0)));
        assert!((s.moon_phase - 0.5).abs() < 0.03, "phase {}", s.moon_phase);
        assert!(s.moon_elevation > 0.0, "a full moon is up at 2am");
    }

    #[test]
    fn phases() {
        let school = GameTime::new(Date::new(2026, 4, 7), at::hm(12, 40));
        assert_eq!(school.phase(), Phase::Lunch);
        let sat = GameTime::new(Date::new(2026, 4, 11), at::hm(12, 40));
        assert_eq!(sat.phase(), Phase::Daytime);
        let mut t = GameTime::new(Date::new(2026, 4, 30), at::hm(23, 0));
        assert_eq!(t.advance(120), 1);
        assert_eq!(t, GameTime::new(Date::new(2026, 5, 1), 60));
    }
}
