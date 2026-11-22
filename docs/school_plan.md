# Kazehana Junior High: the building and the school day

The junior high is where you spend most weekdays. This plan covers:
- the building, room by room;
- the day by the bell, and the week's timetable;
- who is where, and what you can do there.

Like everything else, each piece is built and then shown for approval: the shell first, then each object.

## The site

The town plan (`tools/blender/town_plan.py`, `school_site`) puts the school in a bay under the south hills, with its gate on the Minami road. The 600 m walk from Tsukumodō is the Path level.

| Part | Footprint | Notes |
|---|---|---|
| **Main building** (honkan) | 80 × 12.5 m, three storeys, flat roof | classrooms face south over the sports ground; the corridor runs along the north side, towards the gate |
| **Gym** (taiikukan) | 40 × 32 m | PE in winter and on wet days, assemblies, the entrance ceremony, kendo |
| **Pool** | 25 × 16 m | PE in July |
| **Sports ground** | 208 × 88 m of packed earth | PE from spring to autumn, the soccer, baseball and track clubs, the sports festival |

## The main building

A 1970s–80s reinforced-concrete school of the kind every Japanese town has:
- painted cream render;
- long ribbons of aluminium windows on the south face, with a concrete sunshade slab over each floor's windows;
- the corridor windows and the entrance canopy on the north face;
- a stair hall at each end, the west one going on up to the roof.

The ground floor stands 0.3 m above the yard, and each floor is 3.6 m (ceilings at 3.0 m).

The plan runs west to east, along 80 m:

```
 x  -40   -35.5 -31.5    -22.5    -13.5     -4.5      4.5     13.5     22.5     31.5 35.5  40
     ┌──────┬────┬────────┬────────┬────────┬────────┬────────┬────────┬────────┬────┬──────┐ south (sports ground)
 3F  │stairs│ WC │  3-A   │  3-B   │ spare  │ music  │music & │  art   │ tech-  │ WC │stairs│
     │to the│    │        │        │ class  │  room  │art prep│  room  │ nology │    │      │
     │ roof │    │        │        │  room  │        │        │        │  room  │    │      │
 2F  │      │    │  1-A   │  1-B   │  2-A   │  2-B   │library │computer│home ec │    │      │
 1F  │      │    │science │science │nurse · │ENTRANCE│princi- │    staff room    │    │      │
     │      │    │  room  │prep ·  │counsel-│ shoe   │pal ·   │                  │    │      │
     │      │    │        │lunch   │ling    │lockers │broad-  │                  │    │      │
     │      │    │        │prep    │        │        │cast    │                  │    │      │
     ├──────┴────┴────────┴────────┴────────┴────────┴────────┴────────┴────────┴────┴──────┤
     │                         corridor (3 m), every floor                                  │ north (the gate)
     └──────────────────────────────────────────────────────────────────────────────────────┘
```

- **Classrooms** are 9 × 8 m. Each has two sliding doors onto the corridor (front and back), each with a glass panel, and transom windows above the corridor wall.
  - **The front of the room is the east wall:** the blackboard and the teacher's raised platform (kyōdan).
  - **The back wall** has the message board and the students' cubbies (cubbies are a prop).
  - **Students face east**, with the windows on their left: south light falls across a right-hander's page.
- **The entrance:**
  - double glass doors under a canopy on the north side;
  - the earth-floored entry (doma), then a step up to the corridor;
  - the shoe lockers (getabako, a prop): outdoor shoes off, indoor shoes (uwabaki) on.
- **The staff room** is the long room at the east end: the teachers' desks in islands, the vice principal's desk at the head.
- **Toilets** are at both ends of every floor, next to the stairs.
- **The roof** has a fence round its edge. Its door is meant to be locked; you can get through it (docs/GAME_DESIGN.md).

Not part of the shell, each its own prop approved one at a time:
- desks and chairs;
- cubbies;
- shoe lockers;
- the staff room's desks;
- science benches;
- the piano;
- library shelves;
- PCs;
- cleaning lockers.

What's built into the rooms, as the shop's lights are (`tools/blender/school_fitout.py`):
- **Every room:**
  - plaster on the inside of the outer walls and round each window's opening;
  - a wood wainscot with its rail, deep window sills and curtain rails;
  - a tiled ceiling on its T-bar grid, ceiling air conditioners, the lights, switches;
  - the room's plate over its door in the corridor (室名札).
- **Classrooms:** the blackboard and the teacher's platform, the clock and the speaker over the board, notice boards either side of it, the cork board on the back wall.
- **Corridors:**
  - the long hand-washing sinks with their taps, on tiled bodies;
  - the red fire hydrant box and its lamp;
  - notice boards between the classroom doors, the green exit signs.
- **Stair halls:**
  - a wall behind each landing with the full-length mirror on it;
  - handrails on both sides of every flight, the floor's sign.
- **Toilets** (boys' at the west end, girls' at the east):
  - tiled walls and floor, basins and a mirror by the door;
  - urinals with their dividers, cubicles with their partitions and doors, the WCs.
- **The entrance:** the stone-tiled doma a step below the hall, the step's wooden edge.
- **The staff room:** the month's whiteboard, the key cabinet, the kitchenette.
- **Special rooms:**
  - the science room's lab benches (sinks, gas taps), the teacher's demo bench and the specimen cabinets;
  - the home economics room's kitchen islands (sinks, hobs);
  - the art room's long sink;
  - the technology room's tool board;
  - the music room's staved board and acoustic panels;
  - the nurse's basin and mirror.

## The day (census::school, every school day)

| Time | What | Where |
|---|---|---|
| 7:50 | the gate opens; shoes into the lockers | entrance |
| 8:20 | **morning reading** (asa-dokusho): ten minutes with a book | classroom |
| 8:30 | **morning meeting** (asa no kai): the register, the day's notices | classroom, homeroom teacher |
| 8:50 – 12:40 | **periods 1 to 4**, 50 minutes each with 10-minute breaks | classroom or a special room |
| 12:40 | **school lunch** (kyūshoku): the day's lunch-duty students serve it; everyone eats together | classroom |
| 13:05 | lunch break | anywhere: corridor, library, sports ground, the roof |
| 13:25 | **cleaning** (sōji): each class cleans its classroom and a share of the building | classroom, corridors, toilets, stairs |
| 13:50 – 15:40 | **periods 5 and 6** (5th only on Wednesdays) | |
| 15:40 | **end-of-day meeting** (kaeri no kai) (14:40 on Wednesdays) | classroom |
| 16:00 – 18:00 | **clubs** (not Wednesdays): soccer, baseball, kendo, track, soft tennis, brass band, art, science, the card club | ground, gym, rooms |

## The week (census::school::Timetable)

Each class has 29 lessons a week:

| Subject | Lessons a week | Room |
|---|---|---|
| Japanese | 4 | classroom |
| Maths | 4 | classroom |
| English | 4 | classroom |
| Science | 3 | science room |
| Social studies | 3 | classroom |
| PE | 3 | gym or sports ground |
| Technology and home economics | 2 | technology room |
| Music | 1 | music room |
| Art | 1 | art room |
| Moral education | 1 | classroom |
| Integrated studies | 2 | classroom |
| Class activities (LHR) | 1 | classroom |

These are MEXT's hours. The timetable is built from the census's staff:
- **Subject teachers** take their subject.
- **Homeroom teachers** take their own class's moral education, integrated studies and LHR.
- **The 3rd years** have the vice principal for Japanese, and two part-time lecturers from Minato-shi (Mori-sensei for maths, Sasaki-sensei for English), as small schools are staffed.

The rules hold for every class, and a test checks them (`crates/census/tests/town.rs`):
- no teacher is in two rooms at once;
- no subject comes twice in a day for one class;
- the core subjects mostly fall in the morning, and PE, art, music and technology in the afternoon.

Your class, 1-A (homeroom: Nakano-sensei, Japanese):

| | 1 | 2 | 3 | 4 | 5 | 6 |
|---|---|---|---|---|---|---|
| Mon | social studies | maths | class activities | English | PE | Japanese |
| Tue | English | integrated studies | science | Japanese | technology and home economics | PE |
| Wed | science | Japanese | English | social studies | maths | |
| Thu | Japanese | social studies | maths | moral education | PE | music |
| Fri | maths | integrated studies | science | English | technology and home economics | art |

## Who is where

The census's schedules follow the bell:
- **Students**, in every period: their class's lesson (subject, teacher, room).
- **Teachers**: in a lesson with the class they teach, or in the staff room in a free period.
- **Everyone**: morning reading, lunch, cleaning and the meetings with their class.

You can find anyone at school by the time of day, and a test checks this (`students_and_their_teacher_meet_in_each_lesson`).

## What you do (sim::school, sim::state)

**Going to school is your choice; staying is not.** Once you're in the grounds (from the gate opening at 7:50 until the day's last meeting ends), the gate turns you back.

- **In a lesson or a class meeting**, you can't just get up: **raise your hand to ask for the toilet** (E). The teacher lets you go once a lesson ("Go on. Be quick.").
- **Not back in ten minutes**, and someone from your class is sent to look for you.
- **The toilet trip is the only way out**: you can slip out of school on it. That's cutting school: a day absent, and it goes in the diary.
- **In breaks and the lunch break**, you go where you like inside the school.
- **After school**, you come and go as you please (clubs, the card club, home).

(`GameState::school_locked`, `can_get_up`, `ask_toilet`, `leave_school`; tested in `crates/sim/tests/schedule.rs` and the game's `the_school_gate_during_lessons`.)

**The year is the game.** The game lasts one school year, from the entrance ceremony on 6 April 2026 to the closing ceremony in March 2027. Lessons are mini-games following the Year 7 curriculum. That design is being built separately (`docs/lessons_plan.md`).

- **Lessons**: take your seat (listen, take notes, doodle, doze).
  - Each period counts if you sit through three quarters of it, and it's late if you sit down more than five minutes after the bell.
  - The bell names the lesson and the teacher ("The bell. 2nd period: maths with Yamada-sensei.").
  - The diary writes up the morning's and the afternoon's lessons by name.
- **School lunch**: in the classroom, 12:40.
- **The free times** (breaks, lunch break, after school) are yours: people, trades, the library, the roof, the card club.

Still to come:
- cleaning duty and lunch duty (rotas from the class list);
- being in the right room for science, music, art, technology and PE (once the rooms exist);
- tests and marks by subject;
- the school's events: the entrance ceremony, the sports festival, the culture festival, exams;
- the timetable on paper (a page in your journal, the class timetable on the classroom's back wall).

## Building it (each step shown for approval)

1. **The main building's shell**: the floors, walls, windows, doors, stairs, roof, the built-in blackboards, platforms and lights (`tools/blender/build_school.py`).
2. Built, in review:
   - **The gym** (`tools/blender/build_school_gym.py`):
     - concrete walls with steel sliding doors and low vents, ribbed cladding with clerestory windows, a barrel roof on cream arched trusses;
     - inside: birch plywood panelling, a cedar ceiling, the gallery round three sides with an outside steel stair to it, navy blackout curtains, and a glossy maple floor with the courts' lines;
     - the stage with its curtains and the two flags, basketball backboards hung on lattice frames, high-bay lights;
     - the north annex with the entrance.
   - **The pool** (`build_school_pool.py`): raised on its base, the tiled tank, starting blocks, lane ropes, the steps with the shower arch and the foot-bath, the fence and the clock, the changing block.
   - **The grounds** (`build_school_grounds.py`, in the site's frame from the town plan):
     - the gate with granite pillars, the name plate and the accordion gate, the mesh fence on its curb;
     - flagpoles, the assembly platform, drinking taps, benches;
     - on the ground: the track, the goals, the backstop, the iron bars, the long-jump pit;
     - the bike shed.
   - The yard is packed earth with asphalt paths and concrete aprons (`tools/paint_school_grounds.py`).
3. **Placed in the town** on its site (`crates/town/src/plots.rs`). The main building is approved; the rest is shown in review mode.
4. The props, room by room, starting with 1-A: desks and chairs, cubbies, the shoe lockers.
