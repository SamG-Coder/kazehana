# Kazehana-chō: the population plan

How the town's people are made up, so the town feels like a real rural Japanese town rather than a list of NPCs. The data is generated from this plan by `crates/census` (`cargo run -p census --example build`); `docs/census.md` lists every house and who lives in it, and `assets/town/census.json` is the database the game reads.

Everyone is Japanese.

## The shape of the town

Kazehana-chō is an aging rural town on a branch line, half an hour by train from Minato-shi. Young people leave for university and city jobs; the old families stay on their land. The plan aims for:

| | Target | Generated (seed 7) |
|---|---|---|
| People living in town | 1,300–1,600 | 1,392 |
| Household size | 2.5–3.0 | 2.80 |
| 65 and over | about a third | 29% |
| Under 15 | 12–18% | 18% |
| Living alone, 65+ | many widows | 82 households |
| Three generations under one roof | the farmhouses | 59 households |

Family who left (the cities, university) are on record too, about 340 of them: they are the absent sons and daughters, aunts and uncles that people talk about and that come home at New Year and Obon.

The one deliberate bump is the junior high: Kazehana Junior High has 100 students (33 a year, you among the first-years), more than a town this size would have on its own (think of it as the merged school for the valley). The years either side are filled out so the bump is not a hole.

## Families: old clans and newcomers

Households are not made one by one. They come from **clans**: an old family whose head is in their seventies or eighties, living in a farmhouse, a shophouse or an older house.

- **The heir stays.** Usually the eldest son: he and his wife bring up their children in the same house as his parents (three generations). In a farmhouse or shophouse that's three times in four; in an ordinary house less often.
- **Other sons set up house nearby**, in a newer house or an apartment in the same part of town, with their own families. Their children are cousins of the heir's.
- **Daughters marry into other local families** (taking their husband's name), so a grandmother in Kami has grandchildren in the Centre; or they marry away.
- **The rest left**: Tokyo, Osaka, Nagoya, Sendai, Minato-shi. They are on record as away.
- A daughter with no brother may be the heir; her husband marries into her family (mukoyōshi).
- Some heirs never marry (the bachelor farmer).
- Grandparents who have died are kept on record (so brothers and sisters and cousins are linked) but live nowhere.

**Newcomers** fill the houses no old family holds: families who moved out from the city, young couples, singles, widows and widowers on their own, retired shopkeepers, and teachers posted to the town, who live in the apartment blocks.

Every house on the town plan holds one household; an apartment block holds four. Districts have their own old surnames (three per hamlet); the Centre is mixed.

## What people do

| | |
|---|---|
| Under 3 | at home (some at nursery in Minato-shi) |
| 3–5 | kindergarten in Minato-shi on the bus, or at home |
| 6–11 | Kazehana Elementary (they walk in neighbourhood groups) |
| 12–14 | Kazehana Junior High: classes A and B, clubs (most students are in one) |
| 15–17 | high school in Minato-shi, by train |
| 18–22 | mostly away at university |
| Working age | the family shop, the paddies, the town's workplaces, the train or the car to Minato-shi, the home |
| 65+ | retired, but farmers keep farming into their eighties, and the old shopkeepers keep their shops open shorter hours |

- **Shops**: about seventy shophouses, but most have closed. Their shutters are down, with the old couple still living upstairs. Around fifteen to twenty still open, mostly one of each trade (a second barber or izakaya at most). Each has its own closing day.
- **Farms**: every farmhouse family has its paddies. The busy seasons are planting (April–May) and harvest (September–October). Retired farmers keep a vegetable plot.
- **Workplaces in town**: the JA co-op, the post office, the town office branch, the konbini (two shifts), the drugstore, the petrol station, the car repair shop, the station, and the temple (the family nearest it keeps it).
- **The schools**: twelve staff at the junior high (principal, vice principal, six homeroom teachers, specialists, the nurse) and ten at the elementary school. Some are local; others were posted here and live in the flats.
- **Kenji** runs Tsukumodō (the game decides the hours); **you** have no schedule.

## Groups: who knows whom

| Group | Who | When and where |
|---|---|---|
| Neighbourhood block (han) | about ten households nearby | first Sunday: clean-up |
| Kids' association | a district's elementary kids | walk to school together, play after school |
| Friend groups | 3–5 junior high students: same year, mostly the same sex, same class, club or district | after school and weekends: the konbini, the shrine, the station plaza, someone's house |
| Clubs | junior high clubs | after school, not Wednesdays; outdoor clubs on Saturday mornings |
| Mothers | mothers at home with kids at the schools | Wednesday mornings |
| PTA committee | six junior high parents | first Tuesday evening |
| Seniors' club | a district's over-65s | first Thursday morning |
| Gateball | fourteen keen retirees | Tuesday and Friday mornings at the elementary school |
| Fire brigade | local men, 20–52 | first Sunday morning drill |
| Kenji's old classmates | Kenji and three local men his age | first Friday evening |

Relatives in other houses (parents, in-laws, cousins, aunts and uncles) are worked out from the family links; `docs/census.md` lists each person's.

## Daily life

A school day runs from waking to bed: walk to school, lessons, club or friends, home. Commuters catch the 6:50 train and come back after seven. Farmers follow the season. The old take a morning walk to the shrine or temple, shop at the konbini, sit and chat in the afternoon, and go to bed early. Small daily variations come from a hash of the person and the date, so the same day always plays out the same way.

## Looks

Everyone has a look made from their age, sex and job (`crates/census/src/look.rs`, built into a model by `tools/blender/make_people.py`):

- **Height**: by age and sex, from the MEXT school health survey (children and teenagers) and the national health survey (adults; the old grew up shorter). **Build** varies.
- **Skin**: East Asian tones, browner for people who work outside.
- **Hair**: black or dark brown, greying from the fifties, and dyed brown for some young adults. Haircuts follow age and sex: baseball boys buzzed; schoolgirls in ponytails, bobs and braids; grandmothers in short perms or a bun; receding and bald older men.
- **Glasses**: more often with age.
- **Clothes**: junior high uniforms (the black gakuran, the sailor uniform), randoseru and yellow hats for the youngest, farm wear, shop aprons, the konbini shirt, coveralls, suits and office wear for the commute, teachers' shirts or the PE teacher's tracksuit, the priest's samue. Off duty, casual clothes for their age.

## Changing it

Regenerate with another seed (`cargo run -p census --example build 11`), or edit `census.json` by hand. The generator is deterministic, so the same seed always gives the same town.
