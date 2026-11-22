//! Japanese names, by the generation they were popular in.

use crate::Sex;

/// Surnames of the town. Rural hamlets are dominated by a few old families, so each district
/// draws mostly from its own short list.
pub const SURNAMES: [&str; 56] = [
    "Satō", "Suzuki", "Takahashi", "Tanaka", "Itō", "Watanabe", "Yamamoto", "Nakamura", "Kobayashi", "Katō",
    "Yoshida", "Yamada", "Sasaki", "Yamaguchi", "Matsumoto", "Inoue", "Kimura", "Hayashi", "Shimizu", "Mori",
    "Ikeda", "Hashimoto", "Ishikawa", "Ogawa", "Okada", "Hasegawa", "Fujita", "Kondō", "Murakami", "Endō",
    "Aoki", "Sakamoto", "Saitō", "Fukuda", "Ōta", "Nishimura", "Miura", "Fujii", "Okamoto", "Matsuda",
    "Nakajima", "Harada", "Ono", "Takeuchi", "Kaneko", "Wada", "Nakano", "Ishida", "Ueda", "Morita",
    "Hara", "Shibata", "Sakai", "Kudō", "Yokoyama", "Miyazaki",
];

// born 1936-1960 (66-90 in 2026)
const OLD_M: [&str; 24] = [
    "Shigeru", "Isamu", "Tadashi", "Minoru", "Yoshio", "Kiyoshi", "Toshio", "Masao", "Haruo", "Saburō", "Takeo", "Hideo",
    "Kazuo", "Yukio", "Akio", "Mitsuo", "Noboru", "Susumu", "Tsutomu", "Iwao", "Teruo", "Fumio", "Sadao", "Shōji",
];
const OLD_F: [&str; 24] = [
    "Fumiko", "Kazuko", "Setsuko", "Hanako", "Chiyo", "Toshiko", "Yoshiko", "Masako", "Teruko", "Kimiko", "Sachiko", "Michiko",
    "Keiko", "Hiroko", "Kyōko", "Yōko", "Emiko", "Sumiko", "Mitsuko", "Shizue", "Haruko", "Nobuko", "Hideko", "Tomiko",
];
// born 1961-1985 (41-65)
const MID_M: [&str; 24] = [
    "Hiroshi", "Takeshi", "Makoto", "Kazuya", "Daisuke", "Satoshi", "Naoki", "Tetsuya", "Shinji", "Masaru", "Yūji", "Kōji",
    "Tomoya", "Akira", "Jun", "Osamu", "Hideki", "Manabu", "Tsuyoshi", "Masahiro", "Kenichi", "Shinya", "Toshiyuki", "Hirofumi",
];
const MID_F: [&str; 24] = [
    "Yuko", "Akiko", "Naomi", "Kumiko", "Megumi", "Tomoko", "Mayumi", "Kaori", "Emi", "Junko", "Yumi", "Noriko",
    "Ayumi", "Hiromi", "Kayo", "Rie", "Mariko", "Yukari", "Masami", "Eri", "Chie", "Satomi", "Kazue", "Miho",
];
// born 1986-2005 (21-40)
const YOUNG_M: [&str; 20] = [
    "Shōta", "Kenta", "Yūsuke", "Takuya", "Daichi", "Ryōta", "Kazuki", "Naoya", "Yūki", "Tomoki", "Kōhei", "Shōhei",
    "Tatsuya", "Ryōsuke", "Yūta", "Kōsuke", "Hiroki", "Yūya", "Shingo", "Masaki",
];
const YOUNG_F: [&str; 20] = [
    "Misaki", "Ai", "Yuka", "Haruka", "Mai", "Saki", "Ayaka", "Erika", "Kana", "Rina", "Asuka", "Moe",
    "Natsumi", "Chihiro", "Yui", "Sayaka", "Mizuki", "Aya", "Kanako", "Shiori",
];
// born 2006-2026 (0-20)
const KID_M: [&str; 32] = [
    "Haruto", "Sōta", "Yūto", "Ren", "Minato", "Riku", "Hinata", "Kaito", "Daiki", "Takumi", "Kenta", "Shō",
    "Yamato", "Itsuki", "Ryō", "Hayato", "Kōki", "Taiga", "Asahi", "Sora", "Yūma", "Kōta", "Shun", "Tsubasa",
    "Aoi", "Haruki", "Rui", "Sōma", "Yūsei", "Kanata", "Ritsu", "Akito",
];
const KID_F: [&str; 32] = [
    "Himari", "Yui", "Mei", "Sakura", "Hina", "Rin", "Aoi", "Mio", "Yuna", "Koharu", "Akari", "Saki",
    "Nanami", "Riko", "Kokona", "Hana", "Mitsuki", "Ema", "Ayaka", "Miyu", "Natsuki", "Chiharu", "Momoka", "Kaede",
    "Tsumugi", "Yuzuki", "Hinano", "Kanon", "Sara", "Iroha", "Mao", "Honoka",
];

/// The given names someone of this age and sex might have.
pub fn given(sex: Sex, age: u8) -> &'static [&'static str] {
    match (sex, age) {
        (Sex::M, 66..) => &OLD_M,
        (Sex::F, 66..) => &OLD_F,
        (Sex::M, 41..=65) => &MID_M,
        (Sex::F, 41..=65) => &MID_F,
        (Sex::M, 21..=40) => &YOUNG_M,
        (Sex::F, 21..=40) => &YOUNG_F,
        (Sex::M, _) => &KID_M,
        (Sex::F, _) => &KID_F,
    }
}

/// What the shophouses sell. Many in a town like this have closed (shutters down, the old
/// couple still living upstairs).
pub const TRADES: [&str; 28] = [
    "tofu shop", "rice shop", "liquor shop", "barber", "hair salon", "bakery", "hardware shop", "stationery shop",
    "clothes shop", "electrical shop", "florist", "wagashi (sweets) shop", "soba shop", "izakaya", "tatami maker",
    "bicycle shop", "old pharmacy", "fishmonger", "greengrocer", "butcher", "photo studio", "watch and glasses shop",
    "bookshop", "futon shop", "tea shop", "ramen shop", "kimono shop", "coffee shop (kissaten)",
];

/// Where people who left went.
pub const AWAY: [&str; 9] = ["Tokyo", "Tokyo", "Osaka", "Nagoya", "Sendai", "Minato-shi", "Minato-shi", "Yokohama", "Fukuoka"];
