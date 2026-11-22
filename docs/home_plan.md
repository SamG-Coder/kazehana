# Home: Tsukumodō, upstairs and behind the shop

The plan for where you and Uncle Kenji live, room by room: the layout, the things in each room, what you do with them, which ones carry information (`docs/life_plan.md`: no menus), and which are cloth (`docs/cloth_plan.md`). Nothing gets built or placed until the user approves it, one piece at a time (the shell first, then each object), as with every asset.

## The building as built (the shell, awaiting approval)

`tools/blender/build_shop_tsukumodo.py` (9 m frontage × 10 m deep, two storeys, street at +Y). The partition moved forward from Y −2.4 to **Y −1.2**: the shop keeps 5 m (Y −1.2 to the shopfront at +3.8), and its fittings moved forward with it (`layout.json`: counter and showcases at Y −0.4, the tables at Y 1.65, five shelves down the west wall and two on the east). The calendar hangs on the partition's shop face (`paper.json`, Y −1.185). Review renders: `shots/home/` (`tools/blender/render_house_review.py`).

Levels: the shop floor is at 0.15 m, the home a step up at 0.33 m (the genkan's earth floor stays at the shop's level); upstairs is at 3.6 m (tatami 5.5 cm on the slab).

### Ground floor (behind the shop, Y −1.3 to −4.82)

```
 street (+Y)
 ┌──────────────────────── shop (5 m) ─────────────────────┐
 │  shelves · showcases · counter · play tables · TV        │
 ├──────────┬────────────────────────────┬──── noren ──┬────┤  Y −1.2
 │stockroom │                            │    hall     │ S  │
 │ 2.7×1.4  ▯ kitchen and dining         ▯ (kitchen's  │ T  │
 ├────┬─────┤ 3.8 × 3.5                  │ glass door) │ A  │
 │bath│wash ▯ window over the sink (back) │─ step up ───│ I  │
 │    ▯room │                            │ genkan      │ R  │
 │    │     │                            │ back door   │ S  │
 └────┴─────┴────────────────────────────┴─────────────┴────┘  Y −5
  x −4.5    −2.6 −1.6                    2.2          3.5  4.5
```

- **Hall**: through the noren from the shop; the stairs on your left going up towards the back; the kitchen's sliding glass door on the right; the genkan at the end, its polished step (agarikamachi) down to the earth floor and the back door.
- **Stairs**: 14 risers of 0.23 m, 0.8 m wide, against the east wall, a handrail on the wall.
- **Kitchen and dining**: 3.8 × 3.5 m, wood floor, the window over the sink in the back wall; sliding glass doors to the stockroom and the washroom.
- **Washroom** (1 × 2.1 m) and **bath** (1.7 × 2.1 m, tiled, a step down, the high frosted window, a hinged door).
- **Stockroom**: 2.7 × 1.4 m, behind the shop's west end.

| Room | In it (each its own approved asset) | You can |
|---|---|---|
| **Stockroom** | boxes of product, Kenji's desk with the order ledger, shipping supplies | restock the shelves (carry boxes out to the shop); read the ledger (what's selling) |
| **Stairs** | a light switch, the shoe-scuffed treads | go up and down |
| **Kitchen and dining** | table and two chairs, **the fridge** (with a memo board), the gas stove and kettle, rice cooker, microwave, sink, cupboard, a wall clock | **breakfast and dinner with Kenji** (7:00, 19:00); take food from the fridge and eat; boil water for cup noodles; read Kenji's notes on the fridge |
| **Bath and washroom** | the bath (ofuro), washbasin and mirror, washing machine, towels | **take a bath** (energy back; evenings); see yourself in the mirror |
| **Genkan** | the shoe cupboard (under the stairs' top), an umbrella stand, a key hook | take your shoes off and on; take an umbrella when it rains |

### Upstairs (floor 3.6 m)

```
 street (+Y)     (windows over the shop sign)
 ┌──────────────────────┬──┬──┬───────────────────────────┐
 │ YOUR ROOM            │cl│cl│ KENJI'S ROOM              │
 │ tatami · side window │os│os│ tatami                    │
 │                      │et│et│                           │
 ├────────── ▯ ─────────┴──┴──┴─────────────────── ▯ ─────┤  Y 1.4
 │ corridor across                                        │
 ├──────────────────── ▯ ──────────────────┬──────────────┤  Y 0.4
 │ LIVING ROOM (tatami, 6.8 × 2.9)         │ corridor  ┌──┤
 │                                         │           │ST│
 ├──────────── ▯ ───────────────┬──────────┤           │AI│
 │ BACK ROOM (tatami)           │ toilet   ▯           │RS│
 │ window · balcony glass doors │          │  landing  └──┤
 └──────────────────────────────┴──────────┴──────────────┘
        BALCONY (aluminium, 3.8 × 0.95, laundry poles)
```

- **Your room** and **Kenji's**: tatami, each with its closet (oshiire, two fusuma, the middle shelf for the futon) back to back on the wall between; their fusuma open off the corridor.
- **Living room**: tatami, its fusuma off the corridor; another fusuma through to the back room.
- **Back room**: tatami, the window and the balcony's sliding glass doors.
- **Toilet**: off the corridor down the stairwell's side; a small frosted window.
- Every room has a round ceiling light (`lights.json`: warmer and dimmer than the shop's tubes).

Doors are nodes the game opens (`kzdoor_*`): fusuma and glass doors slide, the bath's and the toilet's swing. Nothing is furnished: every object below is still its own asset, approved one at a time.

### Your room (the one that matters most)

A tatami room over the shop (4.3 × 3.4 m), the street window looking down on the shop sign, a side window to the west, the closet in the wall towards Kenji's.

| Thing | What it's for | Info it carries | Cloth |
|---|---|---|---|
| **Futon** (mattress, quilt, pillow), folded in the closet by day | **sleep** (the one time skip) and naps; laying it out and folding it away | | yes (quilt and mattress) |
| **Study desk** (a Japanese kids' study desk: shelf, lamp, drawer) | **study**, **write the journal**, sort the binder | | |
| **Wall calendar** | read it, flip the months, write plans on it | the month: holidays, school events, release days, your plans (`pages::calendar`) | (paper) |
| **The journal** (on the desk, or in your bag) | read it, open it anywhere | diary, people, asks, plans (`pages::journal`) | (paper) |
| **Alarm clock** on the desk | set it (when you'll wake: the night's sleep) | the time | |
| **School bag** on its hook | open it: what you carry (the bag's 8 slots) | its contents | (canvas, stiff) |
| **Uniform** on a hanger | changing (school days or not) | | yes |
| **Bookshelf** | read what's on it (books and manga you've bought appear here) | | |
| **Card shelf**: the binder, deck boxes, a storage box | the binder (the card session's), your collection | | |
| **Posters** from the shop | | | |
| **Curtains** on the street window | open and close them (morning light) | | yes |
| **Cushion** (zabuton) | | | static |
| **Ceiling light** with a pull cord | lights on and off | | (the cord) |

### Kenji's room

His futon, a low table with the shop's accounts, a clothes rack, boxes of his own old cards. You don't go in uninvited: a closed door, his space.

### Living room

Where the two of you spend evenings.

| Thing | What it's for | Info | Cloth |
|---|---|---|---|
| **Kotatsu** (winter) / low table (summer) | sit and **watch TV**; eat mikan; homework | | yes (the quilt) |
| **TV** | **watch TV** with Kenji: the evening news and **tomorrow's weather** | the weather, news of the region | |
| **Wall clock** | | the time | |
| **The house calendar** (Kenji's, from the shop's supplier) | the household's month: Kenji writes the shop's dates on it | release days, the tournament, Kenji's errands | (paper) |
| **Landline phone** | calls (later: people ring the house) | | |
| **Electric fan** (summer) / heater (winter) | | | |
| **Bookshelf**, cushions | | | |

### Corridor, toilet, balcony

The toilet; a cupboard; the balcony at the back with laundry poles on the wall arms, and the futon put out over the rail on sunny mornings.

## How the day uses the house

| When | Where | What |
|---|---|---|
| 6:30 | your room | the alarm; fold the futon; uniform on |
| 7:00 | kitchen | breakfast with Kenji (the table) |
| 7:40 | genkan | shoes on, out to school |
| after school | the shop | the counter, restocking, the play tables |
| 19:00 | kitchen | dinner with Kenji |
| evening | living room | TV and the weather; the kotatsu in winter |
| evening | bath | a bath |
| night | your room | study, the journal, the binder; lay out the futon; sleep |

## Information on things (all in the home)

- Calendars: **your room** (your plans), **the living room** (Kenji's: the shop and the house), **the shop** (for customers: release days and the tournament).
- Clocks: your alarm clock, the kitchen and living room wall clocks, the shop's clock. Your wristwatch when you're out.
- The fridge memo board: Kenji's notes to you ("Curry in the fridge. Back at 8.") and the shopping list.
- The TV: tomorrow's weather.
- The journal: on your desk, or in your bag.

## Building it (each step shown for approval)

1. **The shell**: the partition moved forward, the back rooms and the upper floor with walls, sliding doors (fusuma and shōji), the stairs, tatami floors, lights. One approval for the shell.
2. **The things that run the systems**, one at a time: the futon, the study desk, the wall calendar, the journal, the alarm clock, the fridge, the dining table, the TV, the bath.
3. **Furnishing**: everything else in the tables above.
4. **In the game**: the interactions (a prompt when you look at a thing), the activities they start (`sim::life`), the pages on the calendar and journal.
