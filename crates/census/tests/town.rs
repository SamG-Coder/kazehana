//! The town's people hang together: everyone has a home and a family that makes sense, the
//! school and its staff are complete, schedules put people where they should be.

use census::{generate, Census, GroupKind, PlaceKind, Plan, Sex, Work};
use std::collections::HashSet;
use std::path::Path;

fn town() -> Census {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/town/town.json");
    let plan = Plan::load(&path).unwrap_or_else(|_| Plan::fallback());
    generate(7, &plan)
}

fn day(weekday: u8, school_day: bool) -> census::Day {
    census::Day { weekday, month: 4, date: 8 + weekday, school_day }
}

#[test]
fn the_town_is_the_right_shape() {
    let c = town();
    let res: Vec<_> = c.residents().collect();
    let n = res.len();
    assert!((1150..=1700).contains(&n), "{n} people");
    let old = res.iter().filter(|p| p.age >= 65).count() as f32 / n as f32;
    assert!((0.24..=0.42).contains(&old), "65+ share {old}");
    let young = res.iter().filter(|p| p.age < 15).count() as f32 / n as f32;
    assert!((0.1..=0.22).contains(&young), "under-15 share {young}");
    // every house is lived in
    for h in &c.houses {
        assert!(!h.households.is_empty(), "house {} empty", h.id);
    }
    // the same seed, the same town
    let again = town();
    assert_eq!(again.people.len(), c.people.len());
    assert_eq!(again.people[500].name(), c.people[500].name());
}

#[test]
fn families_make_sense() {
    let c = town();
    let mut names = HashSet::new();
    for p in c.residents() {
        assert!(names.insert(p.name()), "two people called {}", p.name());
        for parent in [p.father, p.mother].into_iter().flatten() {
            let q = c.person(parent);
            assert!(q.age as i32 - p.age as i32 >= 16, "{} ({}) is the parent of {} ({})", q.name(), q.age, p.name(), p.age);
        }
        if let Some(s) = p.spouse {
            assert_eq!(c.person(s).spouse, Some(p.id), "{} married one way", p.name());
            assert_ne!(c.person(s).sex, p.sex);
        }
        // children live with a grown-up
        if p.age < 15 && p.id != c.player {
            let hh = c.household(p.household.unwrap());
            assert!(hh.members.iter().any(|&m| c.person(m).lives_here() && c.person(m).age >= 20), "{} lives without an adult", p.name());
        }
    }
    // relatives across the town: plenty of people have family in another house
    let linked = c.residents().filter(|p| !c.relatives_elsewhere_in_town(p.id).is_empty()).count();
    assert!(linked > c.residents().count() / 5, "only {linked} people have relatives in other houses");
    assert_eq!(c.person(c.kenji).given_name, "Kenji");
    assert_eq!(c.household(c.person(c.player).household.unwrap()).family_name, "Tsukumo");
}

#[test]
fn the_school_is_complete() {
    let c = town();
    for grade in 1..=3u8 {
        let kids: Vec<_> = c.residents().filter(|p| matches!(p.work, Work::JuniorHigh { grade: g, .. } if g == grade) || (grade == 1 && p.work == Work::Player)).collect();
        assert!((32..=34).contains(&kids.len()) && kids.len() >= 33, "year {grade}: {}", kids.len());
        for class in ['A', 'B'] {
            let k = kids.iter().filter(|p| matches!(p.work, Work::JuniorHigh { class: x, .. } if x == class) || (p.work == Work::Player && class == 'A')).count();
            assert!((15..=18).contains(&k), "{grade}-{class}: {k}");
        }
    }
    let total: usize = c.residents().filter(|p| matches!(p.work, Work::JuniorHigh { .. } | Work::Player)).count();
    assert_eq!(total, 100);
    let jh_staff = c.residents().filter(|p| matches!(p.work, Work::Teacher { school: PlaceKind::JuniorHigh, .. })).count();
    let es_staff = c.residents().filter(|p| matches!(p.work, Work::Teacher { school: PlaceKind::Elementary, .. })).count();
    assert_eq!((jh_staff, es_staff), (12, 10));
    // brothers and sisters are not in the same year
    for p in c.residents().filter(|p| matches!(p.work, Work::JuniorHigh { .. })) {
        for s in c.relatives(p.id).into_iter().filter(|r| r.1 == census::Kin::Sibling) {
            let q = c.person(s.0);
            assert!(q.age != p.age || q.id == p.id, "{} and {} are siblings in one year", p.name(), q.name());
        }
    }
}

#[test]
fn schedules_put_people_where_they_should_be() {
    let c = town();
    let jh = c.place_of(PlaceKind::JuniorHigh).unwrap().id;
    let es = c.place_of(PlaceKind::Elementary).unwrap().id;
    let wed = day(2, true);
    for p in c.residents() {
        let a = c.activity(p.id, wed, 10 * 60);
        match &p.work {
            Work::JuniorHigh { .. } => assert_eq!(a.place, Some(jh), "{} at 10:00: {a:?}", p.name()),
            Work::Elementary { .. } => assert_eq!(a.place, Some(es), "{} at 10:00: {a:?}", p.name()),
            Work::Commuter if p.workdays & 1 << 2 != 0 => assert_eq!(a.place, None, "{} should be in Minato-shi", p.name()),
            _ => {}
        }
        // three in the morning: everyone home
        let night = c.activity(p.id, wed, 3 * 60);
        if p.work != Work::Player {
            assert_eq!(night.place, c.home(p.id), "{} at 3:00: {night:?}", p.name());
        }
    }
    // a Saturday afternoon: some kids are out with friends
    let sat = day(5, false);
    let out = c.residents().filter(|p| c.activity(p.id, sat, 15 * 60).group.is_some_and(|g| c.group(g).kind == GroupKind::Friends)).count();
    assert!(out > 10, "{out} out with friends");
    // the gateball team plays on Tuesday mornings
    let tue = day(1, true);
    let gb = c.groups.iter().find(|g| g.kind == GroupKind::Gateball).unwrap();
    let playing = gb.members.iter().filter(|&&m| c.activity(m, tue, 9 * 60).group == Some(gb.id)).count();
    assert!(playing >= gb.members.len() / 2, "{playing} of {} at gateball", gb.members.len());
}

#[test]
fn everyone_has_a_look_for_their_age() {
    let c = town();
    for p in c.residents() {
        let l = &p.look;
        let mean = census::look::mean_height(p.sex, p.age);
        assert!((l.height_cm as f32 - mean).abs() < 25.0, "{} {}cm at {}", p.name(), l.height_cm, p.age);
        if matches!(p.work, Work::JuniorHigh { .. }) {
            let want = if p.sex == Sex::M { census::look::Outfit::Gakuran } else { census::look::Outfit::Sailor };
            assert_eq!(l.day.outfit, want, "{} not in uniform", p.name());
        }
    }
}

#[test]
fn it_saves_and_loads() {
    let c = town();
    let dir = std::env::temp_dir().join("census_roundtrip.json");
    c.save(&dir).unwrap();
    let back = Census::load(&dir).unwrap();
    assert_eq!(back.people.len(), c.people.len());
    assert_eq!(back.groups.len(), c.groups.len());
}

#[test]
fn a_week_of_everyone_s_schedules() {
    let c = town();
    let konbini = c.place_of(PlaceKind::Konbini).unwrap().id;
    let jh = c.place_of(PlaceKind::JuniorHigh).unwrap().id;
    // Monday 6 April .. Sunday 12 April 2026, every quarter of an hour
    for (k, weekday) in (0..7u8).enumerate() {
        let day = census::Day { weekday, month: 4, date: 6 + k as u8, school_day: weekday < 5 };
        for q in 0..96u16 {
            let minute = q * 15;
            let mut at_konbini = 0;
            let mut teachers_at_jh = 0;
            let mut students_at_jh = 0;
            for p in c.residents() {
                let a = c.activity(p.id, day, minute);
                if let Some(pl) = a.place {
                    assert!((pl as usize) < c.places.len(), "{} at a place that doesn't exist", p.name());
                }
                assert!(!a.doing.is_empty());
                if a.place == Some(konbini) && a.doing == "at work" {
                    at_konbini += 1;
                }
                if a.place == Some(jh) {
                    match p.work {
                        Work::Teacher { .. } => teachers_at_jh += 1,
                        Work::JuniorHigh { .. } => students_at_jh += 1,
                        _ => {}
                    }
                }
            }
            // the konbini always has someone on from 6:00 to 22:00
            if (6 * 60..22 * 60).contains(&minute) {
                assert!(at_konbini >= 1, "nobody at the konbini at {}:{:02} on day {weekday}", minute / 60, minute % 60);
            }
            // whenever students are at school in lesson time, so are teachers
            if day.school_day && (9 * 60..15 * 60).contains(&minute) {
                assert!(students_at_jh >= 90 && teachers_at_jh >= 10, "{students_at_jh} students, {teachers_at_jh} teachers at {minute}");
            }
            // night: (nearly) everyone at home
            if minute == 3 * 60 {
                let out = c.residents().filter(|p| c.activity(p.id, day, minute).place != c.home(p.id) && p.work != Work::Player).count();
                assert_eq!(out, 0, "{out} people out at 3am");
            }
        }
    }
}

#[test]
fn the_school_timetable_fits() {
    use census::school::{periods_on, Subject, Timetable, CLASSES};
    let c = town();
    let tt = Timetable::build(&c);
    for (ci, &class) in CLASSES.iter().enumerate() {
        // every class: 29 lessons, each subject its weekly number, never twice in a day
        let mut n = std::collections::HashMap::new();
        for d in 0..5u8 {
            let mut today = Vec::new();
            for p in 1..=periods_on(d) {
                let l = tt.lesson(class, d, p).unwrap_or_else(|| panic!("{class:?} has a gap on day {d} period {p}"));
                assert!(!today.contains(&l.subject), "{class:?}: {} twice on day {d}", l.subject.name());
                today.push(l.subject);
                *n.entry(l.subject).or_insert(0u8) += 1;
            }
            assert!(tt.lesson(class, d, 6).is_none() || periods_on(d) == 6);
        }
        for &(s, k) in Subject::WEEK.iter() {
            assert_eq!(n.get(&s).copied().unwrap_or(0), k, "{class:?} {}", s.name());
        }
        // the homeroom teacher takes moral education, integrated studies and LHR
        let hr = tt.homeroom[ci];
        assert!(tt.teachers[hr].person.is_some());
    }
    // no teacher in two rooms at once
    for t in 0..tt.teachers.len() {
        for d in 0..5u8 {
            for p in 1..=6u8 {
                let n = CLASSES.iter().filter(|&&cl| tt.lesson(cl, d, p).is_some_and(|l| l.teacher == t)).count();
                assert!(n <= 1, "{} teaches {n} classes at once", tt.teachers[t].name);
            }
        }
    }
    // (1-A's week, for the record)
    for d in 0..5u8 {
        let day: Vec<String> = (1..=periods_on(d)).map(|p| {
            let l = tt.lesson((1, 'A'), d, p).unwrap();
            format!("{} ({})", l.subject.name(), tt.teachers[l.teacher].sensei())
        }).collect();
        println!("{}: {}", ["Mon", "Tue", "Wed", "Thu", "Fri"][d as usize], day.join(" | "));
    }
}

#[test]
fn students_and_their_teacher_meet_in_each_lesson() {
    let c = town();
    let tt = c.timetable();
    let tue = day(1, true);
    let ten = 10 * 60; // 2nd period
    for &class in census::school::CLASSES.iter() {
        let l = tt.lesson(class, 1, 2).unwrap();
        let students: Vec<_> = c.people.iter().filter(|p| matches!(p.work, Work::JuniorHigh { grade, class: k, .. } if (grade, k) == class) && p.lives_here()).collect();
        assert!(!students.is_empty());
        for s in &students {
            let a = c.activity(s.id, tue, ten);
            assert_eq!(a.lesson.as_ref().map(|x| (x.0.subject, x.1)), Some((l.subject, class)), "{} at 10:00 on Tuesday: {}", s.name(), a.doing);
        }
        if let Some(t) = tt.teachers[l.teacher].person {
            let a = c.activity(t, tue, ten);
            assert_eq!(a.lesson.as_ref().map(|x| x.1), Some(class), "{} should be teaching {class:?}", c.person(t).name());
        }
    }
}

#[test]
fn students_get_to_school_at_their_own_times() {
    use census::schedule::{activity, jh_morning, Day};
    let c = census::Census::load(std::path::Path::new("../../assets/town/census.json")).expect("census");
    let students: Vec<_> = c.people.iter().filter(|p| matches!(p.work, census::Work::JuniorHigh { .. })).map(|p| p.id).collect();
    let mut arrivals = Vec::new();
    let mut late = 0;
    for date in 6..=24u8 {
        let day = Day { weekday: ((date - 6) % 7) as u8, month: 4, date, school_day: (date - 6) % 7 < 5 };
        if !day.school_day {
            continue;
        }
        for &id in &students {
            let (wake, leave, arrive, bed) = jh_morning(&c, id, &day);
            assert!(wake < leave && leave < arrive && bed > arrive, "{id}: {wake} {leave} {arrive} {bed}");
            if arrive > 8 * 60 + 25 {
                late += 1;
            }
            arrivals.push(arrive);
            // walking between leaving and arriving; at school after
            assert_eq!(activity(&c, id, day, leave).doing, "walking to school");
            assert!(activity(&c, id, day, arrive + 1).place.is_some());
        }
    }
    let (lo, hi) = (*arrivals.iter().min().unwrap(), *arrivals.iter().max().unwrap());
    // spread out over the morning, not all at one minute; a few late
    assert!(hi - lo >= 20, "arrivals {lo}..{hi}");
    let n = arrivals.len();
    assert!(late * 100 / n <= 5, "{late} of {n} late");
    // and each student's own day differs from day to day
    let id = students[0];
    let a: std::collections::BTreeSet<u16> = (6..=10u8).map(|date| jh_morning(&c, id, &Day { weekday: date - 6, month: 4, date, school_day: true }).2).collect();
    assert!(a.len() > 1, "the same arrival every day: {a:?}");
}
