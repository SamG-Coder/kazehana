# Kazehana — Game Design (working title)

A 3D life-sim / card-tycoon in a small Japanese town, on our own Rust + Vulkan engine (**Akari**).
You are a 12-year-old first-year student at Kazehana Junior High (中学1年 — Year 7). After your
parents died you moved in with your uncle, who runs **Tsukumodō (つくも堂)**, the town's game and
trading card shop. The ground floor is the shop and the family lives upstairs.

The aim is to become the best player of **Mythbeasts** (working name), the monster-and-spell trading
card game every kid in town plays. You get two edges: the uncle's discount on packs, and the shop
itself. Every month a new season (set) comes out and shakes up what everyone wants.

**Card game scope for now:** the theme is fixed: mystical monsters (dragons, golems, krakens,
phoenixes, wraiths), spells and relics across six attributes (Flame, Tide, Gale, Terra, Radiant,
Umbral) and four rarities (Common, Uncommon, Rare, Mythic) plus holo foils. **The play rules are not
designed yet.** Until then, each card's "hype" value stands in for how good it is, and drives demand
and prices in the life sim.

---

## 1. Pillars

1. **Living calendar (in the style of Persona).** Each day has school, lunch, after-school, evening
   and night. You can't do everything, so how you spend time is the core decision.
2. **Get to know people slowly.** The school has 100 students. At the start they are all strangers:
   you only learn someone's name, what they like, what deck they play and which cards they need by
   spending time with them.
3. **A card economy with a real meta.** Twelve monthly seasons. Card value comes from how good the
   card is, how many were opened in town and who needs it. Selling to classmates is personal: each
   kid pays according to their own need, wallet and how much they trust you.
4. **A small, believable town.** Kazehana-chō has about 1,500 people. The school is capped at 100
   students, about 33 per year and two classes per year. Everyone knows everyone, so gossip spreads
   your reputation.

## 2. World and levels

Three levels, each a separate scene with walkable exits between them:

| Level | Contents | Main activities |
|---|---|---|
| **Home: Tsukumodō** | Shop floor (counter, pack wall, singles case, play tables), stockroom, stairs to living area, your bedroom, uncle's kitchen | Buy packs at a discount, work shifts, sell singles, weekend tournaments, build decks, sleep and save |
| **Path to School** | Narrow street of wooden houses, konbini, a small Inari shrine with torii, river bridge, rice paddies, level crossing, vending machines, utility poles | Commute (about 10 min of game time), chance meetings, shrine visits (luck), konbini snacks (energy) |
| **School grounds** | Gate, sports ground, shoe lockers, classroom 1-A, corridor, roof, library, gym, club room (Card Club) | Class (skipped as a time jump with mini events), lunch and after-school socialising, trades and duels |

Exits: Home front door ↔ Path (south end). Path (north end) ↔ School gate.

### Art pipeline

- **Environment:** Fab assets (glTF/FBX → glTF) for buildings, props and characters. Where Fab has
  no match, **Fable 5.2** generates realistic models (or procedural generators) styled to fit the
  Fab set.
- **Cheap realism from SmartImposter:** port to raster these techniques:
  - octahedral parallax impostors for trees, hedges and rice plants
  - the analytic atmosphere and cloud noise (`atmosphere.glsl`, `clouds.glsl`)
  - grass cards for paddies and verges
  - height-mapped materials (parallax occlusion) for brick walls, roof tiles and asphalt
- **Cards:** ChatGPT generates the illustrations only. Code makes everything else: the frame,
  attribute colours, rarity gem, name, type line, flavour text and holo foil, through `tools/cardgen`. Missing
  art gets a procedural placeholder. `assets/cards/art_requests.csv` lists every illustration
  needed, with a ready-to-paste prompt.

## 3. Time and calendar

- The game starts **Monday 6 April 2026**, the first day of the Japanese school year.
- **Clock:** by default 1 real second is 1 game minute, so a full day lasts 24 minutes. Sun and moon
  positions are real for 36°N, so you get full day/night, seasons and shorter winter days.
- **Day phases** (shown in the HUD, Persona-style):
  Early Morning (6:00) · Morning (7:00) · Class (8:30–12:30) · Lunch (12:30–13:15) ·
  Class (13:15–15:20) · After School (15:20–18:00) · Evening (18:00–21:00) · Night (21:00+).
- **School days (Mon–Fri, except holidays):** be at the classroom by **8:30** or you're marked late,
  which costs knowledge and the teacher notices. Entering the classroom time-skips the lessons, with
  small events such as quizzes or a classmate passing a note. Lunch and After School are free time
  at school.
- **Weekends and holidays:** the shop is busiest. Work shifts for wages, run tournaments and help on
  release day.
- **Curfew is 22:00.** Staying up late lowers tomorrow's energy.
- **Calendar events:** a new season releases on the 1st Saturday of each month at Tsukumodō; the
  monthly shop tournament is on the last Sunday; also exams, Sports Day, the Culture Festival, the
  Obon summer festival, Golden Week, and the summer, winter and spring breaks.

## 4. The player

| Stat | Raised by | Unlocks |
|---|---|---|
| **Knowledge** | Class, library, studying | Exam results; reading the meta (price hints) |
| **Charm** | Hanging out, shop shifts | Better haggling; faster friendships |
| **Courage** | Challenging strangers, tournaments | Talking to older students; high-stakes duels |
| **Duel Rank** | Winning duels | Tournament seeding; respect from the Card Club |
| **Energy** | Sleep, food | Doing activities (each costs energy) |

- **Money:** a weekly allowance of ¥1,000 from the uncle (Sundays), shift wages (¥600/shift), and
  sales.
- **Reputation:** town-wide and spread by gossip. Fair deals raise it; gouging a kid who needed a card
  lowers it with everyone that kid talks to.

## 5. People and relationships

- **99 other students** are generated deterministically: family names repeat across a few local
  families, as in a real small town. Grades 1–3, classes A/B. About 45% play Mythbeasts.
- Each student has a **personality** (Competitive, Collector, Casual, Haggler, Shy, Flashy), favourite
  attributes and monsters, a deck, a wallet refilled by weekly pocket money, and a **want list** recalculated every
  season.
- **Kizuna (絆) rank 0–10** per person. You learn about them in stages:
  1. Rank 0, Stranger: you know their face and year only.
  2. Rank 1, introduced: you learn their name.
  3. Rank 2: hobbies and favourite attribute.
  4. Rank 3: their deck archetype.
  5. Rank 4: their want list, so you can sell cards they need.
  6. Rank 6+: they tell you rumours (who's selling, upcoming meta) and give friend discounts.
  7. Rank 10: best friend, with a unique event chain.
- **Schedules:** each student has preferred spots (roof, library, courtyard, gym, Card Club, the
  shop) by phase and weekday, so you learn where to find people.
- **Uncle (Kenji)** has his own bond. As it rises, your pack discount goes up (20% → 50%), you get
  stock-room jobs, and story beats about your parents unlock.

## 6. Economy

- **Pack:** 6 cards, retail ¥300. **Box:** 20 packs, ¥5,400 retail.
- **Market value** per card is updated weekly:
  `base(rarity) × playability × scarcity × age`, where
  - base: Common ¥20 · Uncommon ¥60 · Rare ¥250 · Mythic ¥1,500
  - hype: how much kids talk about the card (stands in for playability until rules exist)
  - scarcity: copies opened in town vs the expected count
  - age: older seasons get cheaper, except Mythics, which slowly appreciate
- **Selling to students:** what they'll pay depends on
  - **need:** on their want list ×1.6–2.5, a favourite attribute ×1.2, otherwise ×0.5 because it's a
    bargain buy
  - **personality:** Collectors overpay for foils, Hagglers counter-offer
  - **relationship:** friends trust your price
  - **wallet:** they can't pay what they don't have
- If you ask more than about 1.3× fair value, they may refuse, haggle, or accept and resent it
  (Kizuna and reputation go down). You can only see a kid's need once you know them (rank 4), so
  relationships are how you find profit.
- **Selling in the shop:** list singles in the case; the uncle takes 10%. Customers come in on
  weekends and release days.

## 7. Seasons (card sets)

One per month, released on the 1st Saturday. Each has 60 cards: 30 Common, 15 Uncommon, 10 Rare and
5 Mythic. The mix is about 65% monsters, 23% spells and 12% relics. Each season features an
attribute, which changes what's hyped:

| # | Month | Season | Featured |
|---|---|---|---|
| S01 | Apr | Dawn of Dragons | Flame |
| S02 | May | Tidecaller's Deep | Tide |
| S03 | Jun | Stormbreak | Gale |
| S04 | Jul | Starfall Sanctum | Radiant |
| S05 | Aug | Night of a Thousand Lanterns | Umbral |
| S06 | Sep | Moonlit Wilds | Terra |
| S07 | Oct | Ember Crown | Flame |
| S08 | Nov | Fox Lords | Radiant |
| S09 | Dec | Frostbound | Tide |
| S10 | Jan | Dreamwalkers | Umbral |
| S11 | Feb | Oni Uprising | Terra |
| S12 | Mar | Rise of the Phoenix | Gale |

Set lists are generated as placeholders into editable JSON (`assets/cards/sets/Sxx.json`). After
generation the JSON is the source of truth, so names, art prompts and hype can be rewritten by hand.

## 8. Engine: Akari (明かり)

Full hardware ray tracing in the style of SmartImposter (DLSS Ray Reconstruction, TAA fallback),
grounded-realistic art, and levels authored in Blender from Fab assets. The architecture and
phases are in [ENGINE_PLAN.md](ENGINE_PLAN.md).

- `crates/akari`: the engine (being rebuilt around ray tracing)
- `crates/sim`: life simulation. No graphics, unit-tested.
- `crates/tcg`: cards, seasons, packs, collections. No play rules yet.
- `crates/game`: the executable

## 9. Roadmap

See [ENGINE_PLAN.md](ENGINE_PLAN.md) for the phases.
