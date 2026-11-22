//! What someone looks like: height and build for their age, skin, hair, glasses, and what they
//! wear (to school or work, and off duty). Everyone is Japanese: East Asian skin tones, black or
//! dark-brown hair (going grey from the fifties; some young people dye it brown).
//!
//! `tools/blender/make_people.py` builds each person's model from this.

use crate::{PlaceKind, Sex, Work};
use serde::{Deserialize, Serialize};
use tcg::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Build {
    Slim,
    Average,
    Sturdy,
    Heavy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HairStyle {
    Buzz,
    Short,
    Spiky,
    SidePart,
    Bowl,
    Long,
    Ponytail,
    TwinTails,
    Bob,
    Shoulder,
    Braid,
    Bun,
    ShortPerm,
    Receding,
    Bald,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Outfit {
    /// Junior high boys: the black stand-collar uniform (gakuran).
    Gakuran,
    /// Junior high girls: the sailor uniform.
    Sailor,
    /// Elementary kids and little ones: T-shirt and shorts or a skirt.
    KidsCasual,
    BabyWear,
    Casual,
    Suit,
    /// Blouse, cardigan and skirt (office, bank, post office counter).
    Office,
    /// Work coveralls (car repair, petrol, factory, construction).
    Coveralls,
    /// Long sleeves, arm covers, work trousers, rubber boots.
    FarmWear,
    /// Shirt and a shop apron.
    ShopApron,
    /// The konbini's striped shirt.
    Konbini,
    /// Tracksuit ("jersey"): PE teachers, elders on walks.
    Tracksuit,
    /// Shirt, tie or cardigan, slacks.
    Teacher,
    /// Elderly men: shirt, cardigan or vest, slacks.
    Cardigan,
    /// Elderly women: blouse, long skirt or slacks, apron.
    Grandma,
    /// The priest's work clothes (samue).
    Samue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Accessory {
    /// The elementary school bag.
    Randoseru,
    /// The yellow hat first-graders wear.
    YellowHat,
    StrawHat,
    SunVisor,
    Cap,
    Headband,
    Apron,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clothes {
    pub outfit: Outfit,
    /// Top, bottom, shoes, extra (tie, apron, collar...): sRGB.
    pub colors: [[u8; 3]; 4],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Look {
    pub height_cm: u16,
    pub build: Build,
    pub skin: [u8; 3],
    pub hair: HairStyle,
    pub hair_color: [u8; 3],
    pub eyes: [u8; 3],
    pub glasses: Option<[u8; 3]>,
    /// Stubble, a moustache.
    pub stubble: bool,
    pub mustache: bool,
    /// A bit bent with age.
    pub stoop: bool,
    /// For school or work.
    pub day: Clothes,
    /// Off duty, weekends.
    pub casual: Clothes,
    pub accessories: Vec<Accessory>,
}

/// Average height (cm) by age and sex: MEXT school health survey for children and teenagers,
/// the national health and nutrition survey for adults (people now in their seventies and
/// eighties grew up shorter).
pub fn mean_height(sex: Sex, age: u8) -> f32 {
    const BOY: [f32; 18] = [50.0, 75.0, 86.0, 95.0, 102.0, 109.0, 116.5, 122.6, 128.4, 133.9, 139.0, 145.2, 152.8, 160.0, 165.4, 168.3, 169.9, 170.6];
    const GIRL: [f32; 18] = [49.0, 74.0, 85.0, 94.0, 101.0, 108.0, 115.6, 121.8, 127.6, 133.6, 140.1, 146.6, 151.9, 154.8, 156.5, 157.2, 157.7, 157.9];
    let a = age as usize;
    if a < 18 {
        return if sex == Sex::M { BOY[a] } else { GIRL[a] };
    }
    let (m, f) = match age {
        18..=49 => (171.5, 158.3),
        50..=59 => (170.0, 157.3),
        60..=69 => (167.6, 154.3),
        70..=79 => (164.0, 150.6),
        _ => (160.5, 146.4),
    };
    if sex == Sex::M { m } else { f }
}

fn normal(rng: &mut Rng) -> f32 {
    // (sum of uniforms: near enough)
    (0..6).map(|_| rng.f32()).sum::<f32>() - 3.0
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8)
}

const NAVY: [u8; 3] = [38, 44, 72];
const BLACK: [u8; 3] = [28, 28, 32];
const WHITE: [u8; 3] = [240, 240, 236];

fn pick_col(rng: &mut Rng, cols: &[[u8; 3]]) -> [u8; 3] {
    *rng.pick(cols)
}

/// Casual clothes for someone's age and sex.
fn casual(rng: &mut Rng, sex: Sex, age: u8) -> Clothes {
    let tops_m: &[[u8; 3]] = &[[62, 92, 140], [200, 200, 196], [70, 110, 80], [180, 60, 50], [40, 40, 44], [150, 130, 100], [90, 90, 120], [220, 180, 90]];
    let tops_f: &[[u8; 3]] = &[[240, 236, 228], [230, 170, 180], [150, 180, 210], [200, 190, 150], [120, 90, 130], [90, 130, 110], [250, 220, 170], [60, 60, 70]];
    let bottoms: &[[u8; 3]] = &[[48, 56, 84], [60, 60, 64], [130, 120, 100], [40, 40, 44], [90, 100, 120]];
    let shoes: &[[u8; 3]] = &[[236, 236, 236], [40, 40, 40], [120, 80, 50], [80, 90, 120]];
    let outfit = if age < 3 { Outfit::BabyWear } else if age < 12 { Outfit::KidsCasual } else if age >= 70 { if sex == Sex::M { Outfit::Cardigan } else { Outfit::Grandma } } else { Outfit::Casual };
    let top = pick_col(rng, if sex == Sex::M { tops_m } else { tops_f });
    let mut bottom = pick_col(rng, bottoms);
    if sex == Sex::F && age >= 70 {
        bottom = pick_col(rng, &[[90, 90, 104], [110, 90, 80], [70, 70, 80]]);
    }
    Clothes { outfit, colors: [top, bottom, pick_col(rng, shoes), pick_col(rng, &[[220, 200, 160], [120, 140, 170], [180, 120, 120]])] }
}

impl Look {
    /// A stand-in until the real look is made (generation fills everyone's in at the end).
    pub(crate) fn blank() -> Self {
        let c = Clothes { outfit: Outfit::Casual, colors: [[0; 3]; 4] };
        Look { height_cm: 0, build: Build::Average, skin: [0; 3], hair: HairStyle::Short, hair_color: [0; 3], eyes: [0; 3], glasses: None, stubble: false, mustache: false, stoop: false, day: c.clone(), casual: c, accessories: Vec::new() }
    }
}

/// Make someone's look. `work` and `place` (their workplace's kind) decide the day clothes.
pub fn make(rng: &mut Rng, sex: Sex, age: u8, work: &Work, place: Option<PlaceKind>, outdoors: bool) -> Look {
    let m = sex == Sex::M;
    let sd = if age < 18 { 5.0 } else if m { 5.7 } else { 5.3 };
    let height_cm = (mean_height(sex, age) + normal(rng) * sd * 0.7).round().max(45.0) as u16;
    let r = rng.f32();
    let build = if age < 13 {
        if r < 0.25 { Build::Slim } else if r < 0.9 { Build::Average } else { Build::Sturdy }
    } else if r < 0.25 {
        Build::Slim
    } else if r < 0.7 {
        Build::Average
    } else if r < 0.9 {
        Build::Sturdy
    } else {
        Build::Heavy
    };
    // skin: light to tanned East Asian tones; people who work outside are browner
    let light = [246, 222, 204];
    let tan = [206, 160, 126];
    let t = (rng.f32() * 0.45 + if outdoors { 0.4 } else { 0.0 } + if age >= 60 { 0.1 } else { 0.0 }).min(1.0);
    let skin = mix(light, tan, t);
    // hair colour: black / dark brown; dyed brown for some young adults; grey from the fifties
    let mut hair_color = if rng.chance(0.7) { [26, 22, 22] } else { [52, 38, 32] };
    if (18..45).contains(&age) && rng.chance(if m { 0.12 } else { 0.3 }) {
        hair_color = [110, 72, 44];
    }
    if age >= 50 {
        let grey = ((age as f32 - 50.0) / 30.0 + normal(rng) * 0.12).clamp(0.0, 1.0);
        let g = if !m && age < 75 && rng.chance(0.5) { grey * 0.3 } else { grey }; // (many women dye it)
        hair_color = mix(hair_color, [212, 212, 214], g);
    }
    let hair = if m {
        match age {
            0..=2 => HairStyle::Short,
            3..=11 => *rng.pick(&[HairStyle::Short, HairStyle::Short, HairStyle::Bowl, HairStyle::Spiky, HairStyle::Buzz]),
            12..=17 => match work {
                Work::JuniorHigh { club: Some(crate::Club::Baseball), .. } => HairStyle::Buzz,
                _ => *rng.pick(&[HairStyle::Short, HairStyle::Spiky, HairStyle::Spiky, HairStyle::SidePart, HairStyle::Bowl]),
            },
            18..=39 => *rng.pick(&[HairStyle::Short, HairStyle::Spiky, HairStyle::SidePart, HairStyle::Short]),
            40..=59 => if rng.chance(0.25) { HairStyle::Receding } else { *rng.pick(&[HairStyle::Short, HairStyle::SidePart, HairStyle::Buzz]) },
            _ => if rng.chance(0.4) { HairStyle::Receding } else if rng.chance(0.25) { HairStyle::Bald } else { *rng.pick(&[HairStyle::Short, HairStyle::Buzz, HairStyle::SidePart]) },
        }
    } else {
        match age {
            0..=2 => HairStyle::Short,
            3..=11 => *rng.pick(&[HairStyle::Bob, HairStyle::Ponytail, HairStyle::TwinTails, HairStyle::Long, HairStyle::Bob]),
            12..=17 => *rng.pick(&[HairStyle::Long, HairStyle::Ponytail, HairStyle::Bob, HairStyle::Shoulder, HairStyle::Braid, HairStyle::Ponytail]),
            18..=39 => *rng.pick(&[HairStyle::Long, HairStyle::Shoulder, HairStyle::Bob, HairStyle::Ponytail]),
            40..=64 => *rng.pick(&[HairStyle::Shoulder, HairStyle::Bob, HairStyle::Bob, HairStyle::Bun, HairStyle::ShortPerm]),
            _ => *rng.pick(&[HairStyle::ShortPerm, HairStyle::ShortPerm, HairStyle::Bun, HairStyle::Bob]),
        }
    };
    let eyes = *rng.pick(&[[70, 46, 34], [92, 60, 40], [56, 40, 32], [110, 72, 46]]);
    let glasses_p = match age {
        0..=9 => 0.03,
        10..=17 => 0.22,
        18..=44 => 0.3,
        45..=64 => 0.45,
        _ => 0.55,
    };
    let glasses = rng.chance(glasses_p).then(|| *rng.pick(&[[30, 30, 34], [90, 60, 40], [150, 150, 160], [60, 50, 40]]));
    let stubble = m && age >= 25 && rng.chance(0.15);
    let mustache = m && age >= 55 && rng.chance(0.15);
    let stoop = age >= 75 && rng.chance(0.6);
    let casual_clothes = casual(rng, sex, age);
    let mut acc = Vec::new();
    let day = match work {
        Work::JuniorHigh { .. } | Work::Player => {
            if m {
                Clothes { outfit: Outfit::Gakuran, colors: [BLACK, BLACK, [236, 236, 236], [200, 170, 60]] }
            } else {
                Clothes { outfit: Outfit::Sailor, colors: [WHITE, NAVY, [236, 236, 236], [180, 40, 50]] }
            }
        }
        Work::Elementary { grade } => {
            acc.push(Accessory::Randoseru);
            if *grade == 1 {
                acc.push(Accessory::YellowHat);
            }
            casual_clothes.clone()
        }
        Work::Farmer => {
            // a straw hat for the old hands, otherwise a cap or a sun visor
            acc.push(if age >= 60 && rng.chance(0.5) { Accessory::StrawHat } else if m { Accessory::Cap } else { Accessory::SunVisor });
            Clothes { outfit: Outfit::FarmWear, colors: [pick_col(rng, &[[110, 120, 100], [150, 140, 120], [90, 100, 120], [200, 190, 170]]), [80, 80, 76], [60, 70, 60], [220, 220, 210]] }
        }
        Work::Shopkeeper | Work::Kenji => Clothes { outfit: Outfit::ShopApron, colors: [pick_col(rng, &[[72, 72, 78], [200, 200, 196], [120, 100, 80]]), [62, 62, 62], [40, 40, 40], pick_col(rng, &[[34, 46, 84], [120, 40, 40], [60, 80, 60]])] },
        Work::Commuter | Work::Local => match place {
            Some(PlaceKind::Konbini) => Clothes { outfit: Outfit::Konbini, colors: [[90, 150, 200], [40, 40, 44], [40, 40, 40], [240, 240, 240]] },
            Some(PlaceKind::Petrol) | Some(PlaceKind::CarRepair) => {
                acc.push(Accessory::Cap);
                Clothes { outfit: Outfit::Coveralls, colors: [pick_col(rng, &[[60, 80, 120], [90, 90, 90], [70, 90, 70]]), [60, 70, 90], [40, 40, 40], [200, 60, 50]] }
            }
            Some(PlaceKind::CoOp) => Clothes { outfit: Outfit::Coveralls, colors: [[70, 110, 90], [60, 60, 64], [40, 40, 40], [240, 240, 240]] },
            Some(PlaceKind::PostOffice) | Some(PlaceKind::TownOffice) | Some(PlaceKind::Drugstore) | Some(PlaceKind::Station) => {
                if m { Clothes { outfit: Outfit::Suit, colors: [[60, 64, 76], [60, 64, 76], [30, 30, 30], [150, 40, 50]] } } else { Clothes { outfit: Outfit::Office, colors: [[240, 240, 236], [60, 64, 80], [40, 40, 40], [150, 160, 190]] } }
            }
            _ => {
                // the commute: office in the city, or the factory / building site
                if rng.chance(0.55) {
                    if m { Clothes { outfit: Outfit::Suit, colors: [pick_col(rng, &[[40, 44, 56], [60, 64, 76], [70, 70, 70]]), [50, 54, 66], [30, 30, 30], pick_col(rng, &[[150, 40, 50], [40, 60, 120], [90, 90, 100]])] } } else { Clothes { outfit: Outfit::Office, colors: [[240, 240, 236], pick_col(rng, &[[60, 64, 80], [40, 40, 44], [120, 110, 100]]), [40, 40, 40], [200, 190, 170]] } }
                } else {
                    Clothes { outfit: Outfit::Coveralls, colors: [pick_col(rng, &[[60, 80, 120], [90, 90, 90], [150, 140, 110]]), [70, 74, 80], [40, 40, 40], [220, 200, 60]] }
                }
            }
        },
        Work::Teacher { role, .. } => {
            if role.contains("PE") {
                Clothes { outfit: Outfit::Tracksuit, colors: [pick_col(rng, &[[40, 60, 130], [40, 40, 44], [140, 40, 50]]), [40, 40, 50], [236, 236, 236], [240, 240, 240]] }
            } else {
                Clothes { outfit: Outfit::Teacher, colors: [[236, 236, 232], pick_col(rng, &[[60, 64, 76], [110, 100, 90], [70, 70, 74]]), [40, 40, 40], pick_col(rng, &[[150, 40, 50], [40, 60, 120], [160, 140, 110]])] }
            }
        }
        Work::Priest => Clothes { outfit: Outfit::Samue, colors: [[60, 64, 80], [60, 64, 80], [30, 30, 30], [60, 64, 80]] },
        Work::Homemaker => {
            if rng.chance(0.4) {
                acc.push(Accessory::Apron);
            }
            casual_clothes.clone()
        }
        Work::Retired => {
            if age >= 70 && rng.chance(0.3) {
                acc.push(if m { Accessory::Cap } else { Accessory::SunVisor });
            }
            if !m && rng.chance(0.4) {
                acc.push(Accessory::Apron);
            }
            casual_clothes.clone()
        }
        _ => casual_clothes.clone(),
    };
    Look { height_cm, build, skin, hair, hair_color, eyes, glasses, stubble, mustache, stoop, day, casual: casual_clothes, accessories: acc }
}
