# Daily life: the plan

How the player's days work. It's a life sim built on Persona's structure (school days, a calendar, stats, bonds), with one big difference: **time never jumps except when you sleep.** Everything else plays out in the world, at the pace of what you're doing, and you can stop any time. Information lives on things in the world (a calendar on the wall, a journal in your bag, a watch on your wrist), not in menus.

## 1. Time

- The clock always runs: one game minute per real second when you're walking about (a day is 24 real minutes).
- **Doing something speeds time up while you do it**: sitting in class, working the counter, studying at your desk, fishing, waiting on a bench. Each activity has a pace (class is about 20 game minutes a second). You can get up and stop at any moment; the time you spent is what you get.
- **Sleep is the only cut.** Lie down on your futon and the night passes to morning (a nap passes an hour or two). How late you go to bed and how long you sleep sets tomorrow's energy.
- Nothing is a menu choice that eats a block of time ("work a 4-hour shift" goes): you work the counter until you stop or the shop closes.
- School is not skipped either: you sit through lessons (fast), and what you do in them (listen, take notes, doze, doodle, read under the desk) changes what you get. Missing a lesson is something your teacher notices.

## 2. You

- **Needs**: fullness (hunger) and energy. Fullness drops through the day, faster when you're active; when you're hungry, energy falls faster and lessons go in less. Energy drops with activity and comes back with sleep, naps, food and rest.
- **Stats** (as in Persona): knowledge, charm, courage, plus what the activities train.
- **Bonds** with everyone in town (the census: classmates, neighbours, shopkeepers, Uncle Kenji).
- **Money**: the weekly allowance, helping in the shop, odd jobs, selling cards.
- **What you carry**: a school bag (a few slots) and your pockets. At home: your room's shelf and desk, the fridge.

## 3. Things in the world (no menus)

| Instead of a menu | There is |
|---|---|
| The calendar screen | Wall calendars (your room, the shop, the classroom): look at one to read it, flip the months, write your plans on it |
| The people screen, notes, to-dos | Your journal: a notebook in your bag. Open it to read: the diary writes itself from your day; a page per person with what you've learned; things people asked of you; your plans |
| The clock in the corner | Your wristwatch (raise your wrist), the clocks on walls, the station clock, the town's 5 o'clock chime |
| The money counter | Your wallet: check it, and it's in your hand when you pay |
| The inventory screen | Your school bag: open it and the things are inside it |
| The school timetable | A sheet in the journal; the board in the classroom |
| News and events | Notice boards (school, the station, the town's circular board), posters in the shop, the TV at home (the weather for tomorrow) |
| Shop menus | Things on shelves: pick one up, take it to the counter |

What stays as on-screen UI: button prompts, what people say, the card binder (the card team's), and the pause and settings screens.

Pages (calendar months, journal spreads) are drawn as textures for the real objects in the world, made again when what's on them changes.

## 4. Items

A catalog of everything you can hold, buy, eat, give or find (`assets/data/items.json`, `sim::items`):

- **Food and drink**, plenty of it: konbini onigiri, bento, bread, hot snacks, cup noodles, sweets and dagashi, ice cream, drinks and vending machine cans; the town's shops (the bakery, the tofu shop, the butcher's croquettes, the wagashi shop's dango, the greengrocer's fruit); Uncle Kenji's breakfasts and dinners; school lunch. Each has a price, how full it makes you, energy, how long it keeps, the season it's sold in, and where it's sold.
- **School things**: notebooks, pencils, textbooks, the journal itself.
- **Books and reading**: manga, magazines, library books (reading trains something).
- **Cards**: booster packs, sleeves, deck boxes (the card game's own items stay in `tcg`).
- **Tools and seasonal things**: a fishing rod, a bug net and cage, an umbrella, a towel, fireworks for summer.
- **Gifts and keepsakes**: shrine charms, omikuji fortunes, festival prizes.

Every item gets a model so it can sit on a shelf, in your bag or in your hand. Models go through the asset gate and the user's approval one at a time, as with every prop.

## 5. What you can do

**At home (Tsukumodō)**: sleep and nap on your futon; breakfast and dinner with Uncle Kenji; snacks from the fridge; study at your desk; read; write in your journal; sort your binder; help in the shop (serve, restock, sweep); the bath; TV with Kenji (tomorrow's weather); play cards with Kenji.

**At school**: lessons (listen, take notes, doze, doodle); school lunch with your class; classroom cleaning after school; club practice (join one); the library; the roof at lunch; talking to anyone.

**In town**: shopping (the konbini, the town's shops, vending machines); the shrine (pray, draw a fortune); the temple; the river (fish, catch crayfish, skip stones); help a farmer in the paddies; errands and deliveries for Kenji to people's houses; odd jobs people ask for (in your journal); the station plaza; jogging; summer bug catching; festivals.

**Seasonal**: cherry blossoms in April, Golden Week, the rainy season, summer holidays and the summer festival, Obon, sports day, the culture festival, New Year at the shrine.

Everyone else in town follows their own day (the census schedules), so who you can meet depends on where and when you go.

## 6. Building it

1. **Core rules** (`crates/sim`, testable without the game): continuous time with activity pace; needs; the item catalog and food; carrying and storing things; eating; activities you start and stop; class by presence (no skip); the journal's data (diary, people, asks, plans); plans on the calendar; meals with Kenji.
2. **In the game**: the activity runner (pace, stop any time), eating from the bag, the watch, buying at the town's shops and the konbini, sleep at the futon in your room.
3. **The objects**: the calendar, journal, watch, bag and wallet as real objects with their pages drawn as textures, plus props for the catalog's items (through the approval gate).
4. **More activities**: class choices, cleaning duty, clubs, the shrine, fishing, errands and deliveries, farm help.
5. **The year**: festivals and seasonal events.
