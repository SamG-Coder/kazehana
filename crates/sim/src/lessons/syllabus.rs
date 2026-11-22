//! The year's syllabus for 1st-year junior high (MEXT course of study, 中学校学習指導要領), week by
//! week, for every subject on 1-A's timetable (docs/lessons_plan.md).
//!
//! The school year has three terms. Their weeks are counted from the week (Monday to Sunday) of
//! each term's first school day:
//! - 1st term: 6 April to 20 July 2026, 15 weeks with lessons;
//! - 2nd term: 1 September to 25 December, 17 weeks;
//! - 3rd term: 8 January to 24 March 2027, 12 weeks.
//!
//! Each week of each subject has a topic and a content key: the key picks the mini-game's
//! questions (`content::tasks`). "review" weeks (the test weeks) draw on the term's weeks so far.

use crate::calendar::{is_school_day, school_break, Date, START_DATE};
use crate::school::Subject;

/// The subjects in a fixed order (index into per-subject tables and records).
pub const SUBJECTS: [Subject; 12] = [
    Subject::Japanese,
    Subject::Maths,
    Subject::English,
    Subject::Science,
    Subject::Social,
    Subject::Pe,
    Subject::TechHome,
    Subject::Music,
    Subject::Art,
    Subject::Moral,
    Subject::Integrated,
    Subject::ClassActivities,
];

pub fn subject_index(s: Subject) -> usize {
    SUBJECTS.iter().position(|&x| x == s).unwrap()
}

/// How many weeks with lessons each term has (the 2026–27 school year).
pub const TERM_WEEKS: [u8; 3] = [15, 17, 12];
/// All the school weeks of the year.
pub const YEAR_WEEKS: usize = 15 + 17 + 12;

/// The school year a date belongs to (the April it began).
pub fn school_year(d: Date) -> i32 {
    if d.month >= 4 {
        d.year
    } else {
        d.year - 1
    }
}

/// Which term (1..=3) a month falls in (August: none).
pub fn term_of(d: Date) -> Option<u8> {
    match d.month {
        4..=7 => Some(1),
        9..=12 => Some(2),
        1..=3 => Some(3),
        _ => None,
    }
}

/// A term's first school day.
pub fn term_start(year: i32, term: u8) -> Date {
    let mut d = match term {
        1 => Date::new(year, 4, 1),
        2 => Date::new(year, 9, 1),
        _ => Date::new(year + 1, 1, 1),
    };
    if term == 1 && year == START_DATE.year {
        d = START_DATE;
    }
    while !is_school_day(d) {
        d = d.add_days(1);
    }
    d
}

fn monday(d: Date) -> Date {
    d.add_days(-(d.weekday().index() as i64))
}

/// (term, week of the term, 1-based) of a date in a term (not in the breaks).
pub fn term_week(d: Date) -> Option<(u8, u8)> {
    let t = term_of(d)?;
    if school_break(d).is_some() {
        return None;
    }
    let first = term_start(school_year(d), t);
    if d < monday(first) {
        return None;
    }
    let w = (monday(d).to_days() - monday(first).to_days()) / 7 + 1;
    let w = w.min(TERM_WEEKS[t as usize - 1] as i64);
    Some((t, w as u8))
}

/// The week of the year (1..=44) for a term and week.
pub fn year_week(term: u8, week: u8) -> usize {
    TERM_WEEKS[..term as usize - 1].iter().map(|&w| w as usize).sum::<usize>() + week as usize
}

/// (term, week) of a week of the year (1..=44).
pub fn from_year_week(n: usize) -> (u8, u8) {
    let mut n = n;
    for (t, &w) in TERM_WEEKS.iter().enumerate() {
        if n <= w as usize {
            return (t as u8 + 1, n as u8);
        }
        n -= w as usize;
    }
    (3, TERM_WEEKS[2])
}

/// A unit of a subject: weeks `from..=to` of a term, one (topic, content key) a week.
pub struct Unit {
    pub term: u8,
    pub from: u8,
    pub title: &'static str,
    pub weeks: &'static [(&'static str, &'static str)],
}

impl Unit {
    pub fn to(&self) -> u8 {
        self.from + self.weeks.len() as u8 - 1
    }
}

/// One week of a subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Week {
    pub subject: Subject,
    pub term: u8,
    pub week: u8,
    pub unit: &'static str,
    pub topic: &'static str,
    pub key: &'static str,
}

impl Week {
    pub fn year_week(&self) -> usize {
        year_week(self.term, self.week)
    }
    pub fn review(&self) -> bool {
        self.key == "review" || self.key == "year_review"
    }
}

/// A subject's week, if it has one then.
pub fn week_of(subject: Subject, term: u8, week: u8) -> Option<Week> {
    units(subject).iter().find(|u| u.term == term && (u.from..=u.to()).contains(&week)).map(|u| {
        let (topic, key) = u.weeks[(week - u.from) as usize];
        Week { subject, term, week, unit: u.title, topic, key }
    })
}

/// What a subject is doing on a date.
pub fn week_on(subject: Subject, d: Date) -> Option<Week> {
    let (t, w) = term_week(d)?;
    week_of(subject, t, w)
}

/// Every week of a subject in the year, in order.
pub fn year_of(subject: Subject) -> Vec<Week> {
    (1..=YEAR_WEEKS).filter_map(|n| {
        let (t, w) = from_year_week(n);
        week_of(subject, t, w)
    }).collect()
}

const R: (&str, &str) = ("Review for the tests", "review");

/// A subject's units for the year.
pub fn units(subject: Subject) -> &'static [Unit] {
    match subject {
        Subject::Maths => MATHS,
        Subject::Japanese => JAPANESE,
        Subject::English => ENGLISH,
        Subject::Science => SCIENCE,
        Subject::Social => SOCIAL,
        Subject::Pe => PE,
        Subject::Music => MUSIC,
        Subject::Art => ART,
        Subject::TechHome => TECHHOME,
        Subject::Moral => MORAL,
        Subject::Integrated => INTEGRATED,
        Subject::ClassActivities => CLASS,
    }
}

/// Kokugo's units and topics as they're written on its pages (in Japanese, vertically): one entry
/// per unit of `JAPANESE`, in order, a topic per week.
pub static JAPANESE_JA: &[(&str, &[&str])] = &[
    ("中学生になって　ことばと声", &["詩を読む　リズムとイメージ", "漢字　一学期の漢字", "話す　自己紹介", "小説　場面と心情", "文法　言葉の単位と文節", "漢字　部首", "テストに向けて復習"]),
    ("読んで理解する", &["説明文　事実と意見", "漢字　筆順と画数", "書く　レポート", "文法　主語と述語", "俳句と季語", "漢字　音読みと訓読み", "テストに向けて復習", "夏の読書"]),
    ("ことばで考える", &["漢字　二学期の漢字", "段落と文章の構成", "文法　品詞", "話し合う　賛成と反対", "小説　人物と視点", "漢字　熟語の構成", "ことわざと慣用句", "テストに向けて復習", "意見文を書く", "漢字　同音異義語", "文法　修飾語と接続語"]),
    ("古典　竹取物語", &["歴史的仮名遣い", "竹取物語　冒頭", "テストに向けて復習", "竹取物語　かぐや姫", "四字熟語", "年賀状と手紙"]),
    ("漢文と一年のまとめ", &["漢字　三学期の漢字", "漢文　返り点", "漢文　論語", "小説　主題と読み手", "文法　品詞のまとめ", "作文　一年をふり返って", "漢字　一年の読み", "テストに向けて復習", "話す　発表", "自由詩", "漢字　一年のまとめ", "一年のまとめ"]),
];

impl Week {
    /// (unit, topic) as written on the subject's own pages: Japanese for kokugo, else as in the
    /// syllabus.
    pub fn titles_local(&self) -> (&'static str, &'static str) {
        if self.subject == Subject::Japanese {
            if let Some(i) = JAPANESE.iter().position(|u| u.term == self.term && (u.from..=u.to()).contains(&self.week)) {
                let (unit, topics) = JAPANESE_JA[i];
                if let Some(t) = topics.get((self.week - JAPANESE[i].from) as usize) {
                    return (unit, t);
                }
            }
        }
        (self.unit, self.topic)
    }
}

// ------------------------------------------------------------------------------------- maths

pub static MATHS: &[Unit] = &[
    Unit { term: 1, from: 1, title: "1. Positive and negative numbers", weeks: &[
        ("Numbers below zero and the number line", "signed"),
        ("Absolute value; comparing numbers", "abs"),
        ("Adding and subtracting", "addsub"),
        ("Multiplying and dividing", "muldiv"),
        ("Powers and the order of operations", "powers"),
        ("Prime numbers and prime factorisation", "primes"),
        R,
    ] },
    Unit { term: 1, from: 8, title: "2. Letters and expressions", weeks: &[
        ("Using letters for numbers", "letters"),
        ("Substituting values", "substitute"),
        ("Like terms: simplifying", "like_terms"),
        ("Multiplying out brackets", "expand"),
        ("Equalities and inequalities from situations", "relations"),
        ("Review: letters and expressions", "letters"),
        R,
    ] },
    Unit { term: 1, from: 15, title: "3. Linear equations", weeks: &[("What an equation is", "eq_basic")] },
    Unit { term: 2, from: 1, title: "3. Linear equations", weeks: &[
        ("Solving by balancing", "eq_basic"),
        ("Unknowns on both sides", "eq_both"),
        ("Equations with brackets", "eq_brackets"),
        ("Fractions and decimals in equations", "eq_fractions"),
        ("Word problems", "eq_word"),
        ("Proportion equations a : b = c : d", "ratio"),
    ] },
    Unit { term: 2, from: 7, title: "4. Proportion and inverse proportion", weeks: &[
        ("Functions; direct proportion", "proportion"),
        R,
        ("Coordinates", "coordinates"),
        ("Graphs of y = ax", "graph_direct"),
        ("Inverse proportion", "inverse"),
        ("Graphs of y = a/x and using proportion", "inverse_graph"),
    ] },
    Unit { term: 2, from: 13, title: "5. Plane figures", weeks: &[
        ("Lines, segments, rays and angles", "angles"),
        R,
        ("Translation, reflection, rotation", "transform"),
        ("Constructions: bisectors and perpendiculars", "constructions"),
        ("Circles and tangents", "circles"),
    ] },
    Unit { term: 3, from: 1, title: "5. Plane figures", weeks: &[("Sectors: arc length and area", "sector")] },
    Unit { term: 3, from: 2, title: "6. Solid figures", weeks: &[
        ("Polyhedra, prisms and pyramids", "solids"),
        ("Nets; lines and planes in space", "nets"),
        ("Surface area", "surface"),
        ("Volume of prisms and cylinders", "volume"),
        ("Volume of pyramids, cones and spheres", "volume2"),
    ] },
    Unit { term: 3, from: 7, title: "7. Data and probability", weeks: &[
        ("Frequency tables and histograms", "frequency"),
        R,
        ("Mean, median, mode and range", "averages"),
        ("Relative frequency", "relfreq"),
        ("Probability from many trials", "probability"),
        ("The year in review", "year_review"),
    ] },
];

// ------------------------------------------------------------------------------------- Japanese

pub static JAPANESE: &[Unit] = &[
    Unit { term: 1, from: 1, title: "Starting junior high: words and voices", weeks: &[
        ("A poem: rhythm and imagery", "poem"),
        ("Kanji: the year's first list", "kanji1"),
        ("Speaking: introducing yourself", "speech"),
        ("A story: scenes and feelings", "story"),
        ("Grammar: words, sentences, bunsetsu", "bunsetsu"),
        ("Kanji: radicals (bushu)", "radicals"),
        R,
    ] },
    Unit { term: 1, from: 8, title: "Reading to understand", weeks: &[
        ("Explanatory writing: fact and opinion", "fact_opinion"),
        ("Kanji: strokes and stroke counts", "strokes"),
        ("Writing a report", "report"),
        ("Grammar: subject and predicate", "subject_pred"),
        ("Haiku and season words (kigo)", "haiku"),
        ("Kanji: on and kun readings", "onkun"),
        R,
        ("Reading for the summer", "kanji1"),
    ] },
    Unit { term: 2, from: 1, title: "Thinking in words", weeks: &[
        ("Kanji: the second list", "kanji2"),
        ("Paragraphs and structure", "paragraphs"),
        ("Grammar: parts of speech", "pos"),
        ("Discussion: agreeing and disagreeing", "discussion"),
        ("A story: characters and viewpoint", "story"),
        ("Kanji: how compounds are built", "jukugo"),
        ("Proverbs and idioms (kotowaza)", "kotowaza"),
        R,
        ("Writing an opinion", "opinion"),
        ("Kanji: same sound, different kanji", "homophones"),
        ("Grammar: modifiers and conjunctions", "pos"),
    ] },
    Unit { term: 2, from: 12, title: "The classics: Taketori Monogatari", weeks: &[
        ("Old kana spelling (rekishiteki kanazukai)", "kana_old"),
        ("Taketori Monogatari: the opening", "taketori"),
        R,
        ("Taketori Monogatari: Kaguya-hime", "taketori"),
        ("Four-character idioms (yoji jukugo)", "yoji"),
        ("New Year cards and letters", "letters_jp"),
    ] },
    Unit { term: 3, from: 1, title: "Chinese classics and the year's words", weeks: &[
        ("Kanji: the third list", "kanji3"),
        ("Kanbun: reading marks (kaeriten)", "kanbun"),
        ("Kanbun: sayings from the Analects", "kanbun"),
        ("A story: theme and the reader", "story"),
        ("Grammar: parts of speech review", "pos"),
        ("Writing: an essay on the year", "report"),
        ("Kanji: readings of the year", "kanji3"),
        R,
        ("Speaking: a presentation", "speech"),
        ("Free verse", "poem"),
        ("Kanji: all the year's lists", "kanji_all"),
        ("The year in review", "year_review"),
    ] },
];

// ------------------------------------------------------------------------------------- English

pub static ENGLISH: &[Unit] = &[
    Unit { term: 1, from: 1, title: "Unit 0: Hello, English", weeks: &[
        ("The alphabet: capitals and small letters", "alphabet"),
        ("Phonics: letters and sounds", "phonics"),
        ("Classroom English and greetings", "greetings"),
    ] },
    Unit { term: 1, from: 4, title: "Unit 1: I am ... / You are ...", weeks: &[
        ("I am ... / You are ...", "be_i_you"),
        ("Are you ...? Yes, I am. / No, I'm not.", "be_q"),
        ("Numbers 1 to 100", "numbers"),
        R,
    ] },
    Unit { term: 1, from: 8, title: "Unit 2: This is ... / I like ...", weeks: &[
        ("This is ... / That is ...", "this_that"),
        ("He is ... / She is ...", "be_he_she"),
        ("I like ... / I play ... (general verbs)", "verbs"),
        ("Do you ...? / I don't ...", "do_q"),
        ("Plurals: two cats, three boxes", "plurals"),
        ("What do you ...?", "what_q"),
        R,
        ("Days, months and seasons", "calendar_en"),
    ] },
    Unit { term: 2, from: 1, title: "Unit 3: I can ...", weeks: &[
        ("Can: what you're able to do", "can"),
        ("Can you ...? / Can I ...?", "can_q"),
        ("Instructions: Open ..., Don't ...", "imperatives"),
    ] },
    Unit { term: 2, from: 4, title: "Unit 4: My friend from overseas", weeks: &[
        ("He plays ... (third person -s)", "third"),
        ("Does he ...? / He doesn't ...", "does_q"),
        ("I, my, me, mine", "pronouns"),
        ("Who, whose, which", "wh"),
        R,
        ("Where, when, how", "wh"),
        ("Telling the time", "time_en"),
    ] },
    Unit { term: 2, from: 11, title: "Unit 5: What are you doing?", weeks: &[
        ("I am reading (present progressive)", "progressive"),
        ("What are you doing?", "progressive_q"),
        ("Spelling the -ing forms", "ing_spell"),
        R,
        ("I want to ... / I like ...ing", "want_to"),
        ("A letter to a friend overseas", "letter_en"),
        ("Christmas and New Year", "season_en"),
    ] },
    Unit { term: 3, from: 1, title: "Unit 6: Last weekend", weeks: &[
        ("Past tense: regular verbs (-ed)", "past_reg"),
        ("Past tense: irregular verbs", "past_irr"),
        ("Did you ...? / I didn't ...", "past_q"),
        ("Was and were", "past_be"),
        ("I was reading (past progressive)", "past_prog"),
        ("Reading: a diary", "diary_en"),
        ("Spelling the -ed forms", "ed_spell"),
        R,
    ] },
    Unit { term: 3, from: 9, title: "Unit 7: Around town", weeks: &[
        ("Showing the way: Go straight ...", "directions"),
        ("Writing about the year", "write_year"),
        ("The year's words", "vocab_all"),
        ("The year in review", "year_review"),
    ] },
];

// ------------------------------------------------------------------------------------- science

pub static SCIENCE: &[Unit] = &[
    Unit { term: 1, from: 1, title: "1. Living things", weeks: &[
        ("Observing living things around school", "observe"),
        ("The loupe and the microscope", "microscope"),
        ("Flowers and their parts", "flower_parts"),
        ("Seed plants: angiosperms and gymnosperms", "seed_plants"),
        ("Leaves, stems and roots: monocots and dicots", "monocot_dicot"),
        ("Plants without seeds: ferns and mosses", "spore_plants"),
        R,
        ("Classifying plants", "plant_classify"),
        ("Vertebrates: five groups", "vertebrates"),
        ("Invertebrates", "invertebrates"),
        ("Classifying animals", "animal_classify"),
    ] },
    Unit { term: 1, from: 12, title: "2. Matter around us", weeks: &[
        ("Lab safety and the gas burner", "burner"),
        ("Metals and non-metals", "metals"),
        R,
        ("Organic and inorganic matter", "organic"),
    ] },
    Unit { term: 2, from: 1, title: "2. Matter around us", weeks: &[
        ("Density", "density"),
        ("Gases: oxygen and carbon dioxide", "gases"),
        ("Gases: hydrogen and ammonia", "gases"),
        ("Solutions and concentration", "solutions"),
        ("Solubility and crystals", "solubility"),
        ("States of matter", "states"),
        ("Melting and boiling points; distillation", "boiling"),
        R,
    ] },
    Unit { term: 2, from: 9, title: "3. Light, sound and force", weeks: &[
        ("Light: reflection", "reflection"),
        ("Light: refraction", "refraction"),
        ("Convex lenses and images", "lenses"),
        ("Sound: vibration and its speed", "sound"),
        ("Sound: pitch and loudness", "pitch"),
        R,
        ("Forces and what they do", "forces"),
        ("Springs: Hooke's law", "hooke"),
        ("Weight, mass and balanced forces", "weight"),
    ] },
    Unit { term: 3, from: 1, title: "4. The changing earth", weeks: &[
        ("Volcanoes and magma", "volcanoes"),
        ("Minerals and igneous rocks", "igneous"),
        ("Earthquakes: P and S waves", "quakes"),
        ("Seismic intensity, magnitude, plates", "plates"),
        ("Strata and sedimentary rocks", "strata"),
        ("Fossils: index and facies fossils", "fossils"),
        ("The land: disasters and blessings", "land"),
        R,
    ] },
    Unit { term: 3, from: 9, title: "3. Force: pressure", weeks: &[
        ("Pressure", "pressure"),
        ("Water pressure and air pressure", "pressure"),
        ("The year's experiments", "review"),
        ("The year in review", "year_review"),
    ] },
];

// ------------------------------------------------------------------------------------- social

pub static SOCIAL: &[Unit] = &[
    Unit { term: 1, from: 1, title: "Geography: the world", weeks: &[
        ("The globe and the world map", "world_map"),
        ("Continents and oceans", "continents"),
        ("Latitude and longitude", "latlong"),
        ("Japan's place and its time", "timezones"),
        ("Japan's territory and the 47 prefectures", "prefectures"),
        ("Climates of the world", "climates"),
        R,
    ] },
    Unit { term: 1, from: 8, title: "Geography: the regions of the world", weeks: &[
        ("Asia", "asia"),
        ("Europe", "europe"),
        ("Africa", "africa"),
        ("North America", "n_america"),
        ("South America", "s_america"),
        ("Oceania", "oceania"),
        R,
        ("Researching a region", "regions_mixed"),
    ] },
    Unit { term: 2, from: 1, title: "History: ancient times", weeks: &[
        ("Eras and centuries", "centuries"),
        ("The first people; Old and New Stone Age", "stone_age"),
        ("The ancient civilisations", "civilisations"),
        ("Greece, Rome and the world religions", "religions"),
        ("The Jōmon period", "jomon"),
        ("The Yayoi period and Himiko", "yayoi"),
        ("Kofun and the Yamato kingship", "kofun"),
        R,
        ("Asuka: Prince Shōtoku", "asuka"),
        ("The Taika reforms and the ritsuryō state", "taika"),
        ("Nara: Heijō-kyō", "nara"),
        ("Nara: Tōdai-ji and the Man'yōshū", "nara_culture"),
        ("Heian-kyō and the Fujiwara", "heian"),
        R,
        ("Kana and court literature", "heian_culture"),
        ("The timeline so far", "timeline"),
        ("Reading an old map", "timeline"),
    ] },
    Unit { term: 3, from: 1, title: "History: the samurai", weeks: &[
        ("The rise of the samurai", "samurai"),
        ("Taira and Minamoto", "samurai"),
        ("The Kamakura shogunate", "kamakura"),
        ("Kamakura culture and new Buddhism", "kamakura"),
        ("The Mongol invasions", "mongol"),
    ] },
    Unit { term: 3, from: 6, title: "Geography: Japan", weeks: &[
        ("The seven regions of Japan", "japan_regions"),
        ("Mountains, rivers and plains", "japan_land"),
        R,
        ("Japan's climates", "japan_climate"),
        ("Natural disasters and preparing for them", "disasters"),
        ("The timeline of the year", "timeline"),
        ("The year in review", "year_review"),
    ] },
];

// ------------------------------------------------------------------------------------- PE

pub static PE: &[Unit] = &[
    Unit { term: 1, from: 1, title: "Spring: fitness and track", weeks: &[
        ("Warm-ups and the rules of PE", "warmup"),
        ("Fitness test: grip, sit-ups, sit-and-reach", "fitness"),
        ("Fitness test: 50 m and the shuttle run", "sprint"),
        ("Track: sprint starts", "sprint"),
        ("Track: the relay baton", "relay"),
        ("Long jump", "jump"),
        ("Health: growing up", "health"),
        ("Ball games: soccer", "soccer"),
        ("Ball games: soccer matches", "soccer"),
        ("Rainy season in the gym: badminton", "badminton"),
    ] },
    Unit { term: 1, from: 11, title: "Summer: swimming", weeks: &[
        ("Swimming: freestyle", "swim"),
        ("Swimming: breaststroke", "swim"),
        ("Swimming: a timed 50 m", "swim"),
        ("Health: sleep and rest", "health"),
        ("Swimming: survival floating", "swim"),
    ] },
    Unit { term: 2, from: 1, title: "Autumn: the sports festival and games", weeks: &[
        ("Sports festival: the class relay", "relay"),
        ("Sports festival: tug of war", "tug"),
        ("Sports festival: the big jump rope", "jumprope"),
        ("Sports festival: marching in", "march"),
        ("Ball games: basketball", "basketball"),
        ("Ball games: basketball matches", "basketball"),
        ("Dance: rhythm dance", "dance"),
        ("Health: injuries and first aid", "health"),
        ("Mat gymnastics", "mat"),
        ("Mat gymnastics: rolls", "mat"),
        ("Long-distance running", "distance"),
        ("Long-distance running: pacing", "distance"),
        ("Kendo: bowing and footwork", "kendo"),
        ("Health: mind and stress", "health"),
        ("Kendo: strikes", "kendo"),
        ("The school marathon", "distance"),
        ("Jump rope (nawatobi)", "jumprope"),
    ] },
    Unit { term: 3, from: 1, title: "Winter: kendo, volleyball, running", weeks: &[
        ("Jump rope", "jumprope"),
        ("Kendo: men, kote, dō", "kendo"),
        ("Kendo: a match", "kendo"),
        ("Volleyball: passing", "volleyball"),
        ("Volleyball: serving", "volleyball"),
        ("Volleyball: a match", "volleyball"),
        ("Health: the environment and health", "health"),
        ("Long-distance: the marathon", "distance"),
        ("Softball throw", "throw"),
        ("Hurdles", "hurdles"),
        ("Dodgeball with the class", "dodgeball"),
        ("The class's choice", "soccer"),
    ] },
];

// ------------------------------------------------------------------------------------- music

pub static MUSIC: &[Unit] = &[
    Unit { term: 1, from: 1, title: "Singing and the recorder", weeks: &[
        ("The school song", "school_song"),
        ("Singing: breath and posture", "singing"),
        ("Notes and rests", "notation"),
        ("The alto recorder: first notes", "recorder"),
        ("Recorder: a simple tune", "recorder"),
        ("Listening: Vivaldi, 'Spring'", "vivaldi"),
        ("Tempo and dynamics", "terms"),
        ("Japanese songs: 'Sakura Sakura'", "sakura"),
        ("Singing in parts", "parts"),
        ("Recorder: legato and tonguing", "recorder"),
        ("Listening: 'Spring' and its sonnet", "vivaldi"),
        ("Rhythm ensemble", "rhythm"),
        ("Summer songs", "singing"),
        R,
        ("Singing for fun", "singing"),
    ] },
    Unit { term: 2, from: 1, title: "The chorus contest and listening", weeks: &[
        ("Chorus contest: choosing the song", "parts"),
        ("Chorus: soprano, alto and the men's part", "parts"),
        ("Chorus: blending voices", "singing"),
        ("Chorus: dynamics", "terms"),
        ("Chorus: the whole piece", "parts"),
        ("Chorus contest rehearsal", "parts"),
        ("Listening: Schubert, 'Erlkönig'", "erlkonig"),
        ("'Erlkönig': four voices in one", "erlkonig"),
        ("Japanese instruments: the koto", "koto"),
        ("'Rokudan no Shirabe'", "koto"),
        ("Time signatures", "notation"),
        ("Recorder: a duet", "recorder"),
        ("Rhythm ensemble", "rhythm"),
        R,
        ("Winter songs", "singing"),
        ("Recorder: a performance", "recorder"),
        ("The instruments of the orchestra", "instruments"),
    ] },
    Unit { term: 3, from: 1, title: "Making music", weeks: &[
        ("The orchestra's families", "instruments"),
        ("Making a melody", "compose"),
        ("Making a melody: phrases", "compose"),
        ("Recorder: the year's tunes", "recorder"),
        ("Listening: gagaku", "gagaku"),
        ("Folk songs of Japan", "folk"),
        ("Songs for the 3rd years' graduation", "singing"),
        R,
        ("Graduation ceremony songs", "singing"),
        ("Rhythm ensemble", "rhythm"),
        ("The school song again", "school_song"),
        ("The year's songs", "singing"),
    ] },
];

// ------------------------------------------------------------------------------------- art

pub static ART: &[Unit] = &[
    Unit { term: 1, from: 1, title: "Seeing, drawing and colour", weeks: &[
        ("What is art? Looking around", "appreciation"),
        ("Pencils: line and shading", "sketch"),
        ("Drawing your own hand", "sketch"),
        ("The colour wheel", "hue"),
        ("Primary and secondary colours", "hue"),
        ("Complementary colours", "complement"),
        ("Tints and shades", "hue"),
        ("Design: lettering", "lettering"),
        ("Design: a poster", "lettering"),
        ("Pattern and repetition", "pattern"),
        ("Still life", "sketch"),
        ("Watercolour: wet on wet", "hue"),
        ("A landscape from the window", "sketch"),
        ("Appreciation: ukiyo-e", "ukiyoe"),
        ("A sketchbook for the summer", "sketch"),
    ] },
    Unit { term: 2, from: 1, title: "Prints, banners and clay", weeks: &[
        ("Woodblock prints: the design", "print"),
        ("Woodblock prints: carving", "print"),
        ("Woodblock prints: printing", "print"),
        ("Culture festival: the class banner", "lettering"),
        ("The banner: a colour plan", "complement"),
        ("The banner: painting", "hue"),
        ("Appreciation: the Impressionists", "impressionism"),
        ("Clay: a pinch pot", "clay"),
        ("Clay: modelling a face", "clay"),
        ("Clay: glazing", "clay"),
        ("Self-portrait", "sketch"),
        ("Self-portrait: colour", "hue"),
        ("One-point perspective", "perspective"),
        ("Appreciation: Japanese painting", "appreciation"),
        ("New Year card design", "lettering"),
        ("Collage", "pattern"),
        ("Display and reflection", "appreciation"),
    ] },
    Unit { term: 3, from: 1, title: "Design and making", weeks: &[
        ("Illustration: a story in one picture", "sketch"),
        ("Colours and moods", "complement"),
        ("Paper craft: planning", "papercraft"),
        ("Paper craft: assembling", "papercraft"),
        ("Appreciation: modern art", "appreciation"),
        ("Design: a logo", "lettering"),
        ("Light and shadow", "perspective"),
        ("Mixing greys and browns", "hue"),
        ("Decorations for the graduation", "papercraft"),
        ("Portfolio", "appreciation"),
        ("Free work", "hue"),
        ("The year's work", "appreciation"),
    ] },
];

// ------------------------------------------------------------------------------------- technology and home economics

pub static TECHHOME: &[Unit] = &[
    Unit { term: 1, from: 1, title: "Technology: materials and making", weeks: &[
        ("Technology around us", "tech_intro"),
        ("Materials: wood, metal, plastic", "materials"),
        ("Wood: grain and strength", "wood"),
        ("Designing a shelf: the drawings", "drawing"),
        ("Marking out", "box_steps"),
        ("Saws: crosscut and rip", "saws"),
        ("Planes and files", "tools"),
        ("Joining: nails and glue", "box_steps"),
        ("Finishing", "box_steps"),
        ("Safety in the workshop", "safety"),
        ("Metal: bending sheet", "metal"),
        ("Plastics", "materials"),
        ("Judging what you made", "box_steps"),
        R,
        ("Technology and the environment", "materials"),
    ] },
    Unit { term: 2, from: 1, title: "Home economics: food and clothes", weeks: &[
        ("Food and nutrients", "nutrients"),
        ("The six food groups", "food_groups"),
        ("A day's meals", "food_groups"),
        ("Fresh food and labels", "labels"),
        ("Cooking: knives and safety", "kitchen_safety"),
        ("Cooking: miso soup", "cooking"),
        ("Cooking: hamburg steak", "cooking"),
        ("Local food and the seasons", "local_food"),
        ("Clothes: fibres and care labels", "care_labels"),
        ("Washing clothes", "washing"),
        ("Sewing: hand stitches", "stitches"),
        ("Sewing: a button", "button"),
        ("Sewing: a small bag", "stitches"),
        R,
        ("A safe home", "home_safety"),
        ("The family and the neighbourhood", "home_safety"),
        ("Planning a winter meal", "food_groups"),
    ] },
    Unit { term: 3, from: 1, title: "Technology: energy and information", weeks: &[
        ("How computers work", "computers"),
        ("Networks and staying safe online", "internet"),
        ("Programming: sequences", "program"),
        ("Programming: loops and branches", "program"),
        ("Measuring and control", "program"),
        ("Electric circuits", "circuits"),
        ("Energy conversion", "energy"),
        R,
        ("Growing plants", "cultivation"),
        ("Consumers: buying wisely", "consumer"),
        ("Consumers: contracts and money", "consumer"),
        ("The year's projects", "year_review"),
    ] },
];

// ------------------------------------------------------------------------------------- moral education

/// Moral education's weekly themes (MEXT's four areas: oneself, others, the group and society,
/// life and nature). The dilemmas themselves are `content::DILEMMAS`, one per theme.
pub static MORAL: &[Unit] = &[
    Unit { term: 1, from: 1, title: "Oneself and others", weeks: &[
        ("A new start: what kind of person?", "dilemma:0"),
        ("Honesty", "dilemma:1"),
        ("Friendship", "dilemma:2"),
        ("Courtesy", "dilemma:3"),
        ("Self-control", "dilemma:4"),
        ("Kindness", "dilemma:5"),
        ("Courage", "dilemma:6"),
        ("Rules and fairness", "dilemma:7"),
        ("Working for others", "dilemma:8"),
        ("Family", "dilemma:9"),
        ("Respecting differences", "dilemma:10"),
        ("The value of life", "dilemma:11"),
        ("Nature", "dilemma:12"),
        ("Gratitude", "dilemma:13"),
        ("Looking back on the term", "dilemma:14"),
    ] },
    Unit { term: 2, from: 1, title: "The group and society", weeks: &[
        ("Responsibility", "dilemma:15"),
        ("Fairness", "dilemma:16"),
        ("Our school", "dilemma:17"),
        ("Our town and its traditions", "dilemma:18"),
        ("People overseas", "dilemma:19"),
        ("Honesty online", "dilemma:20"),
        ("Effort and failure", "dilemma:21"),
        ("Friendship", "dilemma:2"),
        ("Kindness to strangers", "dilemma:5"),
        ("Rules and freedom", "dilemma:7"),
        ("Courage to speak up", "dilemma:6"),
        ("Working together", "dilemma:8"),
        ("Respecting differences", "dilemma:10"),
        ("The value of life", "dilemma:11"),
        ("Family", "dilemma:9"),
        ("Gratitude", "dilemma:13"),
        ("Looking back on the term", "dilemma:14"),
    ] },
    Unit { term: 3, from: 1, title: "Life and the future", weeks: &[
        ("New Year: aims", "dilemma:0"),
        ("Self-control", "dilemma:4"),
        ("Honesty", "dilemma:1"),
        ("Nature and beauty", "dilemma:12"),
        ("Responsibility", "dilemma:15"),
        ("Effort and failure", "dilemma:21"),
        ("Fairness", "dilemma:16"),
        ("Our town and its traditions", "dilemma:18"),
        ("People overseas", "dilemma:19"),
        ("Our school", "dilemma:17"),
        ("Gratitude to the 3rd years", "dilemma:13"),
        ("Looking back on the year", "dilemma:14"),
    ] },
];

// ------------------------------------------------------------------------------------- integrated studies

pub static INTEGRATED: &[Unit] = &[
    Unit { term: 1, from: 1, title: "Finding out about our town", weeks: &[
        ("What integrated study is", "inq_intro"),
        ("Choosing a theme about the town", "inq_theme"),
        ("Asking good questions", "inq_questions"),
        ("Where to find out: sources", "inq_sources"),
        ("Using the library", "inq_sources"),
        ("Planning an interview", "inq_interview"),
        ("Interview manners", "inq_interview"),
        ("Field work: walking the town", "inq_field"),
        ("Field work: notes and photos", "inq_field"),
        ("Sorting what we found", "inq_organise"),
        ("Charts and maps", "inq_charts"),
        ("Group discussion", "inq_discuss"),
        ("Halfway: looking back", "inq_reflect"),
        ("A plan for summer research", "inq_questions"),
        ("A plan for summer research", "inq_sources"),
    ] },
    Unit { term: 2, from: 1, title: "Telling people: the culture festival, safety, work", weeks: &[
        ("Sharing the summer research", "inq_organise"),
        ("Designing a poster", "inq_poster"),
        ("Writing the poster", "inq_poster"),
        ("Presenting well", "inq_present"),
        ("Rehearsal", "inq_present"),
        ("Culture festival preparation", "inq_poster"),
        ("Culture festival preparation", "inq_present"),
        ("After the festival: feedback", "inq_reflect"),
        ("Disaster prevention: hazard maps", "bosai"),
        ("Disaster prevention: evacuating", "bosai_steps"),
        ("Disaster prevention: an emergency kit", "bosai_kit"),
        ("People who work in the town", "inq_interview"),
        ("Jobs: what people do", "careers"),
        ("Jobs: interviewing a worker", "inq_interview"),
        ("Writing up the interview", "inq_organise"),
        ("Sharing the interviews", "inq_present"),
        ("Looking back on the term", "inq_reflect"),
    ] },
    Unit { term: 3, from: 1, title: "Our town's future", weeks: &[
        ("Our town and the world", "intl"),
        ("A proposal for the town: questions", "inq_questions"),
        ("The proposal: research", "inq_sources"),
        ("The proposal: data", "inq_charts"),
        ("The proposal: writing", "inq_organise"),
        ("The proposal: slides", "inq_poster"),
        ("The proposal: presenting", "inq_present"),
        ("Peer review", "inq_reflect"),
        ("Getting ready for the 2nd year's work experience", "careers"),
        ("What I want to be", "careers"),
        ("The year's portfolio", "inq_reflect"),
        ("The year's portfolio", "inq_reflect"),
    ] },
];

// ------------------------------------------------------------------------------------- class activities

pub static CLASS: &[Unit] = &[
    Unit { term: 1, from: 1, title: "Making 1-A", weeks: &[
        ("Introducing ourselves", "ca_intro"),
        ("Electing the class officers", "ca_election"),
        ("Choosing committees", "ca_committees"),
        ("The class goal", "ca_goal"),
        ("Lunch and cleaning rotas", "ca_rota"),
        ("Planning the outdoor school", "ca_plan"),
        ("A study plan for the mid-terms", "ca_study"),
        ("Changing seats (sekigae)", "ca_seats"),
        ("The class rules, again", "ca_goal"),
        ("A rainy-day class game", "ca_game"),
        ("Staying safe in summer", "ca_safety"),
        ("A study plan for the finals", "ca_study"),
        ("The term so far", "ca_reflect"),
        ("A plan for the summer homework", "ca_study"),
        ("Looking back on the term", "ca_reflect"),
    ] },
    Unit { term: 2, from: 1, title: "Festivals", weeks: &[
        ("Sports festival: the teams", "ca_plan"),
        ("Sports festival: the cheering", "ca_plan"),
        ("Sports festival: who runs what", "ca_election"),
        ("After the sports festival", "ca_reflect"),
        ("Culture festival: what our class does", "ca_plan"),
        ("Chorus contest: conductor and pianist", "ca_election"),
        ("Changing seats", "ca_seats"),
        ("A study plan for the mid-terms", "ca_study"),
        ("Culture festival: roles", "ca_committees"),
        ("After the culture festival", "ca_reflect"),
        ("Second-term officers", "ca_election"),
        ("A class game", "ca_game"),
        ("A study plan for the finals", "ca_study"),
        ("Changing seats", "ca_seats"),
        ("Staying safe in winter", "ca_safety"),
        ("The big cleaning (ōsōji)", "ca_rota"),
        ("Looking back on the term", "ca_reflect"),
    ] },
    Unit { term: 3, from: 1, title: "Becoming 2nd years", weeks: &[
        ("New Year goals", "ca_goal"),
        ("Third-term officers", "ca_election"),
        ("Changing seats", "ca_seats"),
        ("A study plan for the year-end exams", "ca_study"),
        ("A class game", "ca_game"),
        ("Thanking the 3rd years", "ca_plan"),
        ("Messages for the graduates", "ca_plan"),
        ("Last study plan of the year", "ca_study"),
        ("Graduation rehearsal", "ca_plan"),
        ("1-A's year", "ca_reflect"),
        ("Becoming 2nd years", "ca_goal"),
        ("Goodbye, 1-A", "ca_reflect"),
    ] },
];

/// The last school day of the year (the closing ceremony).
pub fn closing_day(year: i32) -> Date {
    Date::new(year + 1, 3, 24)
}

/// Whether a date's lessons are replaced by a ceremony (no lessons, no mini-games): the entrance
/// ceremony, and the closing ceremonies of the 2nd term and the year. (The 1st term's, 20 July,
/// falls on Marine Day.)
pub fn ceremony_day(d: Date) -> bool {
    let y = school_year(d);
    d == term_start(y, 1) || d == Date::new(y, 12, 25) || d == closing_day(y)
}
