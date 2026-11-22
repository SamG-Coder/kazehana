# Lessons: the school year as mini-games

You are in class 1-A at Kazehana Junior High for one school year, from the entrance ceremony on Monday 6 April 2026 to the closing ceremony on Wednesday 24 March 2027. **The game ends at the closing ceremony.**

Going to school is your choice. Once you're seated and the bell goes, the period is a short mini-game for that subject: what a real Japanese 1st-year class covers that week (MEXT's course of study, 中学校学習指導要領).

The bells and the week are in `docs/school_plan.md`. This plan covers:
- what each subject teaches, week by week;
- the tests and the report cards;
- how each lesson plays;
- what you bring to a lesson, and what missing one costs;
- where the code is, and what's still thin.

## The year

The year has three terms. Their weeks are counted from the week (Monday to Sunday) of each term's first school day:

| Term | From | To | Weeks with lessons |
|---|---|---|---|
| 1st | Mon 6 Apr 2026 (the entrance ceremony) | Fri 17 Jul (20 Jul is Marine Day) | 15 |
| 2nd | Tue 1 Sep | Fri 25 Dec (the closing ceremony) | 17 |
| 3rd | Fri 8 Jan 2027 | Wed 24 Mar (the closing ceremony; **the game ends**) | 12 |

That's 44 school weeks. Golden Week, the holidays and the breaks come from `sim::calendar`.

### Ceremony days

On these days there are no lessons and no mini-games:
- the entrance ceremony (6 Apr);
- the 2nd term's closing ceremony (25 Dec);
- the year's closing ceremony (24 Mar).

### The tests (teiki tesuto)

Each test runs for three days, in periods 1 to 3. The rest of the day is the normal timetable.

| Tests | Days | Term week | Subjects |
|---|---|---|---|
| 1st-term mid-terms | Tue 19 – Thu 21 May | 1st, week 7 | Japanese, maths, English, science, social studies |
| 1st-term finals | Tue 7 – Thu 9 Jul | 1st, week 14 | the five, plus music, art, PE (health) and technology & home economics |
| 2nd-term mid-terms | Tue 20 – Thu 22 Oct | 2nd, week 8 | the five |
| 2nd-term finals | Tue 1 – Thu 3 Dec | 2nd, week 14 | all nine |
| Year-end tests | Wed 24 – Fri 26 Feb 2027 | 3rd, week 8 | all nine |

The year-end tests run Wednesday to Friday because Tuesday 23 Feb is the Emperor's Birthday. `calendar::events_on` was changed to match.

**What each test covers:**
- a mid-term: the term's weeks up to the test;
- a final: for the five core subjects, the weeks since the mid-term; for the four practical subjects, the whole term;
- the year-end tests: the 3rd term.

In the test week, the five core subjects (and music and technology & home economics in the finals' weeks) have **review weeks**. A review lesson draws on the weeks the test covers.

### The other events

These are already in the calendar:
- the sports festival (last Saturday of September);
- the culture festival (first Saturday of November);
- the Obon festival.

The syllabus follows them:
- PE and class activities spend September on the sports festival;
- integrated studies builds to the culture festival;
- music spends the autumn on the chorus contest.

Events with their own days (the outdoor school, the chorus contest, the graduation ceremony) are not separate game days yet.

## The syllabus

Every subject has a topic and a content key for every one of the 44 weeks:
- the tables are `sim::lessons::syllabus`;
- the questions each key asks are `sim::lessons::content`.

The tables below are printed from the code: `cargo test --release -p sim --lib syllabus_markdown -- --ignored --nocapture`.

What each covers, briefly:
- **Japanese**:
  - kanji (three lists of 1st-year words; radicals, stroke counts, on and kun readings, how compounds are built, homophones, four-character idioms);
  - grammar (bunsetsu, subject and predicate, parts of speech);
  - reading (poetry and its devices, short stories, explanatory writing, fact and opinion);
  - writing and speaking (reports, opinion pieces, speeches, letters);
  - the classics: haiku (Bashō, Buson, Issa, Shiki), Taketori Monogatari, and kanbun with reading marks and sayings from the Analects.
- **Maths**:
  - positive and negative numbers;
  - letters and expressions;
  - linear equations;
  - proportion and inverse proportion;
  - plane figures (lines and angles, moving shapes, constructions, circles, sectors);
  - solid figures (polyhedra, nets, surface area, volume);
  - data and probability (frequency, averages, relative frequency, probability from many trials).
- **English**:
  - the alphabet and phonics;
  - greetings;
  - be-verbs; this and that; he and she;
  - general verbs, do and don't; plurals;
  - can; instructions; the third person -s, does;
  - pronouns; question words; telling the time;
  - the present progressive; want to;
  - the past tense (regular and irregular, did, was and were, the past progressive); directions.
- **Science**:
  - living things (observing, flowers, seed and seedless plants, monocots and dicots, classifying animals);
  - matter (lab safety, metals, density, gases, solutions and solubility, states of matter, distillation);
  - light, sound and force (reflection, refraction, lenses, sound, Hooke's law, weight and mass, pressure);
  - the earth (volcanoes and igneous rocks, earthquakes, strata, fossils, disasters).
- **Social studies**:
  - world geography (the globe, continents and oceans, latitude, longitude and time, Japan's territory, climates, the six regions of the world);
  - history from the Stone Age to the Mongol invasions;
  - Japan's regions, land, climates and disasters.
- **PE**:
  - spring: the fitness test, track, long jump, soccer;
  - summer: swimming;
  - autumn: the sports festival, basketball, dance, mat work, long-distance running, kendo;
  - winter: jump rope, volleyball, the marathon, the softball throw, hurdles;
  - health theory along the way.
- **Music**: the school song, singing in parts and the chorus contest, the alto recorder, notation, tempo and dynamics, Vivaldi's 'Spring', Schubert's 'Erlkönig', the koto and 'Rokudan', gagaku, folk songs, the orchestra, making a melody.
- **Art**: drawing and shading, the colour wheel, complementary colours, tints and shades, lettering and posters, pattern, ukiyo-e, woodblock prints, clay, the Impressionists, perspective, paper craft.
- **Technology & home economics**:
  - technology: materials and making a shelf (wood, saws, planes, joining, safety, metal, plastics), then computers, networks, programming, circuits and energy;
  - home economics: food (nutrients, the six food groups, labels, cooking), clothes (fibres, washing, hand stitches, a button), the home, and consumers.
- **Moral education**: a theme each week from MEXT's four areas (oneself, others, the group and society, life and nature). There are 22 dilemmas, met again later in the year.
- **Integrated studies**:
  - finding out about the town (questions, sources, interviews, field work, charts);
  - telling people at the culture festival;
  - disaster prevention; working people;
  - a proposal for the town's future; careers before the 2nd year's work experience.
- **Class activities**: electing the officers and committees, the class goal, the rotas, changing seats, the festivals, study plans for the tests, thanking the 3rd years, goodbye to 1-A.

### Japanese

| Term | Weeks | Unit | Topics, week by week |
|---|---|---|---|
| 1 | 1–7 | Starting junior high: words and voices | A poem: rhythm and imagery · Kanji: the year's first list · Speaking: introducing yourself · A story: scenes and feelings · Grammar: words, sentences, bunsetsu · Kanji: radicals (bushu) · Review for the tests |
| 1 | 8–15 | Reading to understand | Explanatory writing: fact and opinion · Kanji: strokes and stroke counts · Writing a report · Grammar: subject and predicate · Haiku and season words (kigo) · Kanji: on and kun readings · Review for the tests · Reading for the summer |
| 2 | 1–11 | Thinking in words | Kanji: the second list · Paragraphs and structure · Grammar: parts of speech · Discussion: agreeing and disagreeing · A story: characters and viewpoint · Kanji: how compounds are built · Proverbs and idioms (kotowaza) · Review for the tests · Writing an opinion · Kanji: same sound, different kanji · Grammar: modifiers and conjunctions |
| 2 | 12–17 | The classics: Taketori Monogatari | Old kana spelling (rekishiteki kanazukai) · Taketori Monogatari: the opening · Review for the tests · Taketori Monogatari: Kaguya-hime · Four-character idioms (yoji jukugo) · New Year cards and letters |
| 3 | 1–12 | Chinese classics and the year's words | Kanji: the third list · Kanbun: reading marks (kaeriten) · Kanbun: sayings from the Analects · A story: theme and the reader · Grammar: parts of speech review · Writing: an essay on the year · Kanji: readings of the year · Review for the tests · Speaking: a presentation · Free verse · Kanji: all the year's lists · The year in review |

### Maths

| Term | Weeks | Unit | Topics, week by week |
|---|---|---|---|
| 1 | 1–7 | 1. Positive and negative numbers | Numbers below zero and the number line · Absolute value; comparing numbers · Adding and subtracting · Multiplying and dividing · Powers and the order of operations · Prime numbers and prime factorisation · Review for the tests |
| 1 | 8–14 | 2. Letters and expressions | Using letters for numbers · Substituting values · Like terms: simplifying · Multiplying out brackets · Equalities and inequalities from situations · Review: letters and expressions · Review for the tests |
| 1 | 15–15 | 3. Linear equations | What an equation is |
| 2 | 1–6 | 3. Linear equations | Solving by balancing · Unknowns on both sides · Equations with brackets · Fractions and decimals in equations · Word problems · Proportion equations a : b = c : d |
| 2 | 7–12 | 4. Proportion and inverse proportion | Functions; direct proportion · Review for the tests · Coordinates · Graphs of y = ax · Inverse proportion · Graphs of y = a/x and using proportion |
| 2 | 13–17 | 5. Plane figures | Lines, segments, rays and angles · Review for the tests · Translation, reflection, rotation · Constructions: bisectors and perpendiculars · Circles and tangents |
| 3 | 1–1 | 5. Plane figures | Sectors: arc length and area |
| 3 | 2–6 | 6. Solid figures | Polyhedra, prisms and pyramids · Nets; lines and planes in space · Surface area · Volume of prisms and cylinders · Volume of pyramids, cones and spheres |
| 3 | 7–12 | 7. Data and probability | Frequency tables and histograms · Review for the tests · Mean, median, mode and range · Relative frequency · Probability from many trials · The year in review |

### English

| Term | Weeks | Unit | Topics, week by week |
|---|---|---|---|
| 1 | 1–3 | Unit 0: Hello, English | The alphabet: capitals and small letters · Phonics: letters and sounds · Classroom English and greetings |
| 1 | 4–7 | Unit 1: I am ... / You are ... | I am ... / You are ... · Are you ...? Yes, I am. / No, I'm not. · Numbers 1 to 100 · Review for the tests |
| 1 | 8–15 | Unit 2: This is ... / I like ... | This is ... / That is ... · He is ... / She is ... · I like ... / I play ... (general verbs) · Do you ...? / I don't ... · Plurals: two cats, three boxes · What do you ...? · Review for the tests · Days, months and seasons |
| 2 | 1–3 | Unit 3: I can ... | Can: what you're able to do · Can you ...? / Can I ...? · Instructions: Open ..., Don't ... |
| 2 | 4–10 | Unit 4: My friend from overseas | He plays ... (third person -s) · Does he ...? / He doesn't ... · I, my, me, mine · Who, whose, which · Review for the tests · Where, when, how · Telling the time |
| 2 | 11–17 | Unit 5: What are you doing? | I am reading (present progressive) · What are you doing? · Spelling the -ing forms · Review for the tests · I want to ... / I like ...ing · A letter to a friend overseas · Christmas and New Year |
| 3 | 1–8 | Unit 6: Last weekend | Past tense: regular verbs (-ed) · Past tense: irregular verbs · Did you ...? / I didn't ... · Was and were · I was reading (past progressive) · Reading: a diary · Spelling the -ed forms · Review for the tests |
| 3 | 9–12 | Unit 7: Around town | Showing the way: Go straight ... · Writing about the year · The year's words · The year in review |

### Science

| Term | Weeks | Unit | Topics, week by week |
|---|---|---|---|
| 1 | 1–11 | 1. Living things | Observing living things around school · The loupe and the microscope · Flowers and their parts · Seed plants: angiosperms and gymnosperms · Leaves, stems and roots: monocots and dicots · Plants without seeds: ferns and mosses · Review for the tests · Classifying plants · Vertebrates: five groups · Invertebrates · Classifying animals |
| 1 | 12–15 | 2. Matter around us | Lab safety and the gas burner · Metals and non-metals · Review for the tests · Organic and inorganic matter |
| 2 | 1–8 | 2. Matter around us | Density · Gases: oxygen and carbon dioxide · Gases: hydrogen and ammonia · Solutions and concentration · Solubility and crystals · States of matter · Melting and boiling points; distillation · Review for the tests |
| 2 | 9–17 | 3. Light, sound and force | Light: reflection · Light: refraction · Convex lenses and images · Sound: vibration and its speed · Sound: pitch and loudness · Review for the tests · Forces and what they do · Springs: Hooke's law · Weight, mass and balanced forces |
| 3 | 1–8 | 4. The changing earth | Volcanoes and magma · Minerals and igneous rocks · Earthquakes: P and S waves · Seismic intensity, magnitude, plates · Strata and sedimentary rocks · Fossils: index and facies fossils · The land: disasters and blessings · Review for the tests |
| 3 | 9–12 | 3. Force: pressure | Pressure · Water pressure and air pressure · The year's experiments · The year in review |

### Social studies

| Term | Weeks | Unit | Topics, week by week |
|---|---|---|---|
| 1 | 1–7 | Geography: the world | The globe and the world map · Continents and oceans · Latitude and longitude · Japan's place and its time · Japan's territory and the 47 prefectures · Climates of the world · Review for the tests |
| 1 | 8–15 | Geography: the regions of the world | Asia · Europe · Africa · North America · South America · Oceania · Review for the tests · Researching a region |
| 2 | 1–17 | History: ancient times | Eras and centuries · The first people; Old and New Stone Age · The ancient civilisations · Greece, Rome and the world religions · The Jōmon period · The Yayoi period and Himiko · Kofun and the Yamato kingship · Review for the tests · Asuka: Prince Shōtoku · The Taika reforms and the ritsuryō state · Nara: Heijō-kyō · Nara: Tōdai-ji and the Man'yōshū · Heian-kyō and the Fujiwara · Review for the tests · Kana and court literature · The timeline so far · Reading an old map |
| 3 | 1–5 | History: the samurai | The rise of the samurai · Taira and Minamoto · The Kamakura shogunate · Kamakura culture and new Buddhism · The Mongol invasions |
| 3 | 6–12 | Geography: Japan | The seven regions of Japan · Mountains, rivers and plains · Review for the tests · Japan's climates · Natural disasters and preparing for them · The timeline of the year · The year in review |

### PE

| Term | Weeks | Unit | Topics, week by week |
|---|---|---|---|
| 1 | 1–10 | Spring: fitness and track | Warm-ups and the rules of PE · Fitness test: grip, sit-ups, sit-and-reach · Fitness test: 50 m and the shuttle run · Track: sprint starts · Track: the relay baton · Long jump · Health: growing up · Ball games: soccer · Ball games: soccer matches · Rainy season in the gym: badminton |
| 1 | 11–15 | Summer: swimming | Swimming: freestyle · Swimming: breaststroke · Swimming: a timed 50 m · Health: sleep and rest · Swimming: survival floating |
| 2 | 1–17 | Autumn: the sports festival and games | Sports festival: the class relay · Sports festival: tug of war · Sports festival: the big jump rope · Sports festival: marching in · Ball games: basketball · Ball games: basketball matches · Dance: rhythm dance · Health: injuries and first aid · Mat gymnastics · Mat gymnastics: rolls · Long-distance running · Long-distance running: pacing · Kendo: bowing and footwork · Health: mind and stress · Kendo: strikes · The school marathon · Jump rope (nawatobi) |
| 3 | 1–12 | Winter: kendo, volleyball, running | Jump rope · Kendo: men, kote, dō · Kendo: a match · Volleyball: passing · Volleyball: serving · Volleyball: a match · Health: the environment and health · Long-distance: the marathon · Softball throw · Hurdles · Dodgeball with the class · The class's choice |

### Technology and home economics

| Term | Weeks | Unit | Topics, week by week |
|---|---|---|---|
| 1 | 1–15 | Technology: materials and making | Technology around us · Materials: wood, metal, plastic · Wood: grain and strength · Designing a shelf: the drawings · Marking out · Saws: crosscut and rip · Planes and files · Joining: nails and glue · Finishing · Safety in the workshop · Metal: bending sheet · Plastics · Judging what you made · Review for the tests · Technology and the environment |
| 2 | 1–17 | Home economics: food and clothes | Food and nutrients · The six food groups · A day's meals · Fresh food and labels · Cooking: knives and safety · Cooking: miso soup · Cooking: hamburg steak · Local food and the seasons · Clothes: fibres and care labels · Washing clothes · Sewing: hand stitches · Sewing: a button · Sewing: a small bag · Review for the tests · A safe home · The family and the neighbourhood · Planning a winter meal |
| 3 | 1–12 | Technology: energy and information | How computers work · Networks and staying safe online · Programming: sequences · Programming: loops and branches · Measuring and control · Electric circuits · Energy conversion · Review for the tests · Growing plants · Consumers: buying wisely · Consumers: contracts and money · The year's projects |

### Music

| Term | Weeks | Unit | Topics, week by week |
|---|---|---|---|
| 1 | 1–15 | Singing and the recorder | The school song · Singing: breath and posture · Notes and rests · The alto recorder: first notes · Recorder: a simple tune · Listening: Vivaldi, 'Spring' · Tempo and dynamics · Japanese songs: 'Sakura Sakura' · Singing in parts · Recorder: legato and tonguing · Listening: 'Spring' and its sonnet · Rhythm ensemble · Summer songs · Review for the tests · Singing for fun |
| 2 | 1–17 | The chorus contest and listening | Chorus contest: choosing the song · Chorus: soprano, alto and the men's part · Chorus: blending voices · Chorus: dynamics · Chorus: the whole piece · Chorus contest rehearsal · Listening: Schubert, 'Erlkönig' · 'Erlkönig': four voices in one · Japanese instruments: the koto · 'Rokudan no Shirabe' · Time signatures · Recorder: a duet · Rhythm ensemble · Review for the tests · Winter songs · Recorder: a performance · The instruments of the orchestra |
| 3 | 1–12 | Making music | The orchestra's families · Making a melody · Making a melody: phrases · Recorder: the year's tunes · Listening: gagaku · Folk songs of Japan · Songs for the 3rd years' graduation · Review for the tests · Graduation ceremony songs · Rhythm ensemble · The school song again · The year's songs |

### Art

| Term | Weeks | Unit | Topics, week by week |
|---|---|---|---|
| 1 | 1–15 | Seeing, drawing and colour | What is art? Looking around · Pencils: line and shading · Drawing your own hand · The colour wheel · Primary and secondary colours · Complementary colours · Tints and shades · Design: lettering · Design: a poster · Pattern and repetition · Still life · Watercolour: wet on wet · A landscape from the window · Appreciation: ukiyo-e · A sketchbook for the summer |
| 2 | 1–17 | Prints, banners and clay | Woodblock prints: the design · Woodblock prints: carving · Woodblock prints: printing · Culture festival: the class banner · The banner: a colour plan · The banner: painting · Appreciation: the Impressionists · Clay: a pinch pot · Clay: modelling a face · Clay: glazing · Self-portrait · Self-portrait: colour · One-point perspective · Appreciation: Japanese painting · New Year card design · Collage · Display and reflection |
| 3 | 1–12 | Design and making | Illustration: a story in one picture · Colours and moods · Paper craft: planning · Paper craft: assembling · Appreciation: modern art · Design: a logo · Light and shadow · Mixing greys and browns · Decorations for the graduation · Portfolio · Free work · The year's work |

### Moral education

| Term | Weeks | Unit | Topics, week by week |
|---|---|---|---|
| 1 | 1–15 | Oneself and others | A new start: what kind of person? · Honesty · Friendship · Courtesy · Self-control · Kindness · Courage · Rules and fairness · Working for others · Family · Respecting differences · The value of life · Nature · Gratitude · Looking back on the term |
| 2 | 1–17 | The group and society | Responsibility · Fairness · Our school · Our town and its traditions · People overseas · Honesty online · Effort and failure · Friendship · Kindness to strangers · Rules and freedom · Courage to speak up · Working together · Respecting differences · The value of life · Family · Gratitude · Looking back on the term |
| 3 | 1–12 | Life and the future | New Year: aims · Self-control · Honesty · Nature and beauty · Responsibility · Effort and failure · Fairness · Our town and its traditions · People overseas · Our school · Gratitude to the 3rd years · Looking back on the year |

### Integrated studies

| Term | Weeks | Unit | Topics, week by week |
|---|---|---|---|
| 1 | 1–15 | Finding out about our town | What integrated study is · Choosing a theme about the town · Asking good questions · Where to find out: sources · Using the library · Planning an interview · Interview manners · Field work: walking the town · Field work: notes and photos · Sorting what we found · Charts and maps · Group discussion · Halfway: looking back · A plan for summer research · A plan for summer research |
| 2 | 1–17 | Telling people: the culture festival, safety, work | Sharing the summer research · Designing a poster · Writing the poster · Presenting well · Rehearsal · Culture festival preparation · Culture festival preparation · After the festival: feedback · Disaster prevention: hazard maps · Disaster prevention: evacuating · Disaster prevention: an emergency kit · People who work in the town · Jobs: what people do · Jobs: interviewing a worker · Writing up the interview · Sharing the interviews · Looking back on the term |
| 3 | 1–12 | Our town's future | Our town and the world · A proposal for the town: questions · The proposal: research · The proposal: data · The proposal: writing · The proposal: slides · The proposal: presenting · Peer review · Getting ready for the 2nd year's work experience · What I want to be · The year's portfolio · The year's portfolio |

### Class activities

| Term | Weeks | Unit | Topics, week by week |
|---|---|---|---|
| 1 | 1–15 | Making 1-A | Introducing ourselves · Electing the class officers · Choosing committees · The class goal · Lunch and cleaning rotas · Planning the outdoor school · A study plan for the mid-terms · Changing seats (sekigae) · The class rules, again · A rainy-day class game · Staying safe in summer · A study plan for the finals · The term so far · A plan for the summer homework · Looking back on the term |
| 2 | 1–17 | Festivals | Sports festival: the teams · Sports festival: the cheering · Sports festival: who runs what · After the sports festival · Culture festival: what our class does · Chorus contest: conductor and pianist · Changing seats · A study plan for the mid-terms · Culture festival: roles · After the culture festival · Second-term officers · A class game · A study plan for the finals · Changing seats · Staying safe in winter · The big cleaning (ōsōji) · Looking back on the term |
| 3 | 1–12 | Becoming 2nd years | New Year goals · Third-term officers · Changing seats · A study plan for the year-end exams · A class game · Thanking the 3rd years · Messages for the graduates · Last study plan of the year · Graduation rehearsal · 1-A's year · Becoming 2nd years · Goodbye, 1-A |

## How a lesson plays

Lessons happen in 3D, in classroom 1-A (`crates/game/src/lesson.rs`, `crates/game/src/paper.rs`):

- **The room:** 18 approved desks (`school_desk`) and chairs (`school_chair`), six columns of three rows facing the blackboard. Your seat is the back row by the window. The layout is `assets/buildings/school_main/layout.json`, written by `tools/school_layout.py`; the town places it (`crates/town/src/plots.rs`).
- **Sitting down:** by your desk, E sits you in your seat facing the board.
- **At the bell:** the print (`worksheet`) is on your desk and the subject's notebook (`notebook_open`) open beside it. Their pages are drawn live by `pages::school` from the lesson: the week's questions, your answers in pencil as you write them (faint while you're still writing), and a pencil arrow at the question you're on.
- **Looking:** you sit squared up to the print and lean in over it to write. The look key goes round the print, the notebook and the board.
- **At the bell's end:** the print is marked in red (◯, ✓ and the right answer) and handed back. It stays on your desk; the latest few of each subject are kept.
- **Notes:** taking notes copies the board (the topic, its point, worked examples) into the subject's notebook.
- **Where:** in town, only the subjects taught in your own classroom are worked at your desk, plus all the tests. Science, PE, music, art and technology wait for their rooms.

**Controls**, with the keyboard or the controller:

| Key | Controller | Does |
|---|---|---|
| ← → | d-pad | point at an answer on the print (a number: ↑↓ ±1, ←→ ±10; an order: choose each piece) |
| E or Enter | Cross | write it |
| Esc | Circle | put your head down (doze); again to wake |
| Tab | Triangle | take notes |
| C | Options | look: the print, the notebook, the board |
| P | Create | raise your hand to go to the toilet (the parent session's rule) |

**The clock runs while you play.** The period's 50 minutes are paced to the lesson's tasks, so a period lasts about 1½ to 3 real minutes, and the lesson ends with the bell:
- unfinished tasks count as nothing;
- if you finish early, you sit out the rest at the class's usual pace.

You get one go at each period. If you sit down late, the lesson has fewer tasks: you've missed the start.

### The mechanics

| Mechanic | How it plays | Used in |
|---|---|---|
| **Kanji rain** (timed choice) | The question falls; press the direction of the right answer before it lands. | Japanese kanji, radicals, old kana spelling, four-character idioms; English letters, plurals, spellings, verb forms, vocabulary |
| **Choice** | Up to four answers on the four directions. | everything with a bank of questions |
| **Number dial** | ↑↓ ±1, →← ±10, then confirm. Negative numbers and one decimal place are allowed. | maths (every week's problems are made fresh each lesson), science and social calculations (density, concentration, P-S time, time zones, dates) |
| **Order** | Put the pieces in order: ←→ to choose, confirm to place, ↓ to undo. | English sentence building, Japanese structure (speeches, reports, Taketori's opening, kanbun), maths constructions, the history timeline, the steps of a lab, a recipe, sewing, a woodblock print, an interview, an evacuation |
| **Sort** | One thing at a time into the left or the right bin. | science (angiosperm or gymnosperm, monocot or dicot, metal or not, index or facies fossils...), fact or opinion, on or kun readings, good research questions, the emergency kit |
| **Timing** | A marker sweeps; confirm while it's in the zone. Every sport has its own speed, zone and number of goes. | PE |
| **Recall** | Watch a phrase of notes (do re mi so; the recorder's so la ti do; rhythm syllables), then play it back. | music |
| **Slider** | Turn the colour or the tone to match: mix the colour on the card, paint its complement, shade to a tone. | art |
| **Reflect** | There's no wrong answer: choose what you'd do. What you choose leans to a virtue (kindness, honesty, courage, fairness, responsibility, respect), which feeds charm and courage. | moral education, class activities |

**The three full games** mix their mechanics week by week:

| Subject | Tasks per lesson | Mechanics |
|---|---|---|
| Maths | 8 | made fresh each lesson: number dials, choices, construction orders |
| Japanese | 8 | kanji rain, number (bunsetsu and stroke counts), sort, order, choice |
| English | 8 | rain, choice, sentence building, number (English number words) |

The other subjects have one or two mechanics each:
- science and social studies: 6 tasks;
- technology & home economics, music and art: 6 tasks;
- PE: 4 timing rounds;
- integrated studies: 5 tasks;
- moral education and class activities: 2 discussions.

### What you bring to a lesson

| | Effect |
|---|---|
| **Energy** | Each task's time runs from 100% (rested) down to 55% (exhausted). Tired, PE's zones feel narrower too. |
| **Hunger** | Hungry: time ×0.8, learning ×0.75. Starving: time ×0.6, learning ×0.5. |
| **Notes** (Tab) | Learning ×1.2. Costs more energy (the sim's Notes focus). |
| **Dozing** (Esc) | The tasks go by three times as fast and score nothing. You get energy back (the Doze focus), and the teacher may catch you, which wakes you. |
| **Knowledge** | The old single stat still grows in class (by focus), plus 1.5 × the lesson's score. |

### What a lesson teaches: mastery

Every week of every subject has a **mastery** from 0 to 1: how much of that topic you've learned.

**A played lesson** raises the week's mastery by its score × learning. Subjects with one lesson a week learn as much per lesson as a four-a-week subject does in four, so each topic gets a fair go.

**A lesson you sit through without playing** teaches by your focus:

| Focus | Counts as a score of |
|---|---|
| listening | 0.45 |
| notes | 0.6 |
| doodling | 0.15 |
| dozing | 0 |

**A review lesson** spreads half its score over the weeks it reviews.

**What sitting out costs:**
- **A missed lesson** (under ¾ of the period seated):
  - it teaches nothing, so that week's mastery stays where it is, and the tests draw on it;
  - it counts as 0 in the term's lesson average;
  - it lowers your attendance.
- **Skipping a test** gets 0, marked absent. No make-up tests yet.
- **Dozing through a test** halves what you know of it.

## Marks

### Test marks

**A test you play:** mark = 60 × your score + 40 × your mastery of the weeks it covers. So:
- a perfect paper with nothing learned gets 60;
- what you've learned carries the rest.

**A test you sit without playing** is marked from your mastery and your focus.

### Report cards (tsūchihyō)

You get one on each term's last day: 17 Jul, 25 Dec, and the closing ceremony.

**The nine subjects with marks** get 1 to 5 each term:
- the score is 50% the term's tests + 35% the term's lesson average + 15% attendance (practical subjects with no test that term: 70% lessons, 30% attendance);
- 85 and up is a 5, 70 a 4, 45 a 3, 25 a 2, and below that a 1.

**Moral education, integrated studies and class activities** get a written comment, as real report cards give.

**The year's mark** for each subject is the three terms' marks together.

A good student (80% right, every lesson) gets mostly 4s and some 5s, with tests averaging about 80 (`tests/lessons_year.rs`). One who never goes gets 1s.

## The end of the year

At 10:30 on 24 March 2027, after the closing ceremony and the last homeroom, the game ends (`GameState::year_over`).

`close_year` then:
- issues the 3rd term's report card;
- writes the year's summary:
  - whether you were at the ceremony;
  - each subject's year mark with its three terms;
  - the tests' average and your best subject;
  - mornings or afternoons skipped, and times late;
  - knowledge, charm, courage and money;
  - the cards in your collection, and how many friends you have.

The game shows the summary and then the three report cards (←→ to turn), and time stops. It doesn't add any story.

## The code

| Part | Where |
|---|---|
| The year's weeks and terms; every subject's units and weekly topics | `crates/sim/src/lessons/syllabus.rs` |
| The questions for every content key: generators and banks | `crates/sim/src/lessons/content.rs` |
| The mini-game engine: tasks, input, timing, scoring, `auto` for bots | `crates/sim/src/lessons/play.rs` |
| Records, mastery, the tests, the report cards, the year's end; `GameState::begin_lesson`, `finish_lesson`, `close_year` | `crates/sim/src/lessons/mod.rs` |
| Hooks in the sim | `crates/sim/src/state.rs`: the `lessons` field, each seated minute (`lessons_sat`), and the bell (`lessons_bell`) |
| The lesson screen and the year-end screen | `crates/game/src/lesson.rs` (a child module of `game`) |
| Hooks in the game | `crates/game/src/game.rs`: `Mode::Lesson`, `Mode::YearEnd`, and the update, key and draw arms |
| Screenshots | `--screen lesson-<subject>` and `--screen yearend` (`main.rs`) |
| Tests | the syllabus, content, play, marks, tests and report cards: `cargo test -p sim --lib lessons`; a whole year played and a year skipped: `crates/sim/tests/lessons_year.rs`; headless game tests: `game::lesson::tests` |

## Still thin

- **No people in the room yet:** the class and the teacher need sitting, writing and teaching animations (the anime people sessions' `make_people.py`).
- **No chalk on the board yet**, and the special rooms (science, music, art, technology, the gym) aren't fitted out.
- **The report cards aren't on paper in the world yet.** They should be a page in the journal (the journal is another session's). For now they're on the year-end screen, and each card's arrival is a message and a diary line.
- **Test days keep the normal bells.** The tests are periods 1 to 3, then the normal timetable; real test days end at noon. The bell's announcement still names the timetabled lesson (`school::describe`).
- **No make-up tests**, and no homework or summer assignments.
- **Being in the right room** for science, music, art, technology and PE isn't checked: the lesson plays wherever you sit.
- **The events** (the sports festival, the culture festival, the chorus contest, the outdoor school) shape the syllabus but have no days of their own.
- **Content depth:**
  - Japanese and English banks hold 8–16 items a key, so they repeat within a few weeks;
  - the reading passages are short originals, not textbook texts;
  - the kanji lists are samples of each list, not all ~300 1st-year kanji.
- **No sound** yet: the music phrases are shown, not heard.
- **Balance** is first-pass: the time limits, the mark weights and the grade thresholds want playtesting.
