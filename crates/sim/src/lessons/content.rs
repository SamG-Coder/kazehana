//! What each week's lessons ask: a generator per content key (`syllabus`), from number
//! problems made fresh each time (maths, the science calculations) to banks of questions
//! (kanji readings, grammar, history...). Every key in the syllabus makes tasks (a test checks).

use super::play::{SliderKind, Task, Virtue};
use super::syllabus::{self, Week};
use crate::school::Subject;
use tcg::Rng;

// ------------------------------------------------------------------------------------- helpers

/// (prompt, right answer, wrong answers)
type Q = (&'static str, &'static str, &'static [&'static str]);

fn choice(rng: &mut Rng, prompt: impl Into<String>, right: &str, wrong: &[&str], timed: bool) -> Task {
    let mut ws: Vec<&str> = Vec::new();
    for w in wrong.iter().copied() {
        if w != right && !ws.contains(&w) {
            ws.push(w);
        }
    }
    rng.shuffle(&mut ws);
    ws.truncate(3);
    let mut options: Vec<String> = ws.into_iter().map(String::from).collect();
    let at = rng.below(options.len() + 1);
    options.insert(at, right.to_string());
    Task::Choice { prompt: prompt.into(), options, answer: at, timed }
}

fn bank(rng: &mut Rng, b: &[Q]) -> Task {
    let (p, r, w) = *rng.pick(b);
    choice(rng, p, r, w, false)
}

/// A pair bank: ask about the first, answer the second; the wrong answers are other pairs'.
fn pairs(rng: &mut Rng, b: &[(&'static str, &'static str)], prompt: impl Fn(&str) -> String, timed: bool) -> Task {
    let (q, a) = *rng.pick(b);
    let others: Vec<&str> = b.iter().map(|p| p.1).filter(|x| *x != a).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    choice(rng, prompt(q), a, &others, timed)
}

fn sort(rng: &mut Rng, prompt: &str, bins: [&str; 2], items: &[(&str, bool)]) -> Task {
    let (item, right) = *rng.pick(items);
    Task::Sort { prompt: format!("{prompt}: {item}"), bins: [bins[0].into(), bins[1].into()], item: item.into(), right }
}

fn num(prompt: impl Into<String>, answer: i64) -> Task {
    Task::Number { prompt: prompt.into(), answer, decimals: 0, unit: String::new() }
}

fn num_u(prompt: impl Into<String>, answer: i64, unit: &str) -> Task {
    Task::Number { prompt: prompt.into(), answer, decimals: 0, unit: unit.into() }
}

fn num_d(prompt: impl Into<String>, tenths: i64, unit: &str) -> Task {
    Task::Number { prompt: prompt.into(), answer: tenths, decimals: 1, unit: unit.into() }
}

fn order(rng: &mut Rng, prompt: &str, pieces: &[&str]) -> Task {
    let mut shown: Vec<usize> = (0..pieces.len()).collect();
    for _ in 0..4 {
        rng.shuffle(&mut shown);
        if shown.iter().enumerate().any(|(k, &i)| k != i) {
            break;
        }
    }
    Task::Order { prompt: prompt.into(), pieces: pieces.iter().map(|s| s.to_string()).collect(), shown }
}

fn orders(rng: &mut Rng, b: &[(&'static str, &'static [&'static str])]) -> Task {
    let (p, pieces) = *rng.pick(b);
    order(rng, p, pieces)
}

/// Signed numbers as they're written in a Japanese maths class: (+5), (−3).
fn sg(n: i64) -> String {
    if n < 0 {
        format!("(−{})", -n)
    } else {
        format!("(+{n})")
    }
}

/// A coefficient in an expression: 3x, −x, x.
fn coef(c: i64, x: &str) -> String {
    match c {
        1 => x.to_string(),
        -1 => format!("−{x}"),
        c if c < 0 => format!("−{}{x}", -c),
        c => format!("{c}{x}"),
    }
}

/// " + 4" / " − 4"
fn plus(c: i64) -> String {
    if c < 0 {
        format!(" − {}", -c)
    } else {
        format!(" + {c}")
    }
}

fn nz(rng: &mut Rng, lo: i32, hi: i32) -> i64 {
    loop {
        let v = rng.range(lo, hi);
        if v != 0 {
            return v as i64;
        }
    }
}

// ------------------------------------------------------------------------------------- the generator

/// `n` tasks for a week's lesson (different each time; none twice in a lesson if it can help it).
pub fn tasks(w: &Week, n: usize, rng: &mut Rng) -> Vec<Task> {
    let keys: Vec<&'static str> = if w.review() { review_keys(w) } else { vec![w.key] };
    let mut out: Vec<Task> = Vec::new();
    let mut tries = 0;
    while out.len() < n && tries < n * 6 {
        tries += 1;
        let key = *rng.pick(&keys);
        let t = one(w.subject, key, rng);
        if out.iter().any(|o| o.prompt() == t.prompt()) && tries < n * 5 {
            continue;
        }
        out.push(t);
    }
    out
}

/// A test's questions: drawn from the weeks it covers.
pub fn exam_tasks(subject: Subject, weeks: &[Week], n: usize, rng: &mut Rng) -> Vec<Task> {
    let mut keys: Vec<&'static str> = weeks.iter().filter(|w| !w.review()).map(|w| w.key).collect();
    if subject == Subject::Pe {
        // (PE's paper test is health)
        keys = vec!["health"];
    }
    if keys.is_empty() {
        keys = syllabus::year_of(subject).iter().filter(|w| !w.review()).map(|w| w.key).take(1).collect();
    }
    let mut out: Vec<Task> = Vec::new();
    let mut tries = 0;
    while out.len() < n && tries < n * 6 {
        tries += 1;
        let key = *rng.pick(&keys);
        let t = one(subject, key, rng);
        if out.iter().any(|o| o.prompt() == t.prompt()) && tries < n * 5 {
            continue;
        }
        out.push(t);
    }
    out
}

/// The weeks of the year a review week goes over: the term so far ("year_review": the whole year
/// so far).
pub fn review_weeks(w: &Week) -> Vec<usize> {
    let yw = w.year_week();
    let start = if w.key == "year_review" { 1 } else { syllabus::year_week(w.term, 1) };
    let all = syllabus::year_of(w.subject);
    let mut ws: Vec<usize> = all.iter().filter(|x| (start..yw).contains(&x.year_week()) && !x.review()).map(|x| x.year_week()).collect();
    if ws.is_empty() {
        ws = all.iter().filter(|x| x.year_week() < yw && !x.review()).map(|x| x.year_week()).collect();
    }
    if ws.is_empty() {
        ws.extend(all.iter().find(|x| !x.review()).map(|x| x.year_week()));
    }
    ws
}

/// The content keys a review week goes over.
pub fn review_keys(w: &Week) -> Vec<&'static str> {
    let all = syllabus::year_of(w.subject);
    review_weeks(w).into_iter().filter_map(|n| all.iter().find(|x| x.year_week() == n).map(|x| x.key)).collect()
}

/// One task for a content key. Unknown keys fall back to the subject's first.
pub fn one(subject: Subject, key: &str, rng: &mut Rng) -> Task {
    match subject {
        Subject::Maths => maths(key, rng),
        Subject::Japanese => japanese(key, rng),
        Subject::English => english(key, rng),
        Subject::Science => science(key, rng),
        Subject::Social => social(key, rng),
        Subject::Pe => pe(key, rng),
        Subject::Music => music(key, rng),
        Subject::Art => art(key, rng),
        Subject::TechHome => techhome(key, rng),
        Subject::Moral => moral(key, rng),
        Subject::Integrated => integrated(key, rng),
        Subject::ClassActivities => class_activities(key, rng),
    }
}

/// Whether a key falls through to the subject's fallback (no content of its own): the same tasks
/// as an unknown key from the same seeds. (Tests.)
pub fn falls_back(subject: Subject, key: &str) -> bool {
    key != fallback_key(subject)
        && (1..6).all(|seed| {
            let a = one(subject, key, &mut Rng::new(seed));
            let b = one(subject, "__no_such_key__", &mut Rng::new(seed));
            a == b
        })
}

pub fn fallback_key(s: Subject) -> &'static str {
    syllabus::units(s)[0].weeks[0].1
}

// ------------------------------------------------------------------------------------- maths

fn maths(key: &str, rng: &mut Rng) -> Task {
    match key {
        "signed" => match rng.below(3) {
            0 => {
                let t = rng.range(-3, 8) as i64;
                let d = rng.range(5, 14) as i64;
                num_u(format!("It's {t}°C at dawn. By night it's fallen {d}°C. What is it now?"), t - d, "°C")
            }
            1 => {
                let a = nz(rng, -15, 15);
                num(format!("What is the opposite of {}? (the number the same distance the other side of 0)", sg(a)), -a)
            }
            _ => {
                let a = rng.range(1, 9) as i64;
                num_u(format!("A rise of +{a} m is written +{a}. A fall of {a} m is written how? (dial the number)"), -a, "m")
            }
        },
        "abs" => match rng.below(3) {
            0 => {
                let a = nz(rng, -20, 20);
                num(format!("The absolute value of {} is?", sg(a)), a.abs())
            }
            1 => {
                let a = rng.range(2, 6) as i64;
                num(format!("How many whole numbers have an absolute value less than {a}?"), 2 * a - 1)
            }
            _ => {
                let mut v: Vec<i64> = (0..4).map(|_| nz(rng, -12, 12)).collect();
                v.dedup();
                let lo = *v.iter().min().unwrap();
                let opts: Vec<String> = v.iter().map(|&x| sg(x)).collect();
                let right = sg(lo);
                let wrong: Vec<&str> = opts.iter().map(|s| s.as_str()).filter(|s| *s != right).collect();
                choice(rng, "Which is the smallest?", &right, &wrong, false)
            }
        },
        "addsub" => {
            let (a, b) = (nz(rng, -20, 20), nz(rng, -20, 20));
            if rng.chance(0.5) {
                num(format!("{} + {} =", sg(a), sg(b)), a + b)
            } else {
                num(format!("{} − {} =", sg(a), sg(b)), a - b)
            }
        }
        "muldiv" => {
            let (a, b) = (nz(rng, -9, 9), nz(rng, -9, 9));
            if rng.chance(0.5) {
                num(format!("{} × {} =", sg(a), sg(b)), a * b)
            } else {
                num(format!("{} ÷ {} =", sg(a * b), sg(b)), a)
            }
        }
        "powers" => match rng.below(3) {
            0 => {
                let a = rng.range(2, 5) as i64;
                num(format!("(−{a})² ="), a * a)
            }
            1 => {
                let a = rng.range(2, 4) as i64;
                num(format!("−{a}³ ="), -(a * a * a))
            }
            _ => {
                let (a, b, c) = (nz(rng, -9, 9), rng.range(2, 6) as i64, nz(rng, -6, 6));
                num(format!("{a} + {b} × {} =", sg(c)), a + b * c)
            }
        },
        "primes" => match rng.below(2) {
            0 => {
                let ps = [2i64, 3, 5, 7];
                let (p, q, r) = (*rng.pick(&ps), *rng.pick(&ps), *rng.pick(&[11i64, 13, 7, 5]));
                let n = p * q * r;
                num(format!("Factorise {n} into primes. Its largest prime factor is?"), p.max(q).max(r))
            }
            _ => {
                let a = *rng.pick(&[12i64, 18, 20, 28, 45, 50, 63, 75]);
                // the smallest number to multiply by to make a square
                let mut m = 1;
                let mut x = a;
                let mut p = 2;
                while x > 1 {
                    let mut c = 0;
                    while x % p == 0 {
                        x /= p;
                        c += 1;
                    }
                    if c % 2 == 1 {
                        m *= p;
                    }
                    p += 1;
                }
                num(format!("What is the smallest whole number to multiply {a} by to make a square number?"), m)
            }
        },
        "letters" => match rng.below(3) {
            0 => {
                let (p, a, e) = (*rng.pick(&[80i64, 90, 110, 120, 150]), rng.range(2, 6) as i64, *rng.pick(&[50i64, 60, 100]));
                num_u(format!("Pens cost {p} yen each and an eraser {e} yen. The total for a pens and an eraser is {p}a + {e}. With a = {a}, how much?"), p * a + e, "yen")
            }
            1 => {
                let (x, n) = (rng.range(3, 9) as i64, rng.range(2, 5) as i64);
                num_u(format!("A square has sides of x cm. Its perimeter is 4x cm. With x = {x}, and {n} squares in a row (not touching), the total perimeter?"), 4 * x * n, "cm")
            }
            _ => {
                let (a, b) = (rng.range(2, 9) as i64, rng.range(2, 9) as i64);
                num(format!("Written with letters, a × {a} × b becomes {a}ab. With a = {b}, b = −1, what is {a}ab?"), -(a * b))
            }
        },
        "substitute" => {
            let x = nz(rng, -5, 5);
            match rng.below(3) {
                0 => {
                    let (a, b) = (nz(rng, -6, 6), rng.range(-9, 9) as i64);
                    num(format!("x = {x}: {}{} =", coef(a, "x"), plus(b)), a * x + b)
                }
                1 => num(format!("x = {x}: x² ="), x * x),
                _ => {
                    let a = rng.range(2, 5) as i64;
                    num(format!("x = {x}: −{a}x² ="), -a * x * x)
                }
            }
        }
        "like_terms" => {
            let (a, mut b, c, d) = (nz(rng, -8, 8), nz(rng, -8, 8), rng.range(-9, 9) as i64, rng.range(-9, 9) as i64);
            if a + b == 0 {
                b += if b > 0 { 1 } else { -1 };
            }
            if rng.chance(0.5) {
                num(format!("{}{}{}{} = □x{}: □ =", coef(a, "x"), plus(c), if b < 0 { " − ".to_string() + &coef(-b, "x") } else { " + ".to_string() + &coef(b, "x") }, plus(d), plus(c + d)), a + b)
            } else {
                num(format!("{}{}{}{} = {}{} + □: □ =", coef(a, "a"), plus(c), if b < 0 { " − ".to_string() + &coef(-b, "a") } else { " + ".to_string() + &coef(b, "a") }, plus(d), coef(a + b, "a"), ""), c + d)
            }
        }
        "expand" => {
            let (k, a, b) = (nz(rng, -6, 6), nz(rng, -5, 5), nz(rng, -9, 9));
            if rng.chance(0.5) {
                num(format!("{k}({}{}) = □x{}: □ =", coef(a, "x"), plus(b), plus(k * b)), k * a)
            } else {
                num(format!("{k}({}{}) = {} + □: □ =", coef(a, "x"), plus(b), coef(k * a, "x")), k * b)
            }
        }
        "relations" => {
            let b: &[Q] = &[
                ("x pencils at 60 yen cost less than 500 yen. Which is right?", "60x < 500", &["60x > 500", "60 + x < 500", "60x = 500"]),
                ("a is at least 3 more than b.", "a ≥ b + 3", &["a ≤ b + 3", "a = 3b", "a > 3b"]),
                ("Sharing 200 sheets among x people gives y each and 8 left over.", "200 = xy + 8", &["200 = x + y + 8", "200 − 8 = x + y", "y = 200x + 8"]),
                ("Walking a km at 4 km/h takes b hours.", "a = 4b", &["b = 4a", "a + 4 = b", "ab = 4"]),
                ("x is 5 less than twice y.", "x = 2y − 5", &["x = 2(y − 5)", "x = 5 − 2y", "2x = y − 5"]),
            ];
            bank(rng, b)
        }
        "eq_basic" => {
            let x = nz(rng, -9, 9);
            match rng.below(3) {
                0 => {
                    let a = nz(rng, -12, 12);
                    num(format!("x{} = {}: x =", plus(a), x + a), x)
                }
                1 => {
                    let a = nz(rng, -7, 7);
                    num(format!("{} = {}: x =", coef(a, "x"), a * x), x)
                }
                _ => {
                    let (a, b) = (nz(rng, 2, 6), rng.range(-9, 9) as i64);
                    num(format!("{}{} = {}: x =", coef(a, "x"), plus(b), a * x + b), x)
                }
            }
        }
        "eq_both" => {
            let x = nz(rng, -8, 8);
            let (a, c) = (nz(rng, -6, 6), nz(rng, -6, 6));
            let c = if c == a { a + 1 } else { c };
            let b = rng.range(-12, 12) as i64;
            let d = a * x + b - c * x;
            num(format!("{}{} = {}{}: x =", coef(a, "x"), plus(b), coef(c, "x"), plus(d)), x)
        }
        "eq_brackets" => {
            let x = nz(rng, -6, 8);
            let (k, b, c) = (nz(rng, 2, 4), nz(rng, -5, 5), rng.range(-9, 9) as i64);
            // k(x + b) = x + c'  with c' chosen to fit
            let rhs = k * (x + b) - x;
            let _ = c;
            num(format!("{k}(x{}) = x{}: x =", plus(b), plus(rhs)), x)
        }
        "eq_fractions" => {
            let d = *rng.pick(&[2i64, 3, 4, 5]);
            let x = d * nz(rng, -5, 6);
            if rng.chance(0.5) {
                let b = rng.range(-6, 6) as i64;
                num(format!("x/{d}{} = {}: x =", plus(b), x / d + b), x)
            } else {
                // decimals: 0.3x − 1.2 = 0.x ... kept to tenths
                let (a, b) = (nz(rng, 2, 5), nz(rng, -9, 9));
                let x = nz(rng, -6, 6);
                num(format!("0.{a}x{} = {}: x =", if b < 0 { format!(" − {}", show_tenths(-b)) } else { format!(" + {}", show_tenths(b)) }, show_tenths(a * x + b)), x)
            }
        }
        "eq_word" => match rng.below(3) {
            0 => {
                let (p, x, paid) = (*rng.pick(&[60i64, 80, 90, 120]), rng.range(3, 8) as i64, 1000i64);
                num_u(format!("You buy x notebooks at {p} yen each, pay {paid} yen and get {} yen change. x =", paid - p * x), x, "notebooks")
            }
            1 => {
                let (n, k, left) = (rng.range(4, 9) as i64, rng.range(3, 6) as i64, rng.range(1, 5) as i64);
                let total = n * k + left;
                num_u(format!("Giving each student {k} cards leaves {left} over from {total}. How many students?"), n, "students")
            }
            _ => {
                let (age, years) = (rng.range(10, 14) as i64, rng.range(2, 6) as i64);
                let parent = 3 * (age + years) - years;
                num_u(format!("You're {age}. In how many years will your {parent}-year-old uncle be three times your age?"), years, "years")
            }
        },
        "ratio" => {
            let (a, b, k) = (rng.range(2, 7) as i64, rng.range(2, 7) as i64, rng.range(2, 6) as i64);
            num(format!("x : {} = {} : {b}  →  x =", b * k, a), a * k)
        }
        "proportion" => {
            let a = nz(rng, -5, 5);
            let x0 = nz(rng, 1, 6);
            let x1 = nz(rng, -8, 8);
            num(format!("y is proportional to x, and y = {} when x = {x0}. y when x = {x1}?", a * x0), a * x1)
        }
        "coordinates" => match rng.below(3) {
            0 => {
                let (x, y) = (nz(rng, -6, 6), nz(rng, -6, 6));
                num(format!("Reflect the point ({x}, {y}) in the x-axis. Its y-coordinate?"), -y)
            }
            1 => {
                let (x, y) = (nz(rng, -6, 6), nz(rng, -6, 6));
                num(format!("Reflect ({x}, {y}) through the origin. Its x-coordinate?"), -x)
            }
            _ => {
                let (x, y) = (nz(rng, -6, 6), nz(rng, -6, 6));
                let q = match (x > 0, y > 0) {
                    (true, true) => "1st quadrant",
                    (false, true) => "2nd quadrant",
                    (false, false) => "3rd quadrant",
                    (true, false) => "4th quadrant",
                };
                choice(rng, format!("Where is the point ({x}, {y})?"), q, &["1st quadrant", "2nd quadrant", "3rd quadrant", "4th quadrant"], false)
            }
        },
        "graph_direct" => {
            let a = nz(rng, -4, 4);
            let x = nz(rng, -5, 5);
            if rng.chance(0.6) {
                num(format!("The graph of y = {}x passes through ({x}, □). □ =", a), a * x)
            } else {
                num(format!("A straight line through the origin passes through ({x}, {}). It is y = □x. □ =", a * x), a)
            }
        }
        "inverse" => {
            let a = *rng.pick(&[12i64, 18, 24, 30, 36, 48, 60]);
            let x0 = *rng.pick(&[2i64, 3, 4, 6]);
            let ds: Vec<i64> = (1..=a).filter(|d| a % d == 0).flat_map(|d| [d, -d]).collect();
            let x1 = *rng.pick(&ds);
            if rng.chance(0.5) {
                num(format!("y is inversely proportional to x, and y = {} when x = {x0}. y = a/x: a =", a / x0), a)
            } else {
                num(format!("y = {a}/x. y when x = {x1}?"), a / x1)
            }
        }
        "inverse_graph" => {
            let b: &[Q] = &[
                ("The graph of y = a/x is called a...", "hyperbola", &["parabola", "straight line", "circle"]),
                ("For y = 12/x, as x gets bigger (x > 0), y...", "gets smaller", &["gets bigger", "stays the same", "becomes negative"]),
                ("Sharing 24 cakes among x people, y each: y is...", "inversely proportional to x", &["proportional to x", "equal to x", "neither"]),
                ("The graph of y = −3x goes...", "down to the right", &["up to the right", "flat", "through (1, 3)"]),
            ];
            if rng.chance(0.5) {
                bank(rng, b)
            } else {
                let (s, t) = (rng.range(2, 6) as i64 * 10, rng.range(2, 5) as i64);
                num_u(format!("Taps fill a tank in {t} hours using {s} L a minute. How many minutes with {} L a minute?", s * 2), t * 60 / 2, "min")
            }
        }
        "angles" => match rng.below(3) {
            0 => {
                let a = rng.range(20, 160) as i64;
                num_u(format!("Two straight lines cross. One angle is {a}°. The angle next to it on the line?"), 180 - a, "°")
            }
            1 => {
                let a = rng.range(20, 160) as i64;
                num_u(format!("Two straight lines cross. One angle is {a}°. The vertically opposite angle?"), a, "°")
            }
            _ => {
                let b: &[Q] = &[
                    ("A line that goes on forever in one direction only", "ray (hansenbun)", &["segment", "line", "arc"]),
                    ("Part of a line between two points", "segment (senbun)", &["ray", "line", "chord"]),
                    ("Two lines meeting at 90° are...", "perpendicular (AB ⊥ CD)", &["parallel (AB ∥ CD)", "equal", "opposite"]),
                    ("The shortest distance from a point to a line is along...", "the perpendicular", &["a parallel", "any segment", "the arc"]),
                ];
                bank(rng, b)
            }
        },
        "transform" => match rng.below(2) {
            0 => {
                let (x, y, dx, dy) = (rng.range(-5, 5) as i64, rng.range(-5, 5) as i64, nz(rng, -4, 4), nz(rng, -4, 4));
                num(format!("Translate ({x}, {y}) by {} {} and {} {}. Its new x?", dx.abs(), if dx > 0 { "right" } else { "left" }, dy.abs(), if dy > 0 { "up" } else { "down" }), x + dx)
            }
            _ => {
                let b: &[Q] = &[
                    ("Turning a shape 180° about a point", "point-symmetric move (rotation by 180°)", &["translation", "reflection", "enlargement"]),
                    ("Sliding a shape without turning it", "translation", &["rotation", "reflection", "enlargement"]),
                    ("Flipping a shape over a line", "reflection (line symmetry)", &["translation", "rotation", "enlargement"]),
                    ("In a reflection, the line joining a point to its image is ... to the mirror line", "perpendicular", &["parallel", "equal", "at 45°"]),
                ];
                bank(rng, b)
            }
        },
        "constructions" => {
            let b: &[(&'static str, &'static [&'static str])] = &[
                ("Bisect the segment AB: put the steps in order", &["Arc from A, radius more than half AB", "Same arc from B", "Mark where the arcs cross (two points)", "Join the two crossing points"]),
                ("Bisect the angle AOB: put the steps in order", &["Arc from O cutting OA and OB", "Arcs of the same radius from those two points", "Mark where they cross", "Draw the ray from O through it"]),
                ("Draw the perpendicular from P to line ℓ: in order", &["Arc from P cutting ℓ twice", "Equal arcs from those two points", "Mark where they cross", "Join P to the crossing"]),
            ];
            if rng.chance(0.7) {
                orders(rng, b)
            } else {
                choice(rng, "Points the same distance from A and B lie on...", "the perpendicular bisector of AB", &["the angle bisector", "a circle through A", "the line AB"], false)
            }
        }
        "circles" => {
            let b: &[Q] = &[
                ("A tangent to a circle meets the radius at the point of contact at...", "90°", &["45°", "180°", "60°"]),
                ("A segment joining two points on a circle", "chord (gen)", &["arc", "tangent", "radius"]),
                ("Part of the circle between two points", "arc (ko)", &["chord", "sector", "diameter"]),
                ("The centre of a circle through A, B and C is where ... cross", "the perpendicular bisectors of AB and BC", &["the angle bisectors", "the medians", "the tangents"]),
            ];
            if rng.chance(0.6) {
                bank(rng, b)
            } else {
                let r = rng.range(2, 9) as i64;
                num(format!("A circle of radius {r} cm has circumference □π cm. □ ="), 2 * r)
            }
        }
        "sector" => {
            let r = *rng.pick(&[3i64, 4, 6, 9, 12]);
            let a = *rng.pick(&[30i64, 60, 90, 120, 180, 240]);
            if (2 * r * a) % 360 == 0 && rng.chance(0.5) {
                num(format!("A sector, radius {r} cm, angle {a}°: its arc is □π cm. □ ="), 2 * r * a / 360)
            } else if (r * r * a) % 360 == 0 {
                num(format!("A sector, radius {r} cm, angle {a}°: its area is □π cm². □ ="), r * r * a / 360)
            } else {
                num(format!("A sector, radius 6 cm, angle 60°: its area is □π cm². □ ="), 6)
            }
        }
        "solids" => {
            let n = rng.range(3, 8) as i64;
            let names = ["", "", "", "triangular", "square", "pentagonal", "hexagonal", "heptagonal", "octagonal"];
            match rng.below(4) {
                0 => num(format!("How many faces has a {} prism?", names[n as usize]), n + 2),
                1 => num(format!("How many edges has a {} prism?", names[n as usize]), 3 * n),
                2 => num(format!("How many vertices has a {} pyramid?", names[n as usize]), n + 1),
                _ => num("A regular polyhedron with 20 equilateral-triangle faces: how many vertices?", 12),
            }
        }
        "nets" => {
            let b: &[Q] = &[
                ("The curved side of a cylinder's net is a...", "rectangle", &["sector", "circle", "triangle"]),
                ("The curved side of a cone's net is a...", "sector", &["rectangle", "triangle", "circle"]),
                ("Two lines in space that never meet and aren't parallel are...", "skew (nejire no ichi)", &["perpendicular", "parallel", "crossing"]),
                ("Spinning a rectangle about one side makes a...", "cylinder", &["cone", "sphere", "prism"]),
                ("Spinning a right triangle about one leg makes a...", "cone", &["cylinder", "sphere", "pyramid"]),
                ("A plane and a line on it are...", "the line lies in the plane", &["parallel", "skew", "perpendicular"]),
            ];
            bank(rng, b)
        }
        "surface" => {
            let a = rng.range(2, 9) as i64;
            if rng.chance(0.5) {
                num_u(format!("A cube with edges of {a} cm: its surface area?"), 6 * a * a, "cm²")
            } else {
                let (r, h) = (rng.range(2, 6) as i64, rng.range(3, 10) as i64);
                num(format!("A cylinder, radius {r} cm, height {h} cm: total surface area is □π cm². □ ="), 2 * r * r + 2 * r * h)
            }
        }
        "volume" => {
            if rng.chance(0.5) {
                let (a, b, h) = (rng.range(2, 9) as i64, rng.range(2, 9) as i64, rng.range(2, 10) as i64);
                num_u(format!("A triangular prism: its base is a right triangle with legs {a} cm and {} cm, height {h} cm. Volume?", 2 * b), a * b * h, "cm³")
            } else {
                let (r, h) = (rng.range(2, 6) as i64, rng.range(2, 10) as i64);
                num(format!("A cylinder, radius {r} cm, height {h} cm: volume is □π cm³. □ ="), r * r * h)
            }
        }
        "volume2" => match rng.below(3) {
            0 => {
                let (r, h) = (rng.range(2, 6) as i64, 3 * rng.range(1, 4) as i64);
                num(format!("A cone, radius {r} cm, height {h} cm: volume is □π cm³. □ ="), r * r * h / 3)
            }
            1 => {
                let r = 3 * rng.range(1, 3) as i64;
                num(format!("A sphere of radius {r} cm: volume is □π cm³. □ ="), 4 * r * r * r / 3)
            }
            _ => {
                let (a, h) = (rng.range(2, 7) as i64, 3 * rng.range(1, 4) as i64);
                num_u(format!("A square pyramid, base {a} cm by {a} cm, height {h} cm: volume?"), a * a * h / 3, "cm³")
            }
        },
        "frequency" => {
            let w = *rng.pick(&[5i64, 10]);
            let lo = w * rng.range(2, 8) as i64;
            if rng.chance(0.5) {
                num(format!("In a frequency table, the class {lo} or more and under {}: its class value (the middle)?", lo + w), lo + w / 2)
            } else {
                let f: Vec<i64> = (0..4).map(|_| rng.range(1, 9) as i64).collect();
                num(format!("Frequencies in a table: {}, {}, {}, {}. The cumulative frequency of the third class?", f[0], f[1], f[2], f[3]), f[0] + f[1] + f[2])
            }
        }
        "averages" => {
            let n = 5;
            let mut v: Vec<i64> = (0..n).map(|_| rng.range(1, 10) as i64).collect();
            match rng.below(3) {
                0 => {
                    // make the mean whole
                    let s: i64 = v.iter().sum();
                    v[0] += (n as i64 - s % n as i64) % n as i64;
                    let s: i64 = v.iter().sum();
                    num(format!("Scores: {}. The mean?", list(&v)), s / n as i64)
                }
                1 => {
                    let mut w = v.clone();
                    w.sort();
                    num(format!("Scores: {}. The median?", list(&v)), w[n / 2])
                }
                _ => num(format!("Scores: {}. The range?", list(&v)), v.iter().max().unwrap() - v.iter().min().unwrap()),
            }
        }
        "relfreq" => {
            let (total, step) = *rng.pick(&[(20i64, 5i64), (25, 4), (40, 5), (50, 2)]);
            let p = step * rng.range(2, (90 / step) as i32) as i64;
            num_u(format!("{} of {total} students walk to school. The relative frequency, as a percentage?", total * p / 100), p, "%")
        }
        "probability" => {
            let n = *rng.pick(&[200i64, 500, 1000]);
            let p = rng.range(15, 70) as i64;
            if rng.chance(0.6) {
                num_u(format!("A bottle cap thrown {n} times lands face up {} times. Relative frequency ×100?", n * p / 100), p, "%")
            } else {
                num(format!("A drawing pin lands point-up with relative frequency 0.{p:02}. In 1000 throws, about how many point-up?"), p * 10)
            }
        }
        _ => maths("signed", rng),
    }
}

fn show_tenths(t: i64) -> String {
    super::play::show_number(t, 1)
}

fn list(v: &[i64]) -> String {
    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
}


// ------------------------------------------------------------------------------------- Japanese

const KANJI1: &[(&str, &str)] = &[
    ("鍛える", "きたえる"), ("緊張", "きんちょう"), ("距離", "きょり"), ("微妙", "びみょう"), ("偶然", "ぐうぜん"), ("披露", "ひろう"),
    ("抱える", "かかえる"), ("輝く", "かがやく"), ("詳しい", "くわしい"), ("扱う", "あつかう"), ("震える", "ふるえる"), ("恐怖", "きょうふ"),
    ("返却", "へんきゃく"), ("把握", "はあく"), ("躍動", "やくどう"), ("繊細", "せんさい"),
];
const KANJI2: &[(&str, &str)] = &[
    ("抽象", "ちゅうしょう"), ("網羅", "もうら"), ("頻繁", "ひんぱん"), ("膨大", "ぼうだい"), ("描写", "びょうしゃ"), ("脅かす", "おびやかす"),
    ("滑らか", "なめらか"), ("鮮やか", "あざやか"), ("怠る", "おこたる"), ("妨げる", "さまたげる"), ("矛盾", "むじゅん"), ("顕著", "けんちょ"),
    ("循環", "じゅんかん"), ("慌てる", "あわてる"), ("驚異", "きょうい"), ("摘む", "つむ"),
];
const KANJI3: &[(&str, &str)] = &[
    ("貢献", "こうけん"), ("邪魔", "じゃま"), ("岬", "みさき"), ("沼", "ぬま"), ("峠", "とうげ"), ("稲穂", "いなほ"),
    ("霧", "きり"), ("嵐", "あらし"), ("拍手", "はくしゅ"), ("翻訳", "ほんやく"), ("凝る", "こる"), ("潜る", "もぐる"),
    ("誘う", "さそう"), ("勧める", "すすめる"), ("唇", "くちびる"), ("眺める", "ながめる"),
];

fn kanji_reading(rng: &mut Rng, b: &[(&'static str, &'static str)]) -> Task {
    pairs(rng, b, |k| format!("読み方は? 「{k}」"), true)
}

fn japanese(key: &str, rng: &mut Rng) -> Task {
    match key {
        "kanji1" => kanji_reading(rng, KANJI1),
        "kanji2" => kanji_reading(rng, KANJI2),
        "kanji3" => kanji_reading(rng, KANJI3),
        "kanji_all" => {
            let all: Vec<(&str, &str)> = KANJI1.iter().chain(KANJI2).chain(KANJI3).copied().collect();
            kanji_reading(rng, &all)
        }
        "radicals" => {
            let b: &[(&str, &str)] = &[
                ("清", "さんずい"), ("持", "てへん"), ("花", "くさかんむり"), ("体", "にんべん"), ("話", "ごんべん"), ("村", "きへん"), ("道", "しんにょう"), ("国", "くにがまえ"),
                ("思", "こころ"), ("病", "やまいだれ"), ("空", "あなかんむり"), ("顔", "おおがい"), ("線", "いとへん"), ("時", "ひへん"), ("雪", "あめかんむり"), ("開", "もんがまえ"),
            ];
            pairs(rng, b, |k| format!("部首は? 「{k}」"), true)
        }
        "strokes" => {
            let b: &[(&str, i64)] = &[("必", 5), ("飛", 9), ("右", 5), ("書", 10), ("馬", 10), ("区", 4), ("世", 5), ("発", 9), ("様", 14), ("達", 12), ("緑", 14), ("極", 12), ("郵", 11), ("券", 8), ("版", 8)];
            let (k, n) = *rng.pick(b);
            num(format!("「{k}」の総画数は?"), n)
        }
        "onkun" => {
            let items: &[(&str, bool)] = &[
                ("山（サン）", false), ("山（やま）", true), ("水（スイ）", false), ("水（みず）", true), ("生（セイ）", false), ("生（いきる）", true), ("海（カイ）", false), ("海（うみ）", true),
                ("花（カ）", false), ("花（はな）", true), ("森（シン）", false), ("森（もり）", true), ("雨（ウ）", false), ("雨（あめ）", true), ("学（ガク）", false), ("学（まなぶ）", true),
            ];
            sort(rng, "音読み？訓読み？", ["音読み", "訓読み"], items)
        }
        "bunsetsu" => {
            let b: &[(&str, i64)] = &[
                ("私は毎朝公園を走る。", 4), ("妹が大きな声で笑った。", 4), ("今日は空がとても青い。", 4), ("雨が降る。", 2), ("小さな花が庭に咲いた。", 4), ("先生は黒板に字を書いた。", 4),
                ("明日は友達と海へ行く。", 4), ("鳥が空を飛ぶ。", 3), ("白い猫がゆっくり歩く。", 4), ("駅前の店で本を買った。", 4), ("弟はまだ小学生です。", 3), ("この道はとても長い。", 4),
            ];
            let (s, n) = *rng.pick(b);
            num(format!("文節はいくつ? 「{s}」"), n)
        }
        "subject_pred" => {
            let b: &[(&str, &str, &str, &[&str])] = &[
                ("昨日、兄が駅で友達に会った。", "主語", "兄が", &["昨日", "駅で", "会った"]),
                ("庭の花がきれいに咲いた。", "主語", "花が", &["庭の", "きれいに", "咲いた"]),
                ("私も明日の大会に出る。", "主語", "私も", &["明日の", "大会に", "出る"]),
                ("山の上から海がよく見える。", "主語", "海が", &["山の", "上から", "見える"]),
                ("妹は毎日ピアノを練習する。", "述語", "練習する", &["妹は", "毎日", "ピアノを"]),
                ("夕方の空は本当に美しい。", "述語", "美しい", &["夕方の", "空は", "本当に"]),
                ("ぼくの夢は先生だ。", "述語", "先生だ", &["ぼくの", "夢は", "ぼくの夢は"]),
            ];
            let (s, which, r, w) = *rng.pick(b);
            choice(rng, format!("{which}はどれ? 「{s}」"), r, w, false)
        }
        "pos" => {
            let b: &[(&str, &str)] = &[
                ("走る", "動詞"), ("美しい", "形容詞"), ("静かだ", "形容動詞"), ("ゆっくり", "副詞"), ("学校", "名詞"), ("この", "連体詞"), ("しかし", "接続詞"), ("ああ", "感動詞"),
                ("とても", "副詞"), ("大きな", "連体詞"), ("読む", "動詞"), ("楽しい", "形容詞"), ("元気だ", "形容動詞"), ("だから", "接続詞"), ("山", "名詞"), ("おはよう", "感動詞"),
            ];
            pairs(rng, b, |k| format!("品詞は? 「{k}」"), false)
        }
        "poem" => {
            let b: &[(&str, &str)] = &[
                ("雪のように白い手", "直喩"), ("君は僕の太陽だ", "隠喩"), ("風が窓をたたいている", "擬人法"), ("走れ、走れ、どこまでも", "反復"),
                ("忘れないよ、あの夏の日を", "倒置"), ("夕焼けの空に響く鐘", "体言止め"), ("山が笑っている", "擬人法"), ("星くずのような町の灯", "直喩"),
            ];
            pairs(rng, b, |k| format!("表現の技法は? 「{k}」"), false)
        }
        "story" => {
            let b: &[Q] = &[
                ("「ハルは答案を裏返したまま、しばらく動かなかった。窓の外で、運動部のかけ声だけが明るく響いていた。」ハルの気持ちに近いのは?", "結果を見るのがこわい", &["早く遊びに行きたい", "友達に腹を立てている", "とても眠い"]),
                ("「転校生の席は窓際だった。ユイは消しゴムを二つに割って、片方をそっと差し出した。」ユイがしたことから分かるのは?", "転校生と仲良くなりたい", &["消しゴムをなくした", "先生にしかられた", "席を替わりたい"]),
                ("「雨の音が強くなるたび、祖母は縁側の戸を見た。誰も来ないとわかっていても、夕方になると毎日そうするのだった。」祖母の様子から分かるのは?", "誰かを待ち続けている", &["雨が好きだ", "戸がこわれている", "眠くなっている"]),
                ("「ゴールの直前、ケンは振り返った。最後尾のタクが、足を引きずりながら走っていた。ケンは速度を落とした。」ケンが速度を落としたのはなぜ?", "タクのことが気になった", &["つかれて走れなくなった", "一位になりたかった", "道をまちがえた"]),
                ("物語で、人物の気持ちが直接書かれず、行動や情景で表されているとき、読み手は何をする?", "行動や情景から気持ちを想像する", &["気持ちは書いていないと考える", "作者に聞く", "読み飛ばす"]),
            ];
            bank(rng, b)
        }
        "speech" => {
            let b: &[(&'static str, &'static [&'static str])] = &[
                ("スピーチの組み立て:順に並べよう", &["あいさつ", "名前を言う", "話題を示す", "具体的な例を話す", "まとめ", "お礼を言う"]),
                ("発表の準備:順に並べよう", &["話すことを決める", "メモを作る", "声に出して練習する", "時間を計る", "本番で話す"]),
            ];
            orders(rng, b)
        }
        "fact_opinion" => {
            let items: &[(&str, bool)] = &[
                ("富士山の高さは3776メートルだ。", false), ("夏休みはもっと長いほうがいい。", true), ("水は100度でふっとうする。", false), ("この町の夕日は日本一美しい。", true),
                ("日本には47の都道府県がある。", false), ("朝ご飯は毎日食べるべきだ。", true), ("昨日の気温は30度だった。", false), ("スマートフォンは中学生には早いと思う。", true),
            ];
            sort(rng, "事実? 意見?", ["事実 (fact)", "意見 (opinion)"], items)
        }
        "report" => {
            let b: &[(&'static str, &'static [&'static str])] = &[
                ("レポートを書く手順:順に並べよう", &["テーマを決める", "調べる方法を考える", "調べてメモを取る", "構成を考える", "下書きを書く", "読み返して清書する"]),
                ("作文の推敲:順に並べよう", &["声に出して読む", "分かりにくい所に線を引く", "言葉を直す", "清書する"]),
            ];
            orders(rng, b)
        }
        "opinion" => {
            let b: &[(&'static str, &'static [&'static str])] = &[("意見文の構成:順に並べよう", &["主張を書く", "理由を書く", "具体例を挙げる", "反対意見にふれる", "もう一度主張をまとめる"])];
            if rng.chance(0.6) {
                orders(rng, b)
            } else {
                choice(rng, "意見文で説得力を高めるのは?", "根拠となる事実やデータ", &["感嘆符をたくさん使う", "長い文にする", "難しい言葉だけを使う"], false)
            }
        }
        "paragraphs" => {
            if rng.chance(0.4) {
                let b: &[(&'static str, &'static [&'static str])] = &[("説明文の構成:順に並べよう", &["序論:問題を示す", "本論:例を挙げて説明する", "本論:理由を述べる", "結論:まとめる"])];
                orders(rng, b)
            } else {
                let b: &[(&str, &str)] = &[("つまり", "言いかえ・まとめ"), ("しかし", "逆接"), ("だから", "順接"), ("例えば", "例示"), ("さらに", "添加"), ("ところで", "転換")];
                pairs(rng, b, |k| format!("「{k}」のはたらきは?"), false)
            }
        }
        "discussion" => {
            let b: &[Q] = &[
                ("相手の意見に反対するとき、よい言い方は?", "「〜という考えもあると思います」", &["「それは違う」", "「意味がわからない」", "何も言わない"]),
                ("話し合いで司会がすることは?", "意見をまとめて進める", &["自分の意見だけを言う", "時間を気にしない", "全部決めてしまう"]),
                ("相手の話を聞くときに大切なのは?", "うなずいて最後まで聞く", &["途中で話す", "下を向く", "別のことをする"]),
            ];
            bank(rng, b)
        }
        "haiku" => {
            let b: &[(&str, &str, &str)] = &[
                ("古池や蛙飛びこむ水の音（芭蕉）", "春", "蛙"), ("閑さや岩にしみ入る蝉の声（芭蕉）", "夏", "蝉"), ("柿くへば鐘が鳴るなり法隆寺（子規）", "秋", "柿"),
                ("菜の花や月は東に日は西に（蕪村）", "春", "菜の花"), ("いくたびも雪の深さを尋ねけり（子規）", "冬", "雪"), ("やれ打つな蠅が手をすり足をする（一茶）", "夏", "蠅"),
                ("名月を取ってくれろと泣く子かな（一茶）", "秋", "名月"), ("五月雨をあつめて早し最上川（芭蕉）", "夏", "五月雨"), ("春の海終日のたりのたりかな（蕪村）", "春", "春の海"),
            ];
            let (h, season, kigo) = *rng.pick(b);
            if rng.chance(0.5) {
                choice(rng, format!("季節は? 「{h}」"), season, &["春", "夏", "秋", "冬"], false)
            } else {
                let others: Vec<&str> = b.iter().map(|x| x.2).filter(|k| *k != kigo).collect();
                choice(rng, format!("季語は? 「{h}」"), kigo, &others, false)
            }
        }
        "jukugo" => {
            let b: &[(&str, &str)] = &[
                ("森林", "似た意味の組み合わせ"), ("岩石", "似た意味の組み合わせ"), ("上下", "反対の意味の組み合わせ"), ("強弱", "反対の意味の組み合わせ"), ("青空", "上が下を修飾"), ("急行", "上が下を修飾"),
                ("読書", "下が上の目的・対象"), ("登山", "下が上の目的・対象"), ("日没", "主語と述語"), ("国立", "主語と述語"),
            ];
            pairs(rng, b, |k| format!("熟語の組み立ては? 「{k}」"), false)
        }
        "kotowaza" => {
            let b: &[(&str, &str)] = &[
                ("猿も木から", "落ちる"), ("石の上にも", "三年"), ("急がば", "回れ"), ("花より", "団子"), ("七転び", "八起き"), ("塵も積もれば", "山となる"), ("猫に", "小判"), ("百聞は", "一見にしかず"), ("早起きは", "三文の徳"), ("知らぬが", "仏"),
            ];
            pairs(rng, b, |k| format!("ことわざを完成させよう: 「{k}〜」"), false)
        }
        "yoji" => {
            let b: &[(&str, &str)] = &[("一石二□", "鳥"), ("十人十□", "色"), ("一期一□", "会"), ("自業自□", "得"), ("以心伝□", "心"), ("試行錯□", "誤"), ("起承転□", "結"), ("温故知□", "新"), ("電光石□", "火"), ("一日千□", "秋")];
            pairs(rng, b, |k| format!("四字熟語: 「{k}」の□は?"), true)
        }
        "homophones" => {
            let b: &[(&str, &str, &[&str])] = &[
                ("「カイホウ」感にひたる", "解放", &["開放", "快方", "介抱"]), ("病気が「カイホウ」に向かう", "快方", &["解放", "開放", "介抱"]), ("窓を「カイホウ」する", "開放", &["解放", "快方", "介抱"]),
                ("「タイショウ」的な性格の二人", "対照", &["対象", "対称"]), ("中学生を「タイショウ」にした本", "対象", &["対照", "対称"]), ("左右「タイショウ」の図形", "対称", &["対象", "対照"]),
                ("時間を「ハカ」る", "計", &["測", "量", "図"]), ("体重を「ハカ」る", "量", &["計", "測", "図"]), ("問題の解決を「ハカ」る", "図", &["計", "測", "量"]),
            ];
            let (s, r, w) = *rng.pick(b);
            choice(rng, format!("正しい漢字は? {s}"), r, w, false)
        }
        "kana_old" => {
            let b: &[(&str, &str)] = &[
                ("いふ", "いう"), ("ゐる", "いる"), ("をかし", "おかし"), ("あはれ", "あわれ"), ("かほ", "かお"), ("けふ", "きょう"), ("てふてふ", "ちょうちょう"), ("やうやう", "ようよう"),
                ("なほ", "なお"), ("こゑ", "こえ"), ("まゐる", "まいる"), ("うつくしうて", "うつくしゅうて"),
            ];
            pairs(rng, b, |k| format!("現代仮名遣いにすると? 「{k}」"), true)
        }
        "taketori" => {
            if rng.chance(0.35) {
                let b: &[(&'static str, &'static [&'static str])] = &[
                    ("『竹取物語』冒頭:順に並べよう", &["今は昔、", "竹取の翁といふものありけり。", "野山にまじりて竹を取りつつ、", "よろづのことに使ひけり。"]),
                    ("冒頭の続き:順に並べよう", &["その竹の中に、", "もと光る竹なむ一筋ありける。", "あやしがりて寄りて見るに、", "筒の中光りたり。"]),
                ];
                orders(rng, b)
            } else {
                let b: &[Q] = &[
                    ("「よろづ」の意味は?", "いろいろ（すべて）", &["一万", "少し", "竹"]),
                    ("「翁」とは?", "おじいさん", &["おばあさん", "若者", "子ども"]),
                    ("古語の「いと」の意味は?", "とても", &["糸", "少し", "いや"]),
                    ("古語の「うつくし」の意味は?", "かわいらしい", &["景色がきれいだ", "大きい", "強い"]),
                    ("古語の「あやし」の意味は?", "不思議だ", &["危ない", "怪しい人", "うれしい"]),
                    ("かぐや姫が最後に帰った所は?", "月の都", &["竜宮城", "海の国", "都の御所"]),
                    ("『竹取物語』は日本最古の何と言われる?", "物語", &["和歌集", "日記", "歴史書"]),
                    ("「なむ〜ける」のように文末が変わる決まりは?", "係り結び", &["枕詞", "掛詞", "倒置"]),
                ];
                bank(rng, b)
            }
        }
        "kanbun" => {
            if rng.chance(0.35) {
                let b: &[(&'static str, &'static [&'static str])] = &[
                    ("「学而時習之」の書き下し:順に並べよう", &["学びて", "時に", "之を", "習ふ"]),
                    ("「不入虎穴、不得虎子」:順に並べよう", &["虎穴に", "入らずんば", "虎子を", "得ず"]),
                ];
                orders(rng, b)
            } else {
                let b: &[Q] = &[
                    ("レ点はどう読む?", "下の一字から上の一字に返る", &["上から順に読む", "二字以上へだてて返る", "読まない"]),
                    ("一・二点はどう読む?", "二字以上へだてて上に返る", &["下の一字から返る", "上から順に読む", "読まない"]),
                    ("「温故知新」の意味は?", "古いことを学んで新しい考えを得る", &["新しいものだけを学ぶ", "古い物を温める", "知らないことは聞く"]),
                    ("「己の欲せざる所は人に施すこと勿かれ」の意味は?", "自分がされたくないことを人にしない", &["人に親切にされたい", "欲しい物は人にあげる", "人に頼らない"]),
                    ("『論語』は誰の言葉を記した?", "孔子", &["孟子", "老子", "李白"]),
                    ("「過ぎたるは猶及ばざるがごとし」の意味は?", "やりすぎは足りないのと同じくらいよくない", &["多いほどよい", "過去は戻らない", "急ぐと失敗する"]),
                ];
                bank(rng, b)
            }
        }
        "letters_jp" => {
            if rng.chance(0.3) {
                let b: &[(&'static str, &'static [&'static str])] = &[("手紙の構成:順に並べよう", &["前文（あいさつ）", "主文（用件）", "末文（結びのあいさつ）", "後付け（日付・署名・宛名）"])];
                orders(rng, b)
            } else {
                let b: &[Q] = &[
                    ("手紙の前文に書くのは?", "時候のあいさつ", &["用件", "日付", "署名"]),
                    ("年賀状で避けたほうがよい言葉は?", "去年", &["昨年", "新春", "謹賀新年"]),
                    ("一月の時候のあいさつは?", "厳寒の候", &["新緑の候", "残暑の候", "秋冷の候"]),
                    ("はがきの宛名で、相手の名前の下につけるのは?", "様", &["殿下", "君", "さん付けなし"]),
                ];
                bank(rng, b)
            }
        }
        _ => japanese("poem", rng),
    }
}

// ------------------------------------------------------------------------------------- English

fn words(n: i64) -> String {
    const ONES: [&str; 20] = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen"];
    const TENS: [&str; 10] = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];
    match n {
        0..=19 => ONES[n as usize].into(),
        100 => "one hundred".into(),
        _ if n % 10 == 0 => TENS[(n / 10) as usize].into(),
        _ => format!("{}-{}", TENS[(n / 10) as usize], ONES[(n % 10) as usize]),
    }
}

/// A sentence to build from its words: (the words in order, a Japanese gloss).
fn build(rng: &mut Rng, b: &[(&'static str, &'static [&'static str])]) -> Task {
    let (gloss, ws) = *rng.pick(b);
    order(rng, &format!("Make the sentence: {gloss}"), ws)
}

fn english(key: &str, rng: &mut Rng) -> Task {
    match key {
        "alphabet" => match rng.below(3) {
            0 => {
                let mut ls: Vec<char> = Vec::new();
                while ls.len() < 5 {
                    let c = (b'a' + rng.below(26) as u8) as char;
                    if !ls.contains(&c) {
                        ls.push(c);
                    }
                }
                ls.sort();
                let s: Vec<String> = ls.iter().map(|c| c.to_string()).collect();
                let r: Vec<&str> = s.iter().map(|x| x.as_str()).collect();
                order(rng, "Put the letters in alphabetical order", &r)
            }
            1 => {
                let i = rng.below(25);
                let c = (b'A' + i as u8) as char;
                let n = ((b'A' + i as u8 + 1) as char).to_string();
                let w: Vec<String> = [i + 2, i.saturating_sub(1), (i + 5) % 26].iter().map(|&k| ((b'A' + (k % 26) as u8) as char).to_string()).collect();
                let wr: Vec<&str> = w.iter().map(|x| x.as_str()).collect();
                choice(rng, format!("Which letter comes after {c}?"), &n, &wr, true)
            }
            _ => {
                let b: &[(&str, &str, &[&str])] = &[("Q", "q", &["p", "g", "b"]), ("B", "b", &["d", "p", "q"]), ("D", "d", &["b", "p", "q"]), ("G", "g", &["q", "j", "p"]), ("N", "n", &["m", "h", "u"]), ("R", "r", &["n", "v", "y"]), ("E", "e", &["a", "c", "o"]), ("H", "h", &["n", "k", "b"])];
                let (c, s, w) = *rng.pick(b);
                choice(rng, format!("The small letter for {c}?"), s, w, true)
            }
        },
        "phonics" => {
            if rng.chance(0.5) {
                let ws = ["cat", "dog", "pen", "sun", "fish", "milk", "bed", "cup", "map", "hat", "red", "six", "box", "bus", "leg", "top"];
                let w = *rng.pick(&ws);
                let vi = w.find(|c: char| "aeiou".contains(c)).unwrap();
                let v = &w[vi..vi + 1];
                let shown = format!("{}_{}", &w[..vi], &w[vi + 1..]);
                let wrong: Vec<&str> = ["a", "e", "i", "o", "u"].into_iter().filter(|x| *x != v).collect();
                choice(rng, format!("Which letter is missing? {shown}"), v, &wrong, true)
            } else {
                let b: &[Q] = &[
                    ("Which word starts with the same sound as 'cat'?", "kite", &["city", "nose", "ship"]),
                    ("Which word starts with the same sound as 'phone'?", "fish", &["pig", "house", "nest"]),
                    ("Which word starts with the same sound as 'ship'?", "shoe", &["sun", "cheese", "zoo"]),
                    ("Which word starts with the same sound as 'cherry'?", "chair", &["kite", "shell", "cake"]),
                    ("Which word starts with the same sound as 'thank'?", "think", &["tank", "sink", "fan"]),
                    ("Which word starts with the same sound as 'bag'?", "bed", &["pen", "dog", "van"]),
                ];
                bank(rng, b)
            }
        }
        "greetings" => {
            let b: &[Q] = &[
                ("How are you?", "I'm fine, thank you.", &["I'm Ken.", "Yes, I am.", "Goodbye."]),
                ("Nice to meet you.", "Nice to meet you, too.", &["You're welcome.", "See you.", "I'm sorry."]),
                ("Thank you.", "You're welcome.", &["Me too.", "Good morning.", "No, I'm not."]),
                ("Goodbye!", "See you tomorrow.", &["Nice to meet you.", "How are you?", "Yes, please."]),
                ("Excuse me. Can you say that again?", "Sure.", &["You're welcome.", "I'm fine.", "Good night."]),
                ("'えんぴつ' in English?", "pencil", &["eraser", "ruler", "notebook"]),
                ("'Stand up, please.' The teacher wants you to...", "stand", &["sit", "open your book", "listen"]),
            ];
            bank(rng, b)
        }
        "be_i_you" | "be_he_she" => {
            if rng.chance(0.3) {
                let b: &[(&'static str, &'static [&'static str])] = &[
                    ("私は学生です。", &["I", "am", "a", "student", "."]),
                    ("あなたは野球ファンです。", &["You", "are", "a", "baseball", "fan", "."]),
                    ("彼女は私の姉です。", &["She", "is", "my", "sister", "."]),
                    ("彼は熊本出身です。", &["He", "is", "from", "Kumamoto", "."]),
                ];
                build(rng, b)
            } else {
                let subs: &[(&str, &str)] = if key == "be_i_you" { &[("I", "am"), ("You", "are"), ("We", "are"), ("I", "am")] } else { &[("He", "is"), ("She", "is"), ("Ken", "is"), ("My sister", "is"), ("They", "are"), ("It", "is")] };
                let (s, v) = *rng.pick(subs);
                let rest = *rng.pick(&["a student.", "from Kazehana.", "twelve years old.", "hungry.", "in the gym.", "very tired today."]);
                let rest = if s == "It" { "a cat." } else { rest };
                choice(rng, format!("{s} ___ {rest}"), v, &["am", "is", "are"], false)
            }
        }
        "be_q" => {
            if rng.chance(0.3) {
                let b: &[(&'static str, &'static [&'static str])] = &[("あなたはテニスの選手ですか。", &["Are", "you", "a", "tennis", "player", "?"]), ("私は眠くありません。", &["I'm", "not", "sleepy", "."])];
                build(rng, b)
            } else {
                let b: &[Q] = &[
                    ("Are you a student? — Yes, I ___.", "am", &["is", "are", "do"]),
                    ("Are you from Tokyo? — No, I'm ___.", "not", &["no", "am", "don't"]),
                    ("___ you hungry?", "Are", &["Is", "Am", "Do"]),
                    ("Is Ken your friend? — Yes, he ___.", "is", &["am", "are", "does"]),
                    ("Are you OK? — ___, I am.", "Yes", &["No", "Not", "Am"]),
                ];
                bank(rng, b)
            }
        }
        "numbers" => {
            if rng.chance(0.6) {
                let (a, b) = (rng.range(1, 50) as i64, rng.range(1, 49) as i64);
                num(format!("{} plus {} is...?", words(a), words(b)), a + b)
            } else {
                let n = *rng.pick(&[13i64, 14, 15, 30, 40, 50, 18, 80]);
                let w = match n {
                    13 => vec!["thirty", "threeteen", "thirdteen"],
                    14 => vec!["forty", "fourty", "fourtteen"],
                    15 => vec!["fifty", "fiveteen", "fifthteen"],
                    30 => vec!["thirteen", "threety", "thirdty"],
                    40 => vec!["fourteen", "fourty", "forteen"],
                    50 => vec!["fifteen", "fivety", "fifthy"],
                    18 => vec!["eighty", "eightteen", "eigthteen"],
                    _ => vec!["eighteen", "eightty", "eigthy"],
                };
                choice(rng, format!("How do you write {n} in English?"), &words(n), &w, true)
            }
        }
        "this_that" => {
            if rng.chance(0.3) {
                let b: &[(&'static str, &'static [&'static str])] = &[("これは私の自転車です。", &["This", "is", "my", "bike", "."]), ("あれは図書館ですか。", &["Is", "that", "a", "library", "?"])];
                build(rng, b)
            } else {
                let b: &[Q] = &[
                    ("(A pen in your hand.) ___ is my pen.", "This", &["That", "These", "He"]),
                    ("(A bird far away.) ___ is a bird.", "That", &["This", "He", "I"]),
                    ("Is this your bag? — Yes, ___ is.", "it", &["this", "that", "he"]),
                    ("Is that a dog? — No, it ___.", "isn't", &["is", "aren't", "don't"]),
                    ("What's this? — ___ a ball.", "It's", &["This", "That", "Is"]),
                ];
                bank(rng, b)
            }
        }
        "verbs" => {
            if rng.chance(0.4) {
                let b: &[(&'static str, &'static [&'static str])] = &[
                    ("私はピアノを弾きます。", &["I", "play", "the", "piano", "."]),
                    ("私たちは学校で昼食を食べます。", &["We", "have", "lunch", "at", "school", "."]),
                    ("私は毎日犬と歩きます。", &["I", "walk", "with", "my", "dog", "every", "day", "."]),
                ];
                build(rng, b)
            } else {
                let b: &[Q] = &[
                    ("I ___ soccer every day.", "play", &["am", "is", "plays"]),
                    ("I ___ music.", "like", &["am", "likes", "is"]),
                    ("We ___ English on Monday.", "study", &["are", "studies", "is"]),
                    ("I ___ my homework after dinner.", "do", &["am", "does", "is"]),
                    ("They ___ to school by bus.", "go", &["are", "goes", "is"]),
                ];
                bank(rng, b)
            }
        }
        "do_q" => {
            if rng.chance(0.3) {
                let b: &[(&'static str, &'static [&'static str])] = &[("あなたはテレビゲームをしますか。", &["Do", "you", "play", "video", "games", "?"]), ("私は納豆が好きではありません。", &["I", "don't", "like", "natto", "."])];
                build(rng, b)
            } else {
                let b: &[Q] = &[
                    ("___ you like dogs?", "Do", &["Are", "Is", "Does"]),
                    ("Do you play tennis? — Yes, I ___.", "do", &["am", "does", "play"]),
                    ("Do you have a pet? — No, I ___.", "don't", &["am not", "not", "isn't"]),
                    ("I ___ like natto.", "don't", &["am not", "not", "isn't"]),
                ];
                bank(rng, b)
            }
        }
        "plurals" => {
            let b: &[(&str, &str, &[&str])] = &[
                ("box", "boxes", &["boxs", "boxies", "box"]), ("cat", "cats", &["cates", "caties", "cat"]), ("city", "cities", &["citys", "cityes", "city"]), ("child", "children", &["childs", "childrens", "childes"]),
                ("dish", "dishes", &["dishs", "dishies", "dish"]), ("watch", "watches", &["watchs", "watchies", "watch"]), ("leaf", "leaves", &["leafs", "leafes", "leavs"]), ("man", "men", &["mans", "mens", "manes"]),
                ("tomato", "tomatoes", &["tomatos", "tomatoies", "tomato"]), ("bus", "buses", &["buss", "busies", "bus"]),
            ];
            let (s, p, w) = *rng.pick(b);
            choice(rng, format!("two ___ ({s})"), p, w, true)
        }
        "what_q" => {
            if rng.chance(0.3) {
                let b: &[(&'static str, &'static [&'static str])] = &[("あなたは朝食に何を食べますか。", &["What", "do", "you", "eat", "for", "breakfast", "?"])];
                build(rng, b)
            } else {
                let b: &[Q] = &[
                    ("___ do you have in your bag? — I have a book.", "What", &["Who", "Where", "When"]),
                    ("What sport do you like? — I ___ basketball.", "like", &["am", "is", "likes"]),
                    ("What do you do on Sundays? — I ___ my grandmother.", "visit", &["am", "visits", "is"]),
                    ("What ___ do you like? — I like blue.", "color", &["sport", "food", "time"]),
                ];
                bank(rng, b)
            }
        }
        "calendar_en" => {
            if rng.chance(0.5) {
                let days = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
                let i = rng.below(7);
                let wr: Vec<&str> = days.iter().copied().filter(|d| *d != days[(i + 1) % 7]).collect();
                choice(rng, format!("The day after {} is ___.", days[i]), days[(i + 1) % 7], &wr, true)
            } else {
                let b: &[Q] = &[
                    ("The first month of the year is ___.", "January", &["June", "July", "March"]),
                    ("Cherry blossoms come in ___.", "spring", &["autumn", "winter", "summer"]),
                    ("The month after July is ___.", "August", &["April", "October", "June"]),
                    ("Spell the 4th month:", "April", &["Aprill", "Apiril", "Abril"]),
                    ("Children's Day is in ___.", "May", &["March", "June", "October"]),
                ];
                bank(rng, b)
            }
        }
        "can" | "can_q" => {
            if rng.chance(0.3) {
                let b: &[(&'static str, &'static [&'static str])] = &[("私はカレーを作れます。", &["I", "can", "cook", "curry", "."]), ("あなたはギターを弾けますか。", &["Can", "you", "play", "the", "guitar", "?"])];
                build(rng, b)
            } else {
                let b: &[Q] = if key == "can" {
                    &[("I ___ swim.", "can", &["cans", "am", "do"]), ("She can ___ the piano.", "play", &["plays", "playing", "to play"]), ("I can't ___ well.", "sing", &["sings", "singing", "sang"]), ("My cat ___ jump high.", "can", &["cans", "is", "does"])]
                } else {
                    &[("___ you ski? — Yes, I can.", "Can", &["Do", "Are", "Is"]), ("Can he dance? — No, he ___.", "can't", &["isn't", "don't", "doesn't"]), ("___ I use your eraser? — Sure.", "Can", &["Do", "Am", "Does"]), ("Can you help me? — ___.", "Sure.", &["Yes, I do.", "No, I'm not.", "I'm fine."])]
                };
                bank(rng, b)
            }
        }
        "imperatives" => {
            if rng.chance(0.3) {
                let b: &[(&'static str, &'static [&'static str])] = &[("ドアを閉めてください。", &["Please", "close", "the", "door", "."]), ("廊下を走ってはいけません。", &["Don't", "run", "in", "the", "hallway", "."])];
                build(rng, b)
            } else {
                let b: &[Q] = &[("___ quiet in the library.", "Be", &["Is", "Are", "Do"]), ("___ run in the hallway.", "Don't", &["Not", "No", "Isn't"]), ("Please ___ the window.", "open", &["opens", "opening", "to open"]), ("___ eat lunch together.", "Let's", &["Lets", "Let", "Do"])];
                bank(rng, b)
            }
        }
        "third" => {
            let subs = ["He", "She", "Ken", "My mother", "Aya", "Our teacher"];
            let vs: &[(&str, &str, &str, &str)] = &[
                ("play", "plays", "playes", "soccer after school"), ("like", "likes", "likies", "cats"), ("study", "studies", "studys", "English at home"), ("go", "goes", "gos", "to the library"),
                ("watch", "watches", "watchs", "TV after dinner"), ("have", "has", "haves", "a big dog"), ("wash", "washes", "washs", "the dishes"), ("teach", "teaches", "teachs", "music"),
            ];
            let s = *rng.pick(&subs);
            let (base, third, wrong, obj) = *rng.pick(vs);
            choice(rng, format!("{s} ___ {obj}."), third, &[base, wrong, "is"], false)
        }
        "does_q" => {
            if rng.chance(0.3) {
                let b: &[(&'static str, &'static [&'static str])] = &[("彼は歩いて学校へ行きますか。", &["Does", "he", "walk", "to", "school", "?"])];
                build(rng, b)
            } else {
                let b: &[Q] = &[("___ your sister play tennis?", "Does", &["Do", "Is", "Are"]), ("Does Ken like cats? — No, he ___.", "doesn't", &["don't", "isn't", "not"]), ("My father ___ drink coffee.", "doesn't", &["don't", "isn't", "not"]), ("Does she ___ in Kyoto?", "live", &["lives", "living", "lived"])];
                bank(rng, b)
            }
        }
        "pronouns" => {
            let b: &[Q] = &[
                ("This is Yumi. ___ is my friend.", "She", &["Her", "Hers", "He"]), ("I like Ken. I play soccer with ___.", "him", &["he", "his", "he's"]), ("That's ___ bag. (Aya's)", "her", &["she", "hers", "herself"]),
                ("Is this pencil yours? — Yes, it's ___.", "mine", &["my", "me", "I"]), ("We love ___ school.", "our", &["us", "we", "ours"]), ("Look at the dogs. ___ are cute.", "They", &["Them", "Their", "It"]),
            ];
            bank(rng, b)
        }
        "wh" => {
            let b: &[Q] = &[
                ("___ is that man? — He's our new teacher.", "Who", &["Whose", "Where", "When"]), ("___ bag is this? — It's mine.", "Whose", &["Who", "Which", "What"]), ("___ do you like, cats or dogs?", "Which", &["Who", "Whose", "Where"]),
                ("___ do you live? — In Kazehana.", "Where", &["When", "How", "Who"]), ("___ is your birthday? — It's May 5.", "When", &["Where", "What", "Who"]), ("___ do you come to school? — By bike.", "How", &["What", "When", "Who"]),
                ("___ many books do you have?", "How", &["What", "Which", "Who"]),
            ];
            bank(rng, b)
        }
        "time_en" => {
            let h = rng.range(1, 12) as i64;
            let m = *rng.pick(&[0i64, 15, 30, 45]);
            let say = |h: i64, m: i64| if m == 0 { format!("It's {} o'clock.", words(h)) } else { format!("It's {} {}.", words(h), words(m)) };
            let r = say(h, m);
            let w = [say(h % 12 + 1, m), say(h, (m + 15) % 60), say((h + 10) % 12 + 1, (m + 30) % 60)];
            let wr: Vec<&str> = w.iter().map(|s| s.as_str()).collect();
            choice(rng, format!("What time is it? ({h}:{m:02})"), &r, &wr, false)
        }
        "progressive" | "ing_spell" => {
            let vs: &[(&str, &str, &str, &str)] = &[
                ("read", "reading", "readding", "reads"), ("swim", "swimming", "swiming", "swims"), ("run", "running", "runing", "runs"), ("make", "making", "makeing", "makes"),
                ("write", "writing", "writting", "writes"), ("study", "studying", "studing", "studies"), ("sit", "sitting", "siting", "sits"), ("dance", "dancing", "danceing", "dances"),
                ("cook", "cooking", "cookking", "cooks"), ("lie", "lying", "lieing", "lies"),
            ];
            let (b, ing, wrong, s) = *rng.pick(vs);
            if key == "ing_spell" {
                choice(rng, format!("The -ing form of '{b}'?"), ing, &[wrong, s, &format!("{b}ing")], true)
            } else {
                let sub = *rng.pick(&[("I", "am"), ("She", "is"), ("They", "are"), ("My brother", "is"), ("We", "are")]);
                if rng.chance(0.5) {
                    choice(rng, format!("{} {} ___ now. ({b})", sub.0, sub.1), ing, &[b, s, wrong], false)
                } else {
                    choice(rng, format!("{} ___ {ing} now.", sub.0), sub.1, &["am", "is", "are", "do"], false)
                }
            }
        }
        "progressive_q" => {
            if rng.chance(0.3) {
                let b: &[(&'static str, &'static [&'static str])] = &[("あなたは今何をしていますか。", &["What", "are", "you", "doing", "now", "?"])];
                build(rng, b)
            } else {
                let b: &[Q] = &[("What are you doing? — I ___ watching TV.", "am", &["is", "are", "do"]), ("___ she cooking now? — Yes, she is.", "Is", &["Does", "Are", "Do"]), ("Are they running? — No, they ___.", "aren't", &["don't", "isn't", "not"])];
                bank(rng, b)
            }
        }
        "want_to" => {
            if rng.chance(0.3) {
                let b: &[(&'static str, &'static [&'static str])] = &[("私はオーストラリアに行きたいです。", &["I", "want", "to", "go", "to", "Australia", "."])];
                build(rng, b)
            } else {
                let b: &[Q] = &[("I want ___ a doctor.", "to be", &["be", "being", "to"]), ("I like ___ comics.", "reading", &["read", "to reading", "reads"]), ("She wants ___ Kyoto.", "to visit", &["visit", "visiting", "visits"])];
                bank(rng, b)
            }
        }
        "letter_en" => {
            if rng.chance(0.5) {
                let b: &[(&'static str, &'static [&'static str])] = &[("Put the letter in order", &["Dear Emma,", "How are you?", "I'm a junior high student in Japan.", "I like baseball and comics.", "Please write back.", "Your friend,"])];
                orders(rng, b)
            } else {
                let b: &[Q] = &[("How do you start a letter?", "Dear Sam,", &["Bye Sam,", "Sam is", "To Sam is"]), ("How do you end a letter to a friend?", "Your friend,", &["Dear,", "Hello,", "Excuse me,"])];
                bank(rng, b)
            }
        }
        "season_en" => {
            let b: &[Q] = &[("Merry ___!", "Christmas", &["New Year", "Birthday", "Halloween"]), ("Happy New ___!", "Year", &["Christmas", "Day", "Month"]), ("The last month of the year is ___.", "December", &["November", "January", "October"]), ("We eat toshikoshi soba on New Year's ___.", "Eve", &["Day", "Night", "Morning"])];
            bank(rng, b)
        }
        "past_reg" | "ed_spell" => {
            let vs: &[(&str, &str, &str)] = &[
                ("play", "played", "plaied"), ("study", "studied", "studyed"), ("stop", "stopped", "stoped"), ("like", "liked", "likeed"), ("visit", "visited", "visitted"),
                ("watch", "watched", "watcht"), ("clean", "cleaned", "cleanned"), ("try", "tried", "tryed"), ("live", "lived", "liveed"), ("enjoy", "enjoyed", "enjoied"), ("plan", "planned", "planed"), ("carry", "carried", "carryed"),
            ];
            let (b, ed, wrong) = *rng.pick(vs);
            if key == "ed_spell" {
                choice(rng, format!("The past form of '{b}'?"), ed, &[wrong, b, &format!("{b}ing")], true)
            } else {
                let when = *rng.pick(&["yesterday", "last night", "last Sunday", "two days ago"]);
                choice(rng, format!("I ___ {when}. ({b})"), ed, &[b, wrong, &format!("{b}s")], false)
            }
        }
        "past_irr" => {
            let vs: &[(&str, &str)] = &[("go", "went"), ("eat", "ate"), ("see", "saw"), ("have", "had"), ("make", "made"), ("buy", "bought"), ("come", "came"), ("get", "got"), ("take", "took"), ("write", "wrote"), ("swim", "swam"), ("run", "ran"), ("sit", "sat"), ("give", "gave"), ("know", "knew")];
            let (b, p) = *rng.pick(vs);
            let other = rng.pick(vs).1;
            choice(rng, format!("The past form of '{b}'?"), p, &[&format!("{b}ed"), b, other], true)
        }
        "past_q" => {
            if rng.chance(0.3) {
                let b: &[(&'static str, &'static [&'static str])] = &[("楽しかったですか。", &["Did", "you", "have", "a", "good", "time", "?"]), ("私は昨日テレビを見ませんでした。", &["I", "didn't", "watch", "TV", "yesterday", "."])];
                build(rng, b)
            } else {
                let b: &[Q] = &[("___ you go to the festival? — Yes, I did.", "Did", &["Do", "Were", "Are"]), ("Did she eat breakfast? — No, she ___.", "didn't", &["doesn't", "wasn't", "don't"]), ("I didn't ___ TV yesterday.", "watch", &["watched", "watching", "watches"]), ("Did you ___ your homework?", "do", &["did", "done", "does"])];
                bank(rng, b)
            }
        }
        "past_be" => {
            let b: &[Q] = &[("I ___ tired yesterday.", "was", &["were", "am", "did"]), ("We ___ in the gym.", "were", &["was", "are", "did"]), ("___ he at home last night? — Yes, he was.", "Was", &["Were", "Did", "Is"]), ("The test ___ easy.", "was", &["were", "is", "did"]), ("You ___ late this morning.", "were", &["was", "did", "be"])];
            bank(rng, b)
        }
        "past_prog" => {
            let b: &[Q] = &[("I ___ reading when you called.", "was", &["were", "am", "did"]), ("They were ___ baseball. (play)", "playing", &["play", "played", "plays"]), ("What ___ you doing at eight?", "were", &["was", "did", "are"]), ("She was ___ a letter. (write)", "writing", &["writting", "wrote", "write"])];
            bank(rng, b)
        }
        "diary_en" => {
            let b: &[Q] = &[
                ("'Saturday, May 9. I went to the river with my brother. We caught three fish. It was sunny.' Who did the writer go with?", "the writer's brother", &["a friend", "the father", "nobody"]),
                ("'Saturday, May 9. I went to the river with my brother. We caught three fish. It was sunny.' How was the weather?", "sunny", &["rainy", "cloudy", "snowy"]),
                ("'Sunday, June 7. I made a cake for my mother. She was very happy.' What did the writer make?", "a cake", &["lunch", "a card", "a cake and tea"]),
                ("'Sunday, June 7. I made a cake for my mother. She was very happy.' How did the mother feel?", "happy", &["sad", "angry", "tired"]),
            ];
            if rng.chance(0.2) {
                num("'We caught three fish.' How many fish?", 3)
            } else {
                bank(rng, b)
            }
        }
        "directions" => {
            if rng.chance(0.3) {
                let b: &[(&'static str, &'static [&'static str])] = &[("銀行の角を左に曲がってください。", &["Turn", "left", "at", "the", "bank", "."])];
                build(rng, b)
            } else {
                let b: &[Q] = &[("(Walk along this street.) 'Go ___.'", "straight", &["right", "back", "over"]), ("'Turn ___ at the second corner.' (you turn ←)", "left", &["right", "straight", "back"]), ("'It's ___ your right.'", "on", &["at", "in", "by"]), ("'How can I get to the station?'", "Go straight and turn right.", &["It's sunny.", "I'm a student.", "Yes, it is."])];
                bank(rng, b)
            }
        }
        "write_year" => {
            let b: &[(&'static str, &'static [&'static str])] = &[
                ("私は体育祭を楽しみました。", &["I", "enjoyed", "the", "sports", "festival", "."]),
                ("一番の思い出は合唱コンクールでした。", &["My", "best", "memory", "was", "the", "chorus", "contest", "."]),
                ("私は英語を一生懸命勉強しました。", &["I", "studied", "English", "very", "hard", "."]),
            ];
            build(rng, b)
        }
        "vocab_all" => {
            let b: &[(&str, &str)] = &[("えんぴつ", "pencil"), ("図書館", "library"), ("天気", "weather"), ("夏", "summer"), ("朝ご飯", "breakfast"), ("友達", "friend"), ("学校", "school"), ("水曜日", "Wednesday"), ("駅", "station"), ("走る", "run"), ("台所", "kitchen"), ("冬", "winter")];
            pairs(rng, b, |k| format!("'{k}' in English?"), true)
        }
        _ => english("alphabet", rng),
    }
}

// ------------------------------------------------------------------------------------- science

fn science(key: &str, rng: &mut Rng) -> Task {
    match key {
        "observe" => bank(rng, &[
            ("Sketching a living thing for science, you...", "draw what you see with thin, clear lines", &["shade it so it looks nice", "draw its shadow too", "colour everything in"]),
            ("Dandelions on a trampled path grow...", "flat, leaves spread on the ground", &["very tall", "in water", "only indoors"]),
            ("Pill bugs (dango-mushi) live...", "under stones, in damp dark places", &["in dry sand", "high in trees", "in ponds"]),
            ("Plants in the shade of the building tend to have...", "bigger, thinner leaves", &["no leaves", "red flowers", "thick roots above ground"]),
        ]),
        "microscope" => {
            if rng.chance(0.5) {
                let b: &[(&'static str, &'static [&'static str])] = &[("Using a microscope: in order", &["Fit the lowest-power objective lens", "Look in and adjust the light (mirror)", "Put the slide on the stage", "Watching from the side, bring the lens close to the slide", "Looking in, move the lens away to focus"])];
                orders(rng, b)
            } else if rng.chance(0.5) {
                let (e, o) = (*rng.pick(&[10i64, 15]), *rng.pick(&[4i64, 10, 40]));
                num_u(format!("Eyepiece {e}×, objective {o}×: magnification?"), e * o, "×")
            } else {
                bank(rng, &[("Using a loupe, hold it...", "close to your eye", &["far from your eye", "on the object", "at arm's length"]), ("Never look at ___ through a loupe", "the sun", &["a leaf", "your hand", "a stone"])])
            }
        }
        "flower_parts" => {
            if rng.chance(0.4) {
                let b: &[(&'static str, &'static [&'static str])] = &[("The parts of a flower, from the outside in", &["sepals", "petals", "stamens", "pistil"])];
                orders(rng, b)
            } else {
                bank(rng, &[("Pollen is made in the...", "anther (of the stamen)", &["stigma", "ovary", "sepal"]), ("The ovary grows into the...", "fruit", &["seed", "leaf", "root"]), ("The ovules become...", "seeds", &["fruit", "petals", "pollen"]), ("Pollen landing on the stigma is called...", "pollination", &["fertiliser", "germination", "photosynthesis"])])
            }
        }
        "seed_plants" => sort(rng, "Where does it go", ["angiosperm (ovules in an ovary)", "gymnosperm (bare ovules)"], &[("cherry", false), ("pine", true), ("tulip", false), ("ginkgo", true), ("rice", false), ("cedar", true), ("dandelion", false), ("cycad", true), ("apple", false), ("cypress", true)]),
        "monocot_dicot" => {
            if rng.chance(0.6) {
                sort(rng, "Monocot or dicot", ["monocot", "dicot"], &[("rice", false), ("corn", false), ("lily", false), ("wheat", false), ("bamboo", false), ("sunflower", true), ("morning glory", true), ("dandelion", true), ("bean", true), ("rose", true)])
            } else {
                bank(rng, &[("Monocot leaves have veins that are...", "parallel", &["net-like", "branching", "missing"]), ("A dicot's roots are...", "a main root with side roots", &["many fibrous roots", "no roots", "air roots"]), ("Seed leaves (cotyledons) of a monocot:", "one", &["two", "none", "three"])])
            }
        }
        "spore_plants" => sort(rng, "Seeds or spores", ["makes seeds", "makes spores"], &[("pine", false), ("fern", true), ("moss", true), ("horsetail", true), ("tulip", false), ("bracken", true), ("ginkgo", false), ("liverwort", true)]),
        "plant_classify" => {
            let k = *rng.pick(&["seed_plants", "monocot_dicot", "spore_plants"]);
            science(k, rng)
        }
        "vertebrates" => {
            let b: &[(&str, &str)] = &[("frog", "amphibian"), ("newt", "amphibian"), ("shark", "fish"), ("salmon", "fish"), ("turtle", "reptile"), ("snake", "reptile"), ("penguin", "bird"), ("bat", "mammal"), ("whale", "mammal"), ("crocodile", "reptile"), ("ostrich", "bird"), ("dolphin", "mammal"), ("salamander", "amphibian"), ("eel", "fish")];
            pairs(rng, b, |k| format!("Which group is the {k} in?"), false)
        }
        "invertebrates" => {
            if rng.chance(0.6) {
                sort(rng, "Which group", ["arthropod (insects, crustaceans...)", "mollusc"], &[("grasshopper", false), ("crab", false), ("butterfly", false), ("shrimp", false), ("spider", false), ("octopus", true), ("squid", true), ("snail", true), ("clam", true), ("slug", true)])
            } else if rng.chance(0.5) {
                num("How many legs does an insect have?", 6)
            } else {
                bank(rng, &[("An insect's body has three parts: head, thorax and...", "abdomen", &["tail", "shell", "mantle"]), ("A mollusc's internal organs are covered by its...", "mantle", &["exoskeleton", "feathers", "scales"])])
            }
        }
        "animal_classify" => {
            if rng.chance(0.3) {
                bank(rng, &[("Animals with a backbone are called...", "vertebrates", &["invertebrates", "arthropods", "molluscs"]), ("Breathe with gills when young, lungs as adults:", "amphibians", &["reptiles", "fish", "birds"]), ("Give birth to live young (viviparous):", "mammals", &["birds", "reptiles", "fish"])])
            } else {
                science(*rng.pick(&["vertebrates", "invertebrates"]), rng)
            }
        }
        "burner" => {
            if rng.chance(0.5) {
                let b: &[(&'static str, &'static [&'static str])] = &[("Lighting a gas burner: in order", &["Check both valves are closed", "Open the main gas tap", "Bring a lit match to the top", "Open the gas valve to light it", "Open the air valve till the flame is blue"])];
                orders(rng, b)
            } else {
                bank(rng, &[("A good burner flame is...", "pale blue", &["orange", "yellow and smoky", "red"]), ("To turn the burner off, close the air valve, then the gas valve, then...", "the main tap", &["the window", "nothing", "the lid"])])
            }
        }
        "metals" => {
            if rng.chance(0.7) {
                sort(rng, "Metal or not", ["metal", "non-metal"], &[("iron", false), ("copper", false), ("aluminium", false), ("gold", false), ("silver", false), ("zinc", false), ("glass", true), ("wood", true), ("plastic", true), ("rubber", true), ("paper", true), ("ceramic", true)])
            } else {
                bank(rng, &[("Something every metal does:", "conduct electricity", &["stick to magnets", "float on water", "burn easily"]), ("Polishing a metal makes it...", "shine (metallic lustre)", &["melt", "turn into gas", "dissolve"])])
            }
        }
        "organic" => sort(rng, "Organic or inorganic", ["organic (has carbon; burns to CO₂)", "inorganic"], &[("sugar", false), ("starch", false), ("wood", false), ("paper", false), ("plastic", false), ("ethanol", false), ("salt", true), ("iron", true), ("glass", true), ("water", true), ("sand", true)]),
        "density" => {
            if rng.chance(0.7) {
                let ds = [(27i64, "aluminium"), (79, "iron"), (89, "copper"), (9, "ice"), (10, "water")];
                let (d, _) = *rng.pick(&ds);
                let v = *rng.pick(&[10i64, 20, 50]);
                num_d(format!("A block's mass is {} g and its volume {v} cm³. Its density (1 decimal)?", show_tenths(d * v)), d, "g/cm³")
            } else {
                bank(rng, &[("Something sinks in water if its density is...", "more than 1 g/cm³", &["less than 1 g/cm³", "exactly 0", "anything at all"]), ("Ice floats on water because it is...", "less dense", &["heavier", "colder", "solid"])])
            }
        }
        "gases" => {
            if rng.chance(0.8) {
                let b: &[(&str, &str)] = &[
                    ("Makes a glowing splint burst into flame", "oxygen"), ("Turns limewater cloudy", "carbon dioxide"), ("Burns with a 'pop'", "hydrogen"), ("Dissolves very well, sharp smell, alkaline", "ammonia"),
                    ("About 78% of air", "nitrogen"), ("Made from hydrogen peroxide and manganese dioxide", "oxygen"), ("Made from limestone and dilute hydrochloric acid", "carbon dioxide"), ("Made from zinc and dilute hydrochloric acid", "hydrogen"), ("The lightest gas", "hydrogen"),
                ];
                pairs(rng, b, |k| format!("Which gas? {k}"), false)
            } else {
                bank(rng, &[("A gas that hardly dissolves is collected...", "over water", &["by upward air displacement", "by downward air displacement", "in a beaker"]), ("Ammonia is collected by...", "upward air displacement", &["over water", "downward air displacement", "a filter"])])
            }
        }
        "solutions" => {
            if rng.chance(0.6) {
                let (s, w) = *rng.pick(&[(20i64, 80i64), (10, 90), (25, 75), (15, 85), (30, 120), (5, 45), (40, 160), (12, 48)]);
                num_u(format!("{s} g of salt dissolved in {w} g of water. Concentration?"), s * 100 / (s + w), "%")
            } else {
                bank(rng, &[("In salt water, the salt is the...", "solute", &["solvent", "solution", "crystal"]), ("In salt water, the water is the...", "solvent", &["solute", "precipitate", "crystal"]), ("A dissolved substance spreads through the water...", "evenly", &["only at the top", "only at the bottom", "not at all"])])
            }
        }
        "solubility" => {
            if rng.chance(0.4) {
                let (hot, ht, cold, ct) = *rng.pick(&[(109i64, 60, 32i64, 20), (64, 40, 32, 20), (169, 80, 64, 40)]);
                num_u(format!("100 g of water dissolves {hot} g of potassium nitrate at {ht}°C and {cold} g at {ct}°C. Cool a saturated solution from {ht}°C to {ct}°C: crystals?"), hot - cold, "g")
            } else {
                bank(rng, &[("Crystals form when a hot saturated solution...", "cools", &["is stirred", "is heated more", "gets more water"]), ("Getting crystals by cooling works well for...", "potassium nitrate", &["table salt", "sugar water", "air"]), ("Table salt's solubility changes with temperature...", "only a little", &["a lot", "not at all", "goes up and down"]), ("What's left on the filter paper after filtering:", "the solid that didn't dissolve", &["the solution", "the solute", "nothing"])])
            }
        }
        "states" => {
            if rng.chance(0.6) {
                sort(rng, "At room temperature", ["solid", "liquid"], &[("iron", false), ("salt", false), ("sugar", false), ("wax", false), ("copper", false), ("ethanol", true), ("water", true), ("mercury", true), ("olive oil", true), ("vinegar", true)])
            } else {
                bank(rng, &[("When a substance melts, its mass...", "stays the same", &["goes up", "goes down", "doubles"]), ("When a liquid boils into a gas, its volume...", "gets much bigger", &["stays the same", "gets smaller", "disappears"]), ("When water freezes, its volume...", "gets bigger", &["gets smaller", "stays the same", "disappears"])])
            }
        }
        "boiling" => {
            if rng.chance(0.5) {
                let (q, a) = *rng.pick(&[("Water boils at ___ °C", 100i64), ("Water freezes at ___ °C", 0), ("Ethanol boils at about ___ °C", 78)]);
                num_u(q, a, "°C")
            } else {
                bank(rng, &[("Separating liquids by their boiling points is called...", "distillation", &["filtration", "evaporation", "dissolving"]), ("Heating a water-ethanol mix, the first liquid out is rich in...", "ethanol", &["water", "sugar", "salt"]), ("While a pure substance boils, its temperature...", "stays the same", &["keeps rising", "falls", "jumps"])])
            }
        }
        "reflection" => {
            if rng.chance(0.7) {
                let a = rng.range(15, 75) as i64;
                if rng.chance(0.5) {
                    num_u(format!("A ray hits a mirror with an angle of incidence of {a}°. The angle of reflection?"), a, "°")
                } else {
                    num_u(format!("A ray meets the mirror's surface at {a}°. The angle of reflection?"), 90 - a, "°")
                }
            } else {
                bank(rng, &[("Your image in a mirror looks...", "as far behind the mirror as you are in front", &["on the mirror's surface", "twice as far", "in front of the mirror"]), ("Light bouncing off a rough surface in all directions:", "diffuse reflection", &["refraction", "total reflection", "absorption"])])
            }
        }
        "refraction" => bank(rng, &[
            ("Light going from water into air bends...", "away from the normal", &["towards the normal", "not at all", "back into the water"]),
            ("A coin in a cup of water looks...", "higher than it really is", &["lower than it is", "bigger and deeper", "invisible"]),
            ("Light in water hitting the surface at a shallow angle all reflects back:", "total internal reflection", &["refraction", "diffusion", "absorption"]),
            ("Light bends at a boundary because its ___ changes", "speed", &["colour", "mass", "temperature"]),
        ]),
        "lenses" => {
            if rng.chance(0.5) {
                let f = *rng.pick(&[5i64, 8, 10, 12, 15]);
                num_u(format!("A convex lens, focal length {f} cm. An object {} cm away: the image forms how far away?", 2 * f), 2 * f, "cm")
            } else {
                bank(rng, &[("An object inside the focal point: you see...", "an upright, magnified virtual image", &["an upside-down real image", "no image", "a smaller real image"]), ("A real image through a convex lens is...", "upside down", &["upright", "always bigger", "made by a mirror"]), ("An object very far away: its real image forms near...", "the focal point", &["the lens", "twice the focal length", "nowhere"])])
            }
        }
        "sound" => {
            if rng.chance(0.6) {
                let t = rng.range(2, 9) as i64;
                num_u(format!("Thunder is heard {t} s after the flash. Sound travels 340 m/s. How far away?"), 340 * t, "m")
            } else {
                bank(rng, &[("Sound can't travel through...", "a vacuum", &["water", "iron", "air"]), ("Sound travels fastest through...", "iron", &["air", "water", "a vacuum"]), ("Sound is made by things that...", "vibrate", &["glow", "melt", "dissolve"])])
            }
        }
        "pitch" => {
            if rng.chance(0.4) {
                let (n, t) = *rng.pick(&[(200i64, 5i64), (100, 5), (440, 10), (300, 5)]);
                num_u(format!("A string vibrates {n} times in 0.{t} s. Its frequency?"), n * 10 / t, "Hz")
            } else {
                bank(rng, &[("A higher sound has a larger...", "frequency", &["amplitude", "wavelength", "speed"]), ("A louder sound has a larger...", "amplitude", &["frequency", "pitch", "speed"]), ("To make a string's note higher, make it...", "shorter or tighter", &["longer", "looser", "thicker"]), ("Frequency is measured in...", "hertz (Hz)", &["newtons", "decibels per metre", "metres"])])
            }
        }
        "forces" => bank(rng, &[
            ("The force pulling things towards the earth", "gravity", &["friction", "magnetism", "elastic force"]),
            ("The force that resists sliding", "friction", &["gravity", "magnetism", "buoyancy"]),
            ("A force arrow's length shows...", "the size of the force", &["the time", "the mass", "the speed"]),
            ("1 N is about the weight of...", "a 100 g object", &["a 1 kg object", "a 10 g object", "a 1 g object"]),
            ("A force can change an object's shape, hold it up, or change its...", "motion", &["colour", "mass", "smell"]),
        ]),
        "hooke" => {
            if rng.chance(0.5) {
                let (k, n) = (rng.range(1, 4) as i64, rng.range(2, 7) as i64);
                num_u(format!("A spring stretches {k} cm for every 1 N. Hang {n} N on it: the stretch?"), k * n, "cm")
            } else {
                let (s, g) = (*rng.pick(&[2i64, 3, 4]), *rng.pick(&[20i64, 40]));
                let g2 = g * rng.range(2, 5) as i64;
                num_u(format!("A spring stretches {s} cm with {g} g. With {g2} g?"), s * g2 / g, "cm")
            }
        }
        "weight" => {
            if rng.chance(0.4) {
                let g = 100 * rng.range(2, 15) as i64;
                num_u(format!("The weight of {g} g (1 N per 100 g)?"), g / 100, "N")
            } else {
                bank(rng, &[("On the moon, your mass...", "stays the same", &["becomes 1/6", "becomes 0", "doubles"]), ("On the moon, your weight is about...", "1/6 of Earth's", &["the same", "6 times", "zero"]), ("Two forces balance if they're equal, opposite, and...", "on the same straight line", &["at right angles", "the same way", "on different things"])])
            }
        }
        "volcanoes" => {
            if rng.chance(0.4) {
                sort(rng, "How it cooled", ["volcanic rock (cooled fast)", "plutonic rock (cooled slowly)"], &[("basalt", false), ("andesite", false), ("rhyolite", false), ("granite", true), ("gabbro", true), ("diorite", true)])
            } else {
                bank(rng, &[("Runny magma makes a volcano that is...", "low and wide (a shield volcano)", &["a steep dome", "a cone of ash only", "underground"]), ("Sticky magma erupts...", "explosively, with pale lava", &["quietly, with black lava", "not at all", "only under the sea"]), ("Mount Fuji is a...", "stratovolcano (cone)", &["shield volcano", "lava dome", "caldera lake"])])
            }
        }
        "igneous" => {
            if rng.chance(0.5) {
                sort(rng, "Which kind of mineral", ["dark (coloured) mineral", "pale (colourless/white) mineral"], &[("olivine", false), ("pyroxene", false), ("amphibole", false), ("biotite", false), ("quartz", true), ("feldspar", true)])
            } else {
                bank(rng, &[("Big crystals all the same size (equigranular) mean the magma cooled...", "slowly, deep underground", &["fast, at the surface", "in water", "in the air"]), ("Big crystals in a fine groundmass (porphyritic) mean it cooled...", "fast, near the surface", &["slowly, deep down", "never", "in a lab"])])
            }
        }
        "quakes" => {
            if rng.chance(0.6) {
                let t = rng.range(3, 20) as i64;
                num_u(format!("P-waves travel 6 km/s, S-waves 3 km/s. A place {} km from the focus: how long is the initial tremor?", 6 * t), t, "s")
            } else {
                bank(rng, &[("The first small shaking is caused by...", "P-waves", &["S-waves", "surface waves", "a tsunami"]), ("The point underground where a quake starts", "the focus (hypocentre)", &["the epicentre", "the fault line", "the plate"]), ("The farther from the focus, the initial tremor is...", "longer", &["shorter", "the same", "gone"])])
            }
        }
        "plates" => {
            if rng.chance(0.3) {
                num("How many levels does Japan's seismic intensity (shindo) scale have?", 10)
            } else {
                bank(rng, &[("Magnitude measures...", "the energy the quake released", &["the shaking at one place", "the depth", "how long it lasted"]), ("Japan lies where ___ plates meet", "four", &["two", "one", "six"]), ("The Pacific plate sinks beneath...", "the North American plate", &["the Pacific plate", "Africa", "the Indian plate"])])
            }
        }
        "strata" => {
            if rng.chance(0.3) {
                let b: &[(&'static str, &'static [&'static str])] = &[("Sedimentary rocks by grain size, biggest first", &["conglomerate", "sandstone", "mudstone"])];
                orders(rng, b)
            } else {
                bank(rng, &[("In layers that haven't been folded, the oldest is...", "at the bottom", &["at the top", "in the middle", "anywhere"]), ("Rounded pebbles show the material was carried by...", "water", &["wind alone", "volcanoes", "animals"]), ("Fizzes with dilute hydrochloric acid:", "limestone", &["chert", "sandstone", "granite"]), ("Very hard, made of tiny shells, doesn't fizz with acid:", "chert", &["limestone", "mudstone", "tuff"]), ("A layer of volcanic ash becomes...", "tuff", &["mudstone", "limestone", "granite"])])
            }
        }
        "fossils" => sort(rng, "What it tells you", ["index fossil (the age)", "facies fossil (the environment)"], &[("trilobite", false), ("ammonite", false), ("fusulinid", false), ("Naumann's elephant", false), ("coral", true), ("shijimi clam", true), ("beech leaves", true), ("oyster", true)]),
        "land" => bank(rng, &[
            ("Volcanoes give Japan...", "hot springs and geothermal power", &["only earthquakes", "no benefits", "deserts"]),
            ("A sudden rise of the sea floor in a quake can send...", "a tsunami", &["a typhoon", "a drought", "a blizzard"]),
            ("Liquefaction happens most in...", "reclaimed land and sandy ground", &["solid granite", "mountain tops", "forests"]),
            ("Before a disaster, check your area's...", "hazard map", &["tourist map", "bus timetable", "weather chart"]),
        ]),
        "pressure" => {
            if rng.chance(0.5) {
                let (f, a) = *rng.pick(&[(600i64, 2i64), (300, 3), (1000, 4), (60, 3), (900, 3)]);
                num_u(format!("A {f} N box rests on {a} m². The pressure?"), f / a, "Pa")
            } else {
                bank(rng, &[("Snowshoes stop you sinking because they...", "spread your weight over more area", &["make you lighter", "grip the snow", "melt the snow"]), ("Water pressure gets ___ the deeper you go", "bigger", &["smaller", "the same", "zero"]), ("Air pressure at sea level is about...", "1013 hPa", &["100 Pa", "10 hPa", "1 Pa"]), ("Up a mountain, a sealed snack bag...", "puffs up", &["shrinks", "stays the same", "freezes"])])
            }
        }
        _ => science("observe", rng),
    }
}

// ------------------------------------------------------------------------------------- social studies

/// (year, event) for the timeline
const EVENTS: &[(i64, &str)] = &[
    (239, "Himiko sends envoys to Wei"), (604, "The Seventeen-Article Constitution"), (645, "The Taika reforms"), (701, "The Taihō Code"), (710, "The capital moves to Heijō-kyō"),
    (752, "The Great Buddha of Tōdai-ji is dedicated"), (794, "The capital moves to Heian-kyō"), (894, "The envoys to Tang are stopped"), (1016, "Fujiwara no Michinaga becomes regent"),
    (1167, "Taira no Kiyomori becomes chancellor"), (1185, "The Taira fall at Dan-no-ura"), (1192, "Yoritomo becomes shogun"), (1221, "The Jōkyū war"), (1274, "The first Mongol invasion"), (1281, "The second Mongol invasion"),
];

fn social(key: &str, rng: &mut Rng) -> Task {
    match key {
        "world_map" => bank(rng, &[
            ("A map showing true direction and distance from its centre", "azimuthal equidistant projection", &["Mercator projection", "Mollweide projection", "a road map"]),
            ("On a Mercator map, places near the poles look...", "bigger than they are", &["smaller", "their true size", "upside down"]),
            ("Shows true shapes, areas and directions together", "a globe", &["a Mercator map", "any atlas", "a road map"]),
            ("The line at 0° latitude", "the equator", &["the prime meridian", "the Tropic of Cancer", "the date line"]),
        ]),
        "continents" => {
            if rng.chance(0.3) {
                num("How many continents are there (the six-continent view)?", 6)
            } else {
                bank(rng, &[("The largest continent", "Eurasia", &["Africa", "North America", "Antarctica"]), ("The largest ocean", "the Pacific", &["the Atlantic", "the Indian", "the Arctic"]), ("The three great oceans: the Pacific, the Atlantic and the...", "Indian", &["Arctic", "Southern", "Japan Sea"]), ("The continent with no countries", "Antarctica", &["Australia", "Africa", "South America"]), ("Japan is in...", "East Asia", &["South Asia", "Southeast Asia", "Central Asia"])])
            }
        }
        "latlong" => {
            if rng.chance(0.4) {
                let (q, a) = *rng.pick(&[("Latitude goes up to ___ degrees north and south", 90i64), ("Longitude goes up to ___ degrees east and west", 180)]);
                num_u(q, a, "°")
            } else {
                bank(rng, &[("Latitude measures how far...", "north or south of the equator", &["east or west of London", "above sea level", "from Tokyo"]), ("The prime meridian (0°) passes through...", "London (Greenwich)", &["Tokyo", "New York", "Paris"]), ("Straight through the earth from Japan you'd come out near...", "Brazil and Argentina (in the sea)", &["Australia", "Canada", "India"])])
            }
        }
        "timezones" => {
            if rng.chance(0.65) {
                match rng.below(3) {
                    0 => num_u("The earth turns 360° in 24 hours. How many degrees an hour?", 15, "°"),
                    1 => num_u("Japan's standard meridian is 135°E, London's 0°. The time difference?", 9, "hours"),
                    _ => {
                        let h = rng.range(10, 23) as i64;
                        num(format!("It's {h}:00 in Tokyo. What hour is it in London (24-hour clock)?"), (h - 9).rem_euclid(24))
                    }
                }
            } else {
                bank(rng, &[("Japan's standard time meridian passes through...", "Akashi, Hyōgo", &["Tokyo", "Sapporo", "Naha"]), ("Crossing the date line going east, you...", "go back a day", &["go forward a day", "lose an hour", "nothing happens"])])
            }
        }
        "prefectures" => {
            if rng.chance(0.25) {
                let (q, a) = *rng.pick(&[("How many prefectures does Japan have?", 47i64), ("The exclusive economic zone reaches how many nautical miles out?", 200)]);
                num(q, a)
            } else {
                bank(rng, &[
                    ("Japan's northernmost island", "Etorofu", &["Okinotori", "Minamitori", "Yonaguni"]), ("Japan's southernmost island", "Okinotori-shima", &["Etorofu", "Yonaguni", "Minamitori-shima"]),
                    ("Japan's easternmost island", "Minamitori-shima", &["Etorofu", "Yonaguni-jima", "Okinotori-shima"]), ("Japan's westernmost island", "Yonaguni-jima", &["Etorofu", "Okinotori-shima", "Minamitori-shima"]),
                    ("The biggest prefecture", "Hokkaidō", &["Iwate", "Nagano", "Niigata"]), ("The capital of Ehime", "Matsuyama", &["Matsue", "Takamatsu", "Kōchi"]), ("The capital of Shimane", "Matsue", &["Matsuyama", "Tottori", "Hiroshima"]),
                    ("The capital of Ishikawa", "Kanazawa", &["Toyama", "Fukui", "Niigata"]), ("The capital of Tochigi", "Utsunomiya", &["Maebashi", "Mito", "Urawa"]), ("The capital of Mie", "Tsu", &["Ōtsu", "Nara", "Gifu"]),
                ])
            }
        }
        "climates" => {
            let b: &[(&str, &str)] = &[("Hot and rainy all year; rainforest", "tropical"), ("Little rain; deserts and steppes", "dry"), ("Four seasons, mild", "temperate"), ("Long cold winters; taiga forest", "subarctic"), ("Ice all year; tundra", "polar"), ("Houses on stilts to keep cool and dry", "tropical"), ("Sun-dried brick houses with thick walls", "dry"), ("Frozen ground all year (permafrost)", "polar")];
            pairs(rng, b, |k| format!("Which climate? {k}"), false)
        }
        "asia" => bank(rng, &[
            ("Rice grows well in monsoon Asia thanks to the summer...", "monsoon rains", &["trade winds", "jet stream", "snow"]), ("The country with the world's biggest population (from 2023)", "India", &["China", "Indonesia", "Japan"]),
            ("Ten Southeast Asian countries work together in...", "ASEAN", &["the EU", "NAFTA", "the AU"]), ("Areas China opened to foreign companies", "Special Economic Zones", &["Free cities", "Silk zones", "Tech villages"]),
            ("Most people in Indonesia follow...", "Islam", &["Buddhism", "Hinduism", "Christianity"]), ("India's fast-growing industry, centred on Bengaluru", "IT (software)", &["whaling", "coal", "silk"]), ("Saudi Arabia exports a lot of...", "oil", &["rice", "wool", "coffee"]),
        ]),
        "europe" => bank(rng, &[
            ("Europe is mild for its latitude because of a warm current and the...", "westerlies", &["monsoon", "trade winds", "typhoons"]), ("The euro is the shared currency of the...", "EU", &["UN", "ASEAN", "AU"]),
            ("Around the Mediterranean, farmers grow olives and...", "grapes", &["rice", "tea", "rubber"]), ("Crops and livestock on the same farm", "mixed farming", &["slash and burn", "plantation", "nomadic herding"]),
            ("Fjords are found in...", "Norway", &["Italy", "Spain", "Greece"]), ("A problem inside the EU", "income gaps between members", &["no trade", "no roads", "too few countries"]),
        ]),
        "africa" => bank(rng, &[
            ("Many African borders are straight lines because...", "European colonial powers drew them", &["rivers run straight", "the UN drew them", "of mountains"]), ("Côte d'Ivoire and Ghana export a lot of...", "cacao", &["wheat", "rice", "wool"]),
            ("Rare metals from southern Africa go into...", "electronics", &["food", "clothes", "paper"]), ("The world's largest hot desert", "the Sahara", &["the Gobi", "the Kalahari", "the Atacama"]),
            ("The world's longest river", "the Nile", &["the Congo", "the Niger", "the Mississippi"]), ("An economy depending on one or two exports", "monoculture economy", &["free trade", "industrial economy", "tourism"]),
        ]),
        "n_america" => bank(rng, &[
            ("US farming grows each crop where it suits best:", "appropriate-land farming", &["subsistence farming", "slash-and-burn", "nomadic herding"]), ("The high-tech area near San Francisco", "Silicon Valley", &["the Sun Belt", "the Rust Belt", "the Corn Belt"]),
            ("Spanish-speaking people who have moved to the US", "Hispanics", &["Inuit", "Aborigines", "Maori"]), ("The industrial region south of 37°N", "the Sun Belt", &["the Snow Belt", "the Rust Belt", "the Bible Belt"]),
            ("Canada's second official language", "French", &["Spanish", "German", "Portuguese"]),
        ]),
        "s_america" => bank(rng, &[
            ("The Amazon rainforest is shrinking because of...", "clearing for farms and mines", &["too much rain", "volcanoes", "cold winters"]), ("Brazil's language", "Portuguese", &["Spanish", "English", "French"]),
            ("Fuel Brazil makes from sugar cane", "bioethanol", &["coal", "uranium", "diesel"]), ("The mountains along the west coast", "the Andes", &["the Rockies", "the Alps", "the Himalayas"]),
            ("People of mixed Indigenous and European descent", "mestizo", &["Inuit", "Aborigine", "Maori"]), ("Chile exports a lot of...", "copper", &["cacao", "rice", "wool"]),
        ]),
        "oceania" => bank(rng, &[
            ("Australia's Indigenous people", "Aboriginal Australians", &["Maori", "Inuit", "Ainu"]), ("New Zealand's Indigenous people", "Maori", &["Aboriginal Australians", "Inuit", "Ainu"]),
            ("Australia once took only European immigrants under the...", "White Australia policy", &["open door policy", "green policy", "sakoku"]), ("Australia sends much of its iron ore and coal to...", "China and Japan", &["Africa", "Antarctica", "Greenland"]),
            ("Many Pacific islands are built from...", "coral", &["granite", "ice", "sandstone"]), ("Rising seas threaten island nations like Tuvalu with...", "land lost under the sea", &["more volcanoes", "snowstorms", "earthquakes"]),
        ]),
        "regions_mixed" => social(*rng.pick(&["asia", "europe", "africa", "n_america", "s_america", "oceania"]), rng),
        "centuries" => {
            if rng.chance(0.7) {
                let y = *rng.pick(&[57i64, 239, 604, 794, 1192, 1600, 1868, 2026, 1945, 710]);
                num(format!("Which century is the year {y}?"), (y - 1) / 100 + 1)
            } else {
                bank(rng, &[("The Western calendar counts from...", "the birth of Christ", &["the founding of Rome", "Japan's first emperor", "the Buddha's birth"]), ("Japan's era name in 2026", "Reiwa", &["Heisei", "Shōwa", "Taishō"])])
            }
        }
        "stone_age" => bank(rng, &[
            ("Tools of the Old Stone Age", "chipped stone tools", &["polished stone tools", "iron tools", "bronze bells"]), ("Tools of the New Stone Age", "polished stone tools", &["chipped stone tools", "iron swords", "printing"]),
            ("The first humans who walked upright lived in...", "Africa", &["Europe", "Asia", "the Americas"]), ("People of the Old Stone Age learned to use...", "fire", &["iron", "paper", "rice"]), ("Iwajuku showed Japan had an...", "Old Stone Age", &["iron age", "empire", "alphabet"]),
        ]),
        "civilisations" => {
            if rng.chance(0.5) {
                let b: &[(&str, &str)] = &[("the Nile", "Egyptian"), ("the Tigris and Euphrates", "Mesopotamian"), ("the Indus", "Indus"), ("the Yellow River", "Chinese")];
                pairs(rng, b, |k| format!("Which civilisation grew up along {k}?"), false)
            } else {
                bank(rng, &[("Cuneiform writing", "Mesopotamia", &["Egypt", "China", "the Indus"]), ("Hieroglyphs", "Egypt", &["Mesopotamia", "China", "the Indus"]), ("Oracle bone script", "China (Yin)", &["Egypt", "Mesopotamia", "the Indus"]), ("Mohenjo-daro", "the Indus", &["Egypt", "China", "Mesopotamia"]), ("The seven-day week and time in sixties", "Mesopotamia", &["Egypt", "China", "the Indus"])])
            }
        }
        "religions" => bank(rng, &[
            ("Buddhism was founded by...", "Shakyamuni (the Buddha)", &["Jesus", "Muhammad", "Confucius"]), ("Islam was founded by...", "Muhammad", &["Jesus", "Shakyamuni", "Confucius"]),
            ("Islam's holy book", "the Quran", &["the Bible", "the sutras", "the Analects"]), ("Democracy began in the city-state of...", "Athens", &["Rome", "Sparta", "Carthage"]),
            ("The Qin emperor who unified China built up...", "the Great Wall", &["the Parthenon", "the pyramids", "the Colosseum"]), ("Confucius taught in...", "China", &["India", "Greece", "Egypt"]),
        ]),
        "jomon" => bank(rng, &[
            ("Jōmon pottery is named after its...", "cord markings", &["blue glaze", "iron rims", "gold leaf"]), ("Clay figures of the Jōmon period", "dogū", &["haniwa", "dōtaku", "magatama"]),
            ("Heaps of shells left by Jōmon people", "shell mounds (kaizuka)", &["kofun", "castles", "rice paddies"]), ("The great Jōmon site in Aomori", "Sannai-Maruyama", &["Yoshinogari", "Toro", "Iwajuku"]), ("Jōmon people lived in...", "pit dwellings", &["castles", "temples", "brick houses"]),
        ]),
        "yayoi" => {
            if rng.chance(0.25) {
                num("The year Himiko sent envoys to Wei", 239)
            } else {
                bank(rng, &[("What spread in the Yayoi period?", "rice farming", &["Buddhism", "kana writing", "tea"]), ("Raised storehouses kept rice away from...", "damp and mice", &["fire", "thieves only", "the sun"]), ("The moated Yayoi settlement in Saga", "Yoshinogari", &["Sannai-Maruyama", "Iwajuku", "Heijō-kyō"]), ("Bronze bells used in rituals", "dōtaku", &["dogū", "haniwa", "kofun"]), ("The queen of Yamatai", "Himiko", &["Suiko", "Jitō", "Kōmyō"])])
            }
        }
        "kofun" => bank(rng, &[
            ("The biggest keyhole tomb (Daisen Kofun) is in...", "Sakai, Ōsaka", &["Nara", "Kyoto", "Kamakura"]), ("Clay figures set around kofun", "haniwa", &["dogū", "dōtaku", "ema"]), ("The shape of the biggest kofun", "keyhole", &["round", "square", "star"]),
            ("People from the Korean peninsula brought...", "new skills like sue pottery and kanji", &["guns", "printing", "tea ceremony"]), ("The Yamato kings were called...", "ōkimi (great kings)", &["shōgun", "daimyō", "regents"]),
        ]),
        "asuka" => {
            if rng.chance(0.3) {
                let (q, a) = *rng.pick(&[("How many articles in Prince Shōtoku's constitution?", 17i64), ("The year of the Seventeen-Article Constitution", 604)]);
                num(q, a)
            } else {
                bank(rng, &[("The twelve cap ranks chose officials by...", "ability", &["family", "wealth", "age"]), ("The envoy Shōtoku sent to the Sui", "Ono no Imoko", &["Abe no Nakamaro", "Kūkai", "Ganjin"]), ("The world's oldest wooden temple, built by Shōtoku", "Hōryū-ji", &["Tōdai-ji", "Kinkaku-ji", "Byōdō-in"]), ("Shōtoku worked with the powerful...", "Soga clan", &["Fujiwara clan", "Taira clan", "Minamoto clan"])])
            }
        }
        "taika" => {
            if rng.chance(0.3) {
                num("The year of the Taika reforms", 645)
            } else {
                bank(rng, &[("The Taika reforms were led by Prince Naka no Ōe and...", "Nakatomi no Kamatari", &["Soga no Umako", "Prince Shōtoku", "Taira no Kiyomori"]), ("Land and people belong to the state:", "kōchi kōmin", &["shōen", "kokubunji", "sekkan"]), ("Rice fields the state gave everyone over six", "kubunden", &["shōen", "konden", "daimyō land"]), ("The legal codes of 701", "the Taihō Code", &["the Seventeen Articles", "the Goseibai Shikimoku", "the Meiji constitution"]), ("The tax paid in rice", "so", &["chō", "yō", "nengu"])])
            }
        }
        "nara" => {
            if rng.chance(0.3) {
                num("The year the capital moved to Heijō-kyō", 710)
            } else {
                bank(rng, &[("Heijō-kyō was modelled on the Tang capital...", "Chang'an", &["Beijing", "Nanjing", "Kaifeng"]), ("Coins of the Nara era", "Wadō kaichin", &["Kan'ei tsūhō", "yen", "Song coins"]), ("The 743 law letting people keep new fields forever", "the Konden Einen Shizai law", &["kōchi kōmin", "the Taihō Code", "the sword hunt"]), ("Envoys sent to Tang China", "kentōshi", &["kenzuishi", "shogun", "kebiishi"])])
            }
        }
        "nara_culture" => bank(rng, &[
            ("The emperor who built Tōdai-ji's Great Buddha", "Shōmu", &["Kanmu", "Tenmu", "Saga"]), ("The Chinese monk who reached Japan on his sixth try", "Ganjin", &["Gyōki", "Kūkai", "Saichō"]),
            ("The oldest collection of Japanese poems", "the Man'yōshū", &["the Kokinshū", "the Kojiki", "the Tale of Genji"]), ("Tōdai-ji's treasure house", "the Shōsōin", &["Byōdō-in", "Hōryū-ji", "Kinkaku-ji"]),
            ("Temples Shōmu had built in every province", "kokubunji", &["shōen", "kofun", "jinja"]),
        ]),
        "heian" => {
            if rng.chance(0.3) {
                num("The year the capital moved to Heian-kyō", 794)
            } else {
                bank(rng, &[("The emperor who moved the capital to Heian-kyō", "Kanmu", &["Shōmu", "Tenmu", "Go-Daigo"]), ("The Fujiwara ruled as regents:", "sekkan politics", &["insei", "the bakufu", "ritsuryō"]), ("The Fujiwara regent at the clan's height", "Fujiwara no Michinaga", &["Fujiwara no Kamatari", "Taira no Kiyomori", "Minamoto no Yoritomo"]), ("The Phoenix Hall at Uji", "Byōdō-in", &["Hōryū-ji", "Tōdai-ji", "Kinkaku-ji"]), ("Who had the envoys to Tang stopped in 894", "Sugawara no Michizane", &["Fujiwara no Michinaga", "Kūkai", "Saichō"]), ("Private estates of nobles and temples", "shōen", &["kubunden", "kokubunji", "jōkamachi"])])
            }
        }
        "heian_culture" => bank(rng, &[
            ("The Tale of Genji was written by...", "Murasaki Shikibu", &["Sei Shōnagon", "Ono no Komachi", "Ki no Tsurayuki"]), ("The Pillow Book was written by...", "Sei Shōnagon", &["Murasaki Shikibu", "Ono no Komachi", "Ki no Tsurayuki"]),
            ("The writing that grew out of kanji in Heian times", "kana", &["kanbun", "rōmaji", "hangul"]), ("Heian court culture is called...", "kokufū bunka (national culture)", &["Tenpyō culture", "Asuka culture", "Genroku culture"]),
            ("Ki no Tsurayuki compiled the...", "Kokinshū", &["Man'yōshū", "Kojiki", "Shin Kokinshū"]), ("Pure Land Buddhism taught chanting...", "'Namu Amida Butsu'", &["'Namu Myōhō Renge Kyō'", "nothing: only zazen", "the Analects"]),
        ]),
        "timeline" => {
            let mut ev: Vec<(i64, &str)> = Vec::new();
            while ev.len() < 4 {
                let e = *rng.pick(EVENTS);
                if !ev.contains(&e) {
                    ev.push(e);
                }
            }
            ev.sort();
            let pieces: Vec<&str> = ev.iter().map(|e| e.1).collect();
            order(rng, "Put them in order, earliest first", &pieces)
        }
        "samurai" => bank(rng, &[
            ("Taira no Kiyomori traded with...", "the Song (China)", &["the Tang", "the Sui", "Portugal"]), ("The battle where the Taira fell (1185)", "Dan-no-ura", &["Sekigahara", "Okehazama", "Nagashino"]),
            ("Rule by retired emperors", "insei", &["sekkan", "bakufu", "kanpaku"]), ("The Minamoto general who beat the Taira", "Minamoto no Yoshitsune", &["Ashikaga Takauji", "Oda Nobunaga", "Kusunoki Masashige"]),
            ("Kiyomori was the first samurai to become...", "chancellor (daijō-daijin)", &["shogun", "emperor", "regent"]),
        ]),
        "kamakura" => {
            if rng.chance(0.25) {
                num("The year Yoritomo became shogun", 1192)
            } else {
                bank(rng, &[("Land for loyal service: the bond of Kamakura", "go'on and hōkō", &["kōchi kōmin", "sekkan", "sankin kōtai"]), ("The Hōjō ruled as...", "shikken (the shogun's regents)", &["kanpaku", "daimyō", "court nobles"]), ("The samurai law code of 1232", "the Goseibai Shikimoku", &["the Taihō Code", "the Seventeen Articles", "the Buke Shohatto"]), ("The guardian statues at Tōdai-ji's south gate were carved by...", "Unkei and Kaikei", &["Jōchō", "Sesshū", "Hokusai"]), ("Hōnen founded...", "the Jōdo sect", &["Zen", "the Nichiren sect", "Shingon"]), ("A war story chanted to the biwa", "the Tale of the Heike", &["the Tale of Genji", "the Man'yōshū", "the Kojiki"])])
            }
        }
        "mongol" => {
            if rng.chance(0.3) {
                let (q, a) = *rng.pick(&[("The year of the first Mongol invasion", 1274i64), ("The year of the second Mongol invasion", 1281)]);
                num(q, a)
            } else {
                bank(rng, &[("The Mongol ruler who sent his fleets to Japan", "Kublai Khan", &["Genghis Khan", "Ögedei", "Tamerlane"]), ("The regent who refused the Mongols", "Hōjō Tokimune", &["Hōjō Masako", "Hōjō Yasutoki", "Minamoto no Sanetomo"]), ("After the invasions, the samurai were unhappy because...", "they got little land as reward", &["they lost", "they paid tax to the Mongols", "the shogun fled"]), ("The 1297 order cancelling samurai debts", "the Einin tokuseirei", &["the sword hunt", "kōchi kōmin", "sakoku"])])
            }
        }
        "japan_regions" => {
            let b: &[(&str, &str)] = &[("Ōsaka and Kyoto", "Kinki"), ("Nagoya and Mount Fuji", "Chūbu"), ("Hiroshima and Okayama", "Chūgoku-Shikoku"), ("Sendai and Aomori", "Tōhoku"), ("Tokyo and Yokohama", "Kantō"), ("Fukuoka and Okinawa", "Kyūshū"), ("Sapporo", "Hokkaidō")];
            pairs(rng, b, |k| format!("Which region has {k}?"), false)
        }
        "japan_land" => bank(rng, &[
            ("Japan's longest river", "the Shinano", &["the Tone", "the Ishikari", "the Kiso"]), ("Japan's biggest lake", "Biwa", &["Kasumigaura", "Saroma", "Inawashiro"]), ("Japan's biggest plain", "the Kantō plain", &["the Nōbi plain", "the Ishikari plain", "the Echigo plain"]),
            ("The 'Japan Alps' are in...", "Chūbu", &["Tōhoku", "Kyūshū", "Shikoku"]), ("About how much of Japan is mountains?", "three quarters", &["a quarter", "half", "a tenth"]),
            ("Fan-shaped land where a river leaves the mountains", "an alluvial fan", &["a delta", "a terrace", "a ria"]), ("A coast of drowned valleys, like Sanriku", "a ria coast", &["a sandy coast", "a fjord", "a delta"]),
        ]),
        "japan_climate" => {
            let b: &[(&str, &str)] = &[("Snowy winters on the Japan Sea side", "Japan Sea climate"), ("Dry, sunny winters, wet summers on the Pacific side", "Pacific climate"), ("Little rain, hot summers and cold winters, inland (Nagano)", "inland climate"), ("Little rain all year, mild (Takamatsu)", "Seto Inland Sea climate"), ("Cold, no real rainy season", "Hokkaidō climate"), ("Warm all year, lots of rain (Naha)", "Nansei Islands climate")];
            pairs(rng, b, |k| format!("Which climate? {k}"), false)
        }
        "disasters" => bank(rng, &[
            ("A wall of sea after an earthquake", "a tsunami", &["a typhoon", "yamase", "a landslide"]), ("Cold summer winds that ruin Tōhoku's rice", "yamase", &["föhn", "the monsoon", "a typhoon"]),
            ("Public help, mutual help, and...", "self-help", &["luck", "waiting", "nothing"]), ("Disaster Prevention Day (1 September) remembers...", "the Great Kantō earthquake (1923)", &["the Hanshin earthquake", "the Tōhoku earthquake", "an eruption of Fuji"]),
        ]),
        _ => social("world_map", rng),
    }
}

// ------------------------------------------------------------------------------------- PE

fn pe(key: &str, rng: &mut Rng) -> Task {
    let (prompt, speed, width, tries): (&str, f32, f32, u8) = match key {
        "warmup" => ("Stretch on the count", 0.8, 0.22, 4),
        "fitness" => ("Sit-ups: up on the beat", 1.2, 0.16, 5),
        "sprint" => ("Push off on the gun", 1.8, 0.10, 3),
        "relay" => ("Take the baton in the zone", 1.4, 0.12, 3),
        "jump" => ("Take off at the board", 1.6, 0.10, 3),
        "soccer" => ("Shoot as the keeper moves", 1.3, 0.14, 4),
        "badminton" => ("Hit the shuttle at the top", 1.5, 0.12, 4),
        "swim" => ("Breathe on the stroke", 1.0, 0.16, 5),
        "tug" => ("Pull on the class's call", 0.9, 0.20, 5),
        "jumprope" => ("Jump as the rope comes round", 1.4, 0.14, 6),
        "march" => ("Step with the drum", 1.0, 0.18, 5),
        "basketball" => ("Release at the top of the jump", 1.5, 0.11, 4),
        "dance" => ("Hit the beat", 1.2, 0.14, 6),
        "mat" => ("Tuck at the right moment", 1.1, 0.14, 3),
        "distance" => ("Keep your pace", 0.7, 0.24, 5),
        "kendo" => ("Strike as they open up", 1.9, 0.09, 4),
        "volleyball" => ("Bump as the ball drops", 1.3, 0.13, 4),
        "throw" => ("Release at 45°", 1.6, 0.10, 3),
        "hurdles" => ("Lead leg up at the hurdle", 1.7, 0.10, 4),
        "dodgeball" => ("Dodge as it's thrown", 1.8, 0.12, 4),
        "health" => return health(rng),
        _ => ("Stretch on the count", 0.8, 0.22, 4),
    };
    if key == "kendo" && rng.chance(0.25) {
        return bank(rng, &[("A kendo match begins and ends with...", "a bow (rei)", &["a shout", "a handshake", "nothing"]), ("Kendo's valid targets: men, kote, dō and...", "tsuki (the throat)", &["the knees", "the back", "the feet"])]);
    }
    let a = 0.15 + rng.f32() * (0.7 - width);
    Task::Timing { prompt: prompt.into(), speed: speed * (0.9 + rng.f32() * 0.2), zone: (a, a + width), tries }
}

fn health(rng: &mut Rng) -> Task {
    bank(rng, &[
        ("Before exercise you should...", "warm up", &["eat a big meal", "sit still", "skip water"]),
        ("For a sprained ankle, first...", "rest, ice, compress, raise (RICE)", &["run it off", "heat it", "massage it hard"]),
        ("The body changes of puberty begin...", "at different ages for different people", &["at exactly 12", "at exactly 15", "the same day for everyone"]),
        ("Stress is eased by...", "talking to someone and exercise", &["skipping sleep", "eating only snacks", "keeping it secret"]),
        ("For a nosebleed...", "lean forward and pinch your nose", &["lean back", "lie flat", "blow hard"]),
        ("Heatstroke: drink water and...", "rest in the shade", &["keep running", "wear thick clothes", "skip water"]),
        ("A junior high student needs about ___ hours of sleep", "8 to 10", &["4 to 5", "5 to 6", "12 to 14"]),
        ("Healthy rooms need fresh air, the right temperature and...", "enough light", &["loud music", "no windows", "dust"]),
    ])
}

// ------------------------------------------------------------------------------------- music

fn recall(rng: &mut Rng, prompt: &str, labels: [&str; 4], len: usize) -> Task {
    let seq: Vec<u8> = (0..len).map(|_| rng.below(4) as u8).collect();
    Task::Recall { prompt: prompt.into(), labels: labels.map(String::from), seq }
}

fn music(key: &str, rng: &mut Rng) -> Task {
    let len = 3 + rng.below(3);
    match key {
        "school_song" | "singing" | "parts" | "sakura" | "folk" if rng.chance(0.6) => recall(rng, "Sing the phrase back", ["ド", "レ", "ミ", "ソ"], len),
        "recorder" if rng.chance(0.7) => recall(rng, "Play the phrase back on the recorder", ["ソ", "ラ", "シ", "ド"], len),
        "rhythm" | "compose" => recall(rng, "Clap the rhythm back", ["タン", "タタ", "ウン", "ターアー"], len + 1),
        "school_song" | "singing" => bank(rng, &[("Singing well: stand with your feet...", "shoulder-width apart, back straight", &["together, leaning back", "crossed", "on tiptoe"]), ("Breathe in for singing through...", "nose and mouth, filling the belly", &["only the nose, chest up", "not at all", "the shoulders"])]),
        "parts" => bank(rng, &[("The highest women's part", "soprano", &["alto", "tenor", "bass"]), ("The lower women's part", "alto", &["soprano", "tenor", "bass"]), ("In a chorus, listen to the other parts to...", "blend and stay in tune", &["sing louder than them", "stop singing", "change the key"])]),
        "recorder" => bank(rng, &[("On the alto recorder, the back hole is closed with your...", "left thumb", &["right thumb", "left little finger", "nothing"]), ("Start each note with the tongue, saying...", "'tu'", &["'la'", "'pa'", "'mm'"])]),
        "notation" => {
            let (q, a) = *rng.pick(&[("How many quarter notes (♩) fill a whole note?", 4i64), ("How many eighth notes fill a half note?", 4), ("In 3/4 time, how many beats in a bar?", 3), ("How many beats does a dotted half note last (♩ = 1)?", 3), ("In 6/8 time, how many eighth notes in a bar?", 6)]);
            num(q, a)
        }
        "terms" => {
            let b: &[(&str, &str)] = &[("Allegro", "fast"), ("Andante", "at a walking pace"), ("Adagio", "slowly"), ("p (piano)", "soft"), ("f (forte)", "loud"), ("crescendo", "gradually louder"), ("mf (mezzo forte)", "fairly loud"), ("ritardando", "gradually slower")];
            pairs(rng, b, |k| format!("What does '{k}' mean?"), false)
        }
        "vivaldi" => bank(rng, &[("'Spring' is part of...", "The Four Seasons", &["The Planets", "The Nutcracker", "Water Music"]), ("Vivaldi came from...", "Venice, Italy", &["Vienna, Austria", "Bonn, Germany", "Paris, France"]), ("Each of the four concertos comes with a...", "sonnet (a poem)", &["painting", "dance", "story book"]), ("The solo instrument in 'Spring'", "violin", &["flute", "piano", "trumpet"]), ("Music that pictures something outside music", "programme music", &["absolute music", "a fugue", "a march"])]),
        "erlkonig" => {
            if rng.chance(0.25) {
                num("How many characters does the one singer play in 'Erlkönig'?", 4)
            } else {
                bank(rng, &[("The poem of 'Erlkönig' is by...", "Goethe", &["Schiller", "Heine", "Shakespeare"]), ("Schubert wrote over 600...", "Lieder (art songs)", &["operas", "symphonies", "concertos"]), ("The piano's hammering triplets suggest...", "a galloping horse", &["rain", "a clock", "the sea"]), ("The Erlking's voice is sung...", "softly and sweetly, to tempt the child", &["loudly and angrily", "by a choir", "not at all"])])
            }
        }
        "koto" => {
            if rng.chance(0.3) {
                num("How many strings does a koto have?", 13)
            } else {
                bank(rng, &[("'Rokudan no Shirabe' was composed by...", "Yatsuhashi Kengyō", &["Miyagi Michio", "Takemitsu Tōru", "Rentarō Taki"]), ("The picks worn on the fingers", "tsume", &["bachi", "ji", "koma"]), ("The movable bridges under the strings", "ji", &["tsume", "bachi", "neck"]), ("'Rokudan' starts slow and gets faster:", "jo-ha-kyū", &["rondo", "theme and variations only", "a march"])])
            }
        }
        "sakura" => bank(rng, &[("'Sakura Sakura' uses the Japanese scale called...", "miyako-bushi", &["the major scale", "the blues scale", "the chromatic scale"]), ("An instrument often heard with 'Sakura Sakura'", "the koto", &["the bagpipes", "the organ", "the drum kit"])]),
        "instruments" => {
            let b: &[(&str, &str)] = &[("violin", "strings"), ("cello", "strings"), ("flute", "woodwind"), ("clarinet", "woodwind"), ("oboe", "woodwind"), ("trumpet", "brass"), ("horn", "brass"), ("trombone", "brass"), ("timpani", "percussion"), ("xylophone", "percussion")];
            pairs(rng, b, |k| format!("Which family is the {k} in?"), false)
        }
        "gagaku" => bank(rng, &[("Gagaku is the music of the...", "imperial court", &["kabuki theatre", "fishing villages", "rock bands"]), ("The gagaku wind instrument with 17 pipes", "shō", &["shakuhachi", "shamisen", "taiko"]), ("The small double-reed gagaku instrument", "hichiriki", &["shō", "koto", "biwa"]), ("A famous gagaku piece", "'Etenraku'", &["'Rokudan'", "'Sakura Sakura'", "'Spring'"])]),
        "folk" => bank(rng, &[("The fishermen's net-hauling song of Hokkaidō", "'Sōran-bushi'", &["'Kiso-bushi'", "'Sakura Sakura'", "'Etenraku'"]), ("Folk songs passed down by ear are called...", "min'yō", &["enka", "gagaku", "J-pop"]), ("'Kobushi' in folk singing is...", "a decorative turn of the voice", &["a drum", "a dance step", "a tempo"])]),
        _ => recall(rng, "Sing the phrase back", ["ド", "レ", "ミ", "ソ"], len),
    }
}

// ------------------------------------------------------------------------------------- art

/// A hue's name (for the art prompts).
pub fn hue_name(h: f32) -> &'static str {
    const NAMES: [&str; 12] = ["red", "orange", "yellow", "yellow-green", "green", "teal", "cyan", "sky blue", "blue", "violet", "purple", "magenta"];
    NAMES[((h.rem_euclid(1.0) * 12.0 + 0.5) as usize) % 12]
}

fn art(key: &str, rng: &mut Rng) -> Task {
    match key {
        "hue" => {
            let t = rng.below(40) as f32 * 0.025;
            Task::Slider { prompt: format!("Mix the colour on the card: {}", hue_name(t)), kind: SliderKind::Hue, target: t, tolerance: 0.1 }
        }
        "complement" => {
            let t = rng.below(40) as f32 * 0.025;
            Task::Slider { prompt: format!("Paint the complementary colour of {}", hue_name(t)), kind: SliderKind::Hue, target: (t + 0.5).rem_euclid(1.0), tolerance: 0.1 }
        }
        "sketch" | "perspective" if rng.chance(0.5) => {
            let t = (rng.below(30) as f32 + 5.0) * 0.025;
            Task::Slider { prompt: "Shade to match the tone on the card".into(), kind: SliderKind::Value, target: t, tolerance: 0.12 }
        }
        "sketch" => bank(rng, &[("Holding the pencil out at arm's length helps you measure...", "proportions", &["colours", "the time", "the paper"]), ("A quick drawing of a pose", "croquis", &["collage", "fresco", "mosaic"]), ("Shading shows...", "light and solid form", &["the date", "the artist's name", "movement only"])]),
        "perspective" => bank(rng, &[("In one-point perspective, lines going away meet at the...", "vanishing point", &["edge of the paper", "bottom corner", "centre of every picture"]), ("Things far away look...", "smaller and paler", &["bigger", "darker", "sharper"])]),
        "lettering" => bank(rng, &[("Letters with small 'feet' on the strokes, like Minchō", "serif", &["sans serif", "script", "stencil"]), ("Gothic (sans serif) letters have strokes...", "of even thickness", &["thick and thin", "with feet", "in brush style"]), ("A good poster's message is...", "short and readable from far away", &["long and detailed", "tiny", "in many fonts"])]),
        "pattern" => bank(rng, &[("Repeating the same motif", "repetition", &["contrast", "gradation", "balance"]), ("Changing step by step, light to dark", "gradation", &["repetition", "symmetry", "contrast"]), ("Both halves the same, mirrored", "symmetry", &["rhythm", "gradation", "contrast"])]),
        "ukiyoe" => bank(rng, &[("'The Great Wave off Kanagawa' is by...", "Katsushika Hokusai", &["Utagawa Hiroshige", "Kitagawa Utamaro", "Sesshū"]), ("'The Fifty-three Stations of the Tōkaidō' is by...", "Utagawa Hiroshige", &["Katsushika Hokusai", "Kitagawa Utamaro", "Sesshū"]), ("Ukiyo-e were usually...", "woodblock prints", &["oil paintings", "photographs", "sculptures"]), ("A European painter inspired by ukiyo-e", "Van Gogh", &["Leonardo", "Michelangelo", "Rembrandt"])]),
        "impressionism" => bank(rng, &[("The Impressionists painted...", "light and how it changes, outdoors", &["only portraits of kings", "perfect straight lines", "dark church scenes"]), ("'Water Lilies' was painted by...", "Monet", &["Picasso", "Dalí", "Vermeer"]), ("The name came from Monet's 'Impression, ___'", "Sunrise", &["Sunset", "Rain", "Night"])]),
        "appreciation" => bank(rng, &[("The three primary colours of paint", "magenta, yellow, cyan (red, yellow, blue)", &["black, white, grey", "green, orange, purple", "brown, pink, gold"]), ("Colours opposite each other on the wheel", "complementary colours", &["analogous colours", "neutral colours", "warm colours"]), ("Who painted the 'Mona Lisa'?", "Leonardo da Vinci", &["Michelangelo", "Raphael", "Monet"]), ("The Muromachi master of ink painting", "Sesshū", &["Hokusai", "Unkei", "Hiroshige"])]),
        "print" => {
            let b: &[(&'static str, &'static [&'static str])] = &[("Making a woodblock print: in order", &["Draw the design", "Transfer it to the block, reversed", "Carve away what stays white", "Ink the block", "Press the paper with a baren"])];
            orders(rng, b)
        }
        "clay" => {
            let b: &[(&'static str, &'static [&'static str])] = &[("Making a pot in clay: in order", &["Knead the clay to get the air out", "Shape it", "Dry it slowly", "Bisque firing", "Glaze it", "Glaze firing"])];
            orders(rng, b)
        }
        "papercraft" => {
            let b: &[(&'static str, &'static [&'static str])] = &[("Paper craft: in order", &["Plan and sketch", "Draw the net", "Cut it out", "Score the folds", "Fold", "Glue the tabs"])];
            orders(rng, b)
        }
        _ => art("appreciation", rng),
    }
}

// ------------------------------------------------------------------------------------- technology and home economics

fn techhome(key: &str, rng: &mut Rng) -> Task {
    match key {
        "tech_intro" => bank(rng, &[("Technology is about...", "using knowledge to solve real problems", &["only computers", "only factories", "magic"]), ("Good technology balances cost, safety and...", "the environment", &["colour", "fashion", "luck"])]),
        "materials" => {
            let b: &[(&str, &str)] = &[("Light, easy to cut, but swells with moisture", "wood"), ("Conducts heat and electricity, bends without breaking", "metal"), ("Doesn't rust, moulds when heated", "plastic"), ("Can rust when wet", "metal"), ("Has a grain", "wood"), ("Made from oil", "plastic")];
            pairs(rng, b, |k| format!("Which material? {k}"), false)
        }
        "wood" => bank(rng, &[("Wood is strongest along the...", "grain (the fibres)", &["cross-grain", "knots", "end grain"]), ("Wood shrinks most...", "around the rings (tangentially)", &["along the grain", "not at all", "at the knots only"]), ("The side of a board that faced the bark", "kiomote (the outer face)", &["kiura (the inner face)", "the end grain", "the edge"])]),
        "drawing" => bank(rng, &[("A drawing with front, top and side views", "orthographic (third-angle) projection", &["isometric drawing", "perspective drawing", "a sketch"]), ("Front face true, depth at 45° and half length:", "cabinet drawing", &["isometric drawing", "orthographic projection", "a plan"]), ("Dimensions on a drawing are in...", "millimetres", &["centimetres", "metres", "inches"])]),
        "box_steps" => {
            let b: &[(&'static str, &'static [&'static str])] = &[("Making the shelf: in order", &["Design and draw it", "Mark out the parts", "Saw the parts", "Plane the edges", "Nail and glue", "Sand and finish"])];
            orders(rng, b)
        }
        "saws" => bank(rng, &[("Teeth for cutting across the grain", "crosscut teeth", &["rip teeth", "either", "neither"]), ("Teeth for cutting along the grain", "rip teeth", &["crosscut teeth", "either", "neither"]), ("A Japanese saw cuts on the...", "pull stroke", &["push stroke", "both strokes", "side stroke"])]),
        "tools" => bank(rng, &[("A plane takes off...", "thin shavings, to make it smooth and true", &["big chunks", "paint", "nails"]), ("A file cuts on the...", "push stroke", &["pull stroke", "both", "neither"]), ("A square marks lines at...", "90°", &["45°", "60°", "30°"])]),
        "safety" => bank(rng, &[("Before using the drill press, tie back...", "long hair and loose clothes", &["nothing", "your shoelaces only", "the window"]), ("Gloves near a spinning drill are...", "dangerous: they can catch", &["required", "fine", "lucky"]), ("Safety glasses protect from...", "flying chips", &["noise", "heat only", "boredom"])]),
        "metal" => bank(rng, &[("Bending sheet metal along a line, you use...", "a folding block and a mallet", &["a saw", "glue", "a plane"]), ("Metal springing back after bending shows its...", "elasticity", &["plasticity", "rust", "grain"]), ("Bend metal back and forth and it gets harder then breaks:", "work hardening", &["annealing", "melting", "rusting"])]),
        "nutrients" => {
            let b: &[(&str, &str)] = &[("rice", "carbohydrates"), ("bread", "carbohydrates"), ("fish", "protein"), ("tofu", "protein"), ("butter", "fats"), ("cooking oil", "fats"), ("spinach", "vitamins"), ("carrots", "vitamins"), ("milk", "minerals (calcium)"), ("seaweed", "minerals (calcium)")];
            pairs(rng, b, |k| format!("Main nutrient in {k}?"), false)
        }
        "food_groups" => {
            let b: &[(&str, i64)] = &[("fish", 1), ("eggs", 1), ("tofu", 1), ("milk", 2), ("small fish eaten whole", 2), ("seaweed", 2), ("carrots", 3), ("spinach", 3), ("cabbage", 4), ("apples", 4), ("rice", 5), ("potatoes", 5), ("sugar", 5), ("butter", 6), ("oil", 6)];
            let (f, g) = *rng.pick(b);
            num(format!("Which of the six food groups (1–6) is {f} in?"), g)
        }
        "labels" => bank(rng, &[("'Best before' (shōmi kigen) means...", "it tastes best before this date", &["it's dangerous after it", "it was made then", "it was sold then"]), ("'Use by' (shōhi kigen) is for food that...", "goes off quickly", &["keeps for years", "is frozen", "is a drink"]), ("Ingredients are listed in order of...", "weight, the most first", &["price", "colour", "the alphabet"])]),
        "kitchen_safety" => bank(rng, &[("Hand someone a knife...", "blade away, handle towards them", &["blade first", "by throwing it", "point up"]), ("The hand holding the food is shaped like a...", "cat's paw", &["flat palm", "fist", "pinch"]), ("A pan's handle should point...", "away from the edge", &["out over the edge", "at the person", "at the flame"]), ("Stopping food poisoning: keep germs out, stop them growing, and...", "kill them with heat", &["ignore them", "add sugar", "wash in cold water only"])]),
        "cooking" => {
            if rng.chance(0.6) {
                let b: &[(&'static str, &'static [&'static str])] = &[
                    ("Miso soup: in order", &["Make the dashi", "Add what takes longest to cook", "Add the tofu", "Turn the heat down", "Dissolve the miso", "Serve before it boils"]),
                    ("Hamburg steak: in order", &["Chop and fry the onion", "Mix meat, onion, egg and breadcrumbs", "Knead and shape the patties", "Press a dip in the middle", "Fry both sides", "Check the juices run clear"]),
                ];
                orders(rng, b)
            } else {
                bank(rng, &[("Miso goes in at the end so it doesn't...", "boil and lose its flavour", &["freeze", "dissolve", "burn the pan"]), ("Dashi is often made from kombu and...", "katsuobushi (bonito flakes)", &["sugar", "milk", "tomato"])])
            }
        }
        "local_food" => bank(rng, &[("Eating food grown nearby", "chisan chishō", &["fast food", "food loss", "dieting"]), ("The time food is at its best", "shun (in season)", &["best before", "harvest festival", "kigo"]), ("'Food miles' measure...", "how far food travels", &["how much it costs", "how long it keeps", "its calories"])]),
        "care_labels" => {
            let b: &[(&str, &str)] = &[("cotton", "plant fibre"), ("linen", "plant fibre"), ("wool", "animal fibre"), ("silk", "animal fibre"), ("polyester", "synthetic fibre"), ("nylon", "synthetic fibre")];
            if rng.chance(0.7) {
                pairs(rng, b, |k| format!("What kind of fibre is {k}?"), false)
            } else {
                bank(rng, &[("Wool shrinks if washed...", "in hot water and rubbed", &["in cold water gently", "never", "with soap"]), ("Dries fastest:", "polyester", &["cotton", "wool", "linen"])])
            }
        }
        "washing" => {
            let b: &[(&'static str, &'static [&'static str])] = &[("Doing the washing: in order", &["Check the care label", "Sort by colour and dirt", "Treat the stains", "Wash with the right detergent", "Rinse", "Hang to dry in shape"])];
            if rng.chance(0.6) {
                orders(rng, b)
            } else {
                bank(rng, &[("How much detergent?", "the amount on the box", &["as much as possible", "none", "double"])])
            }
        }
        "stitches" => bank(rng, &[("No gaps, like a machine's stitch", "backstitch (hon-gaeshi-nui)", &["running stitch", "blanket stitch", "slip stitch"]), ("The simple in-and-out stitch", "running stitch (nami-nui)", &["backstitch", "blind hem", "cross stitch"]), ("A stitch that hides a hem", "blind hem (matsuri-nui)", &["running stitch", "backstitch", "chain stitch"]), ("The knot at the end of sewing", "tama-dome", &["tama-musubi", "kaeshi-nui", "nami-nui"]), ("The knot at the start of sewing", "tama-musubi", &["tama-dome", "kaeshi-nui", "nami-nui"])]),
        "button" => {
            let b: &[(&'static str, &'static [&'static str])] = &[("Sewing on a button: in order", &["Thread and knot the needle", "Mark the place", "Sew through the holes 3 or 4 times, leaving slack", "Wind the thread round to make a stem", "Knot it off on the back"])];
            orders(rng, b)
        }
        "home_safety" => bank(rng, &[("Small children are most often hurt at home by...", "falls and swallowing things", &["sunburn", "homework", "music"]), ("A home with no steps or bumps is called...", "barrier-free", &["open plan", "eco", "compact"]), ("Smoke alarms belong in...", "bedrooms and on the stairs", &["the fridge", "the garden", "the car"]), ("Opening windows to change the air", "ventilation", &["insulation", "dehumidifying", "heating"])]),
        "computers" => {
            if rng.chance(0.25) {
                num("How many bits in a byte?", 8)
            } else {
                let b: &[(&str, &str)] = &[("CPU", "calculates and controls"), ("memory (RAM)", "holds data while working"), ("storage (SSD)", "keeps data when it's off"), ("keyboard", "an input device"), ("display", "an output device")];
                pairs(rng, b, |k| format!("What does the {k} do?"), false)
            }
        }
        "internet" => bank(rng, &[("A strong password is...", "long and mixed, not your name or birthday", &["your name", "1234", "the same everywhere"]), ("Before posting a photo of a friend...", "ask them first", &["post it anyway", "add their address", "tag their school"]), ("'You've won a prize! Click here' is probably...", "a scam (phishing)", &["a real prize", "from your school", "safe"]), ("Copyright is...", "the creator's right over their work", &["the right to copy anything", "a kind of password", "a website"])]),
        "program" => match rng.below(3) {
            0 => {
                let (a, b) = (rng.range(2, 6) as i64, rng.range(2, 6) as i64);
                num(format!("repeat {a} times [ move {b} squares ]  — how many squares in all?"), a * b)
            }
            1 => {
                let b: &[(&'static str, &'static [&'static str])] = &[("The robot must reach the flag: in order", &["start", "move forward 2", "turn right", "move forward 1", "stop"]), ("Turning on the class's night light: in order", &["read the light sensor", "if it's dark", "turn the LED on", "wait 1 second", "repeat"])];
                orders(rng, b)
            }
            _ => bank(rng, &[("Steps one after another", "sequence", &["loop", "branch", "variable"]), ("Repeating steps", "loop", &["sequence", "branch", "input"]), ("Choosing by a condition", "branch", &["loop", "sequence", "output"])]),
        },
        "circuits" => {
            if rng.chance(0.4) {
                let (v, a) = (*rng.pick(&[100i64, 12, 6]), rng.range(1, 5) as i64);
                num_u(format!("{v} V × {a} A: how many watts?"), v * a, "W")
            } else {
                bank(rng, &[("A bulb and a motor on one loop are in...", "series", &["parallel", "neither", "both"]), ("Current is measured in...", "amperes", &["volts", "watts", "ohms only"]), ("A breaker cuts off when there's...", "too much current", &["too little light", "a cold day", "no plug"])])
            }
        }
        "energy" => {
            if rng.chance(0.3) {
                let o = rng.range(2, 9) as i64 * 10;
                num_u(format!("100 J of electricity in, {o} J of light out. The efficiency?"), o, "%")
            } else {
                let b: &[(&str, &str)] = &[("A solar panel", "light → electricity"), ("A motor", "electricity → motion"), ("A generator", "motion → electricity"), ("A light bulb", "electricity → light (and heat)"), ("A speaker", "electricity → sound")];
                pairs(rng, b, |k| format!("{k} changes energy how?"), false)
            }
        }
        "cultivation" => bank(rng, &[("Seeds need water, air and...", "the right temperature", &["darkness only", "salt", "music"]), ("Thinning seedlings gives the rest...", "room to grow", &["more water", "colour", "nothing"]), ("Fertiliser rich in nitrogen mainly helps...", "the leaves", &["the flowers only", "the roots only", "the seeds"])]),
        "consumer" => {
            if rng.chance(0.25) {
                num("Cooling-off lets you cancel many door-to-door sales within how many days?", 8)
            } else {
                bank(rng, &[("A contract is made when...", "both sides agree, even by word of mouth", &["only when written", "only when paid", "never for children"]), ("Paying by card later is still...", "spending real money", &["free", "a loan from school", "points"]), ("A receipt proves...", "what you bought and paid", &["the price next week", "the shop's name only", "nothing"])])
            }
        }
        _ => techhome("tech_intro", rng),
    }
}

// ------------------------------------------------------------------------------------- moral education

/// The moral-education dilemmas: (situation, what you could do, and what each leans to).
pub const DILEMMAS: &[(&str, [(&str, Virtue); 3])] = &[
    ("First day: the student next to you looks lost and alone.", [("Say hello and show them round", Virtue::Kindness), ("Tell the teacher they seem lost", Virtue::Responsibility), ("Leave them be: they may want space", Virtue::Respect)]),
    ("You find a ¥1000 note by the shoe lockers. Nobody saw.", [("Hand it in at the staff room", Virtue::Honesty), ("Ask around who dropped it", Virtue::Responsibility), ("Put a notice on the board", Virtue::Fairness)]),
    ("Your friend copies your homework every morning.", [("Tell them honestly you'll stop", Virtue::Honesty), ("Offer to do it together after school", Virtue::Kindness), ("Let them, it's their choice", Virtue::Respect)]),
    ("An old man drops his shopping at the bus stop.", [("Help pick it up and say good morning", Virtue::Kindness), ("Help, then let him go first on the bus", Virtue::Respect), ("Ask if he needs help first", Virtue::Respect)]),
    ("You've a test tomorrow and a new game came out today.", [("Study first, play after", Virtue::Responsibility), ("Play one hour, then study", Virtue::Fairness), ("Ask a friend to study together", Virtue::Courage)]),
    ("A classmate is left out of every group in PE.", [("Ask them to join yours", Virtue::Kindness), ("Talk to them at lunch", Virtue::Kindness), ("Suggest the teacher mixes the groups", Virtue::Fairness)]),
    ("Others are laughing at a student's accent.", [("Say it isn't funny", Virtue::Courage), ("Talk to the student afterwards", Virtue::Kindness), ("Tell the homeroom teacher", Virtue::Responsibility)]),
    ("The team could win by bending a rule the referee won't see.", [("Play by the rules", Virtue::Fairness), ("Say so to the team", Virtue::Courage), ("Ask the referee about the rule", Virtue::Honesty)]),
    ("Nobody wants the job of cleaning the toilets this week.", [("Volunteer", Virtue::Responsibility), ("Suggest taking turns", Virtue::Fairness), ("Do it with a friend", Virtue::Kindness)]),
    ("Your uncle's tired after a long day in the shop.", [("Make tea and ask about his day", Virtue::Kindness), ("Help close up the shop", Virtue::Responsibility), ("Keep quiet so he can rest", Virtue::Respect)]),
    ("A new student from abroad eats different food at lunch.", [("Ask them about it", Virtue::Respect), ("Share some of yours", Virtue::Kindness), ("Stop others from staring", Virtue::Courage)]),
    ("The class's goldfish is ill and nobody's noticed.", [("Tell the class and look after it", Virtue::Responsibility), ("Find out how to help it", Virtue::Kindness), ("Ask the science teacher", Virtue::Respect)]),
    ("On a hike, your group wants to leave rubbish behind to save weight.", [("Carry it down yourself", Virtue::Responsibility), ("Say why it matters", Virtue::Courage), ("Share the load fairly", Virtue::Fairness)]),
    ("The lunch staff cook for 300 people every day.", [("Say thank you at the window", Virtue::Respect), ("Eat it all and not waste any", Virtue::Responsibility), ("Write them a class card", Virtue::Kindness)]),
    ("Looking back: something you did this term you're not proud of.", [("Apologise to the person", Virtue::Honesty), ("Decide to do better", Virtue::Responsibility), ("Talk it over with someone", Virtue::Courage)]),
    ("You broke the class's stapler by accident.", [("Own up to the teacher", Virtue::Honesty), ("Offer to replace it", Virtue::Responsibility), ("Tell the class what happened", Virtue::Honesty)]),
    ("The best player always gets picked first; others never play.", [("Suggest everyone plays equal time", Virtue::Fairness), ("Pick someone who never gets picked", Virtue::Kindness), ("Ask the class what's fair", Virtue::Fairness)]),
    ("Graffiti appeared on the school's back wall.", [("Help clean it off", Virtue::Responsibility), ("Report it", Virtue::Honesty), ("Suggest a mural for the wall instead", Virtue::Courage)]),
    ("The town festival needs young people to carry the mikoshi.", [("Join in", Virtue::Courage), ("Help the elders prepare", Virtue::Respect), ("Ask what the tradition means", Virtue::Respect)]),
    ("A pen pal abroad asks what Japan is like.", [("Write honestly, good and bad", Virtue::Honesty), ("Ask about their country too", Virtue::Respect), ("Send something from your town", Virtue::Kindness)]),
    ("A group chat is spreading a rumour about a classmate.", [("Don't pass it on", Virtue::Honesty), ("Say it should stop", Virtue::Courage), ("Check on the classmate", Virtue::Kindness)]),
    ("You practised for weeks and still came last.", [("Keep going: next time", Virtue::Courage), ("Ask the winner how they train", Virtue::Respect), ("Congratulate them", Virtue::Respect)]),
];

fn moral(key: &str, rng: &mut Rng) -> Task {
    let i = key.strip_prefix("dilemma:").and_then(|s| s.parse::<usize>().ok()).unwrap_or_else(|| rng.below(DILEMMAS.len())) % DILEMMAS.len();
    let (sit, opts) = &DILEMMAS[i];
    Task::Reflect { prompt: format!("{sit} What would you do?"), options: opts.iter().map(|(s, v)| (s.to_string(), *v)).collect() }
}

// ------------------------------------------------------------------------------------- integrated studies

fn integrated(key: &str, rng: &mut Rng) -> Task {
    match key {
        "inq_intro" => bank(rng, &[("Integrated study is about...", "finding your own questions and answering them", &["copying the textbook", "only reading", "tests every week"]), ("Who decides the questions?", "you and your group", &["only the teacher", "the internet", "nobody"])]),
        "inq_theme" => bank(rng, &[("A good research theme is...", "something you can really find out about here", &["something nobody can answer", "the whole history of the world", "what you already know"]), ("Narrow 'the town' down to...", "one thing: the river, the shops, the festival", &["the whole country", "the universe", "everything at once"])]),
        "inq_questions" => sort(rng, "Is this a good research question", ["good question", "too vague / yes-no"], &[("Why does the river flood in June?", false), ("Is the river nice?", true), ("How has the number of shops on the main street changed since 1990?", false), ("Do people like things?", true), ("What do the farms grow in winter, and why?", false), ("Is the shrine old?", true)]),
        "inq_sources" => bank(rng, &[("The best source for how the town looked in 1960", "old photos at the library or town hall", &["a random website", "your guess", "a comic"]), ("The best source for today's bus times", "the bus company's own timetable", &["a friend's memory", "an old map", "a novel"]), ("To check something from the internet...", "compare it with another reliable source", &["believe it", "ask nobody", "copy it out"]), ("For each source, write down...", "who made it and when", &["its colour", "nothing", "only the title"])]),
        "inq_interview" => {
            let b: &[(&'static str, &'static [&'static str])] = &[("An interview: in order", &["Ask for an appointment", "Prepare your questions", "Greet them and say why you've come", "Ask and take notes", "Thank them", "Send a thank-you letter"])];
            orders(rng, b)
        }
        "inq_field" => bank(rng, &[("On field work, write down...", "where, when and what you saw", &["only what you liked", "nothing: remember it", "the weather only"]), ("Before photographing a person...", "ask their permission", &["just shoot", "hide", "post it first"])]),
        "inq_organise" => bank(rng, &[("Marking your data on a map shows...", "where things are", &["how they feel", "the date", "nothing"]), ("Grouping cards of similar ideas helps you...", "see patterns", &["lose them", "forget them", "count them only"]), ("A summary should be...", "short, in your own words", &["copied from the source", "as long as possible", "a list of websites"])]),
        "inq_charts" => {
            let b: &[(&str, &str)] = &[("changes over the years", "a line graph"), ("parts of a whole", "a pie chart"), ("comparing amounts", "a bar graph"), ("where things are", "a map")];
            pairs(rng, b, |k| format!("To show {k}, use..."), false)
        }
        "inq_discuss" => bank(rng, &[("When someone in the group disagrees...", "ask for their reasons", &["ignore them", "vote at once", "leave"]), ("A good discussion ends with...", "a summary of what was decided", &["nothing", "an argument", "the bell"])]),
        "inq_reflect" => bank(rng, &[("Looking back means...", "what went well, and what to do better", &["only the marks", "blaming others", "forgetting it"]), ("A good next step is...", "specific: 'interview two more shops'", &["vague: 'do better'", "none", "the same again"])]),
        "inq_poster" => bank(rng, &[("A poster's title should be...", "short and big", &["long and small", "hidden", "in pencil"]), ("Data on a poster needs...", "its source", &["a joke", "nothing", "glitter"]), ("Leave space round things so the poster is...", "easy to read", &["empty", "boring", "cheaper"])]),
        "inq_present" => {
            let b: &[(&'static str, &'static [&'static str])] = &[("A presentation: in order", &["Greeting and title", "Why we chose it", "What we found", "What it means", "Questions", "Thank you"])];
            orders(rng, b)
        }
        "bosai" => bank(rng, &[("A hazard map shows...", "places at risk of floods, landslides and tsunami", &["the best shops", "bus stops", "the school's rules"]), ("In a tsunami warning, go...", "to high ground at once", &["to the beach to look", "home for your things", "under a desk"]), ("'Tendenko' means...", "everyone flees at once, on their own", &["wait for your family", "stay home", "film it"])]),
        "bosai_steps" => {
            let b: &[(&'static str, &'static [&'static str])] = &[("An earthquake in class: in order", &["Get under the desk, protect your head", "Wait till the shaking stops", "Listen to the teacher", "Walk out: don't push, run or talk", "Line up in the yard for the roll call"])];
            orders(rng, b)
        }
        "bosai_kit" => sort(rng, "The emergency kit", ["in the kit", "leave it out"], &[("water", false), ("a torch", false), ("a radio", false), ("a first-aid kit", false), ("spare batteries", false), ("canned food", false), ("a game console", true), ("a cushion", true), ("a big dictionary", true), ("a glass vase", true)]),
        "careers" => bank(rng, &[("Farming and fishing are...", "primary industry", &["secondary industry", "tertiary industry", "no industry"]), ("Making things in factories is...", "secondary industry", &["primary industry", "tertiary industry", "trade"]), ("Shops, hospitals and schools are...", "tertiary industry (services)", &["primary industry", "secondary industry", "mining"]), ("Work experience in the 2nd year is called...", "shokuba taiken", &["shūgaku ryokō", "bunkasai", "sotsugyōshiki"])]),
        "intl" => {
            if rng.chance(0.3) {
                num("How many Sustainable Development Goals (SDGs) did the UN set?", 17)
            } else {
                bank(rng, &[("A town twinned with one abroad", "a sister city", &["a capital", "a port", "a colony"]), ("Fair trade means...", "paying producers a fair price", &["the cheapest price", "free goods", "no trade"])])
            }
        }
        _ => integrated("inq_intro", rng),
    }
}

// ------------------------------------------------------------------------------------- class activities

fn class_activities(key: &str, rng: &mut Rng) -> Task {
    let r = |p: &str, o: [(&str, Virtue); 3]| Task::Reflect { prompt: p.into(), options: o.iter().map(|(s, v)| (s.to_string(), *v)).collect() };
    match key {
        "ca_intro" => r("Introduce yourself to 1-A: what do you lead with?", [("Something you like", Virtue::Courage), ("What you hope this year brings", Virtue::Responsibility), ("A question for the class", Virtue::Respect)]),
        "ca_election" => r("Who gets your vote?", [("Someone who listens to everyone", Virtue::Fairness), ("Someone who gets things done", Virtue::Responsibility), ("You: you put your own hand up", Virtue::Courage)]),
        "ca_committees" => r("Which job do you take?", [("Library committee", Virtue::Responsibility), ("Health committee", Virtue::Kindness), ("Whatever's left over", Virtue::Fairness)]),
        "ca_goal" => r("What should the class goal be?", [("Everyone says good morning", Virtue::Respect), ("No one left out", Virtue::Kindness), ("Do our best at the festivals", Virtue::Courage)]),
        "ca_rota" => r("How should the rota work?", [("Strict turns, the same for all", Virtue::Fairness), ("Swap if someone's busy", Virtue::Kindness), ("Pairs, to make it quicker", Virtue::Responsibility)]),
        "ca_plan" => r("What do you suggest for the class?", [("Something everyone can join", Virtue::Fairness), ("Something bold to stand out", Virtue::Courage), ("Ask the quiet ones first", Virtue::Respect)]),
        "ca_study" => {
            if rng.chance(0.5) {
                r("Your study plan for the tests:", [("A little every day", Virtue::Responsibility), ("Study with friends", Virtue::Kindness), ("Start with the hardest subject", Virtue::Courage)])
            } else {
                bank(rng, &[("A good study plan is...", "a little every day, with breaks", &["everything the night before", "only your best subject", "no plan at all"])])
            }
        }
        "ca_seats" => r("Changing seats: how?", [("Draw lots", Virtue::Fairness), ("Let people with poor eyesight sit at the front", Virtue::Kindness), ("Let the teacher decide", Virtue::Respect)]),
        "ca_game" => r("Which game for the class?", [("Fruits basket", Virtue::Kindness), ("Dodgeball", Virtue::Courage), ("A quiz", Virtue::Fairness)]),
        "ca_safety" => bank(rng, &[("Walking home after dark...", "stay on lit roads, with a friend", &["take shortcuts", "wear headphones", "go alone through the park"]), ("In the heat, carry...", "water and a hat", &["nothing", "a heavy bag", "a coat"]), ("On icy roads, walk...", "in small steps, hands out of pockets", &["fast", "on the edge", "looking at your phone"])]),
        "ca_reflect" => r("Looking back: what will you do next term?", [("Help others more", Virtue::Kindness), ("Speak up more", Virtue::Courage), ("Keep my promises", Virtue::Responsibility)]),
        _ => class_activities("ca_intro", rng),
    }
}
