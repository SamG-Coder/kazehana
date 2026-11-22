# Cloth: everything that should move like fabric

What in the game is cloth, and how much simulation each piece gets. The cloth system (`crates/play/src/cloth.rs`) simulates fabric with XPBD: pinned edges, stretch and bending, wind and air drag, collision with people (capsules) and walls, self collision. It already runs the shop's noren (`kzcloth_noren` nodes in a building's glTF, skipped by collision, driven by the game).

Full simulation costs too much to run for every cloth in town at once, so each piece gets one of four tiers.

| Tier | What it does | Cost | When |
|---|---|---|---|
| **A: simulated** | XPBD every frame: people push through it, wind moves it, it settles | the most | near you (within about 15 m), only a few at once |
| **B: wind only** | a cheap sway from the wind field in the vertex positions, no collisions | small | far away, or lots of the same thing (flags, laundry across town) |
| **C: bones** | a few spring bones (for people's clothes): swings with the walk, settles | small | everyone in town, near and far |
| **D: still** | a fixed mesh with fabric shading | none | things that don't move (cushions, folded clothes) |

Tier A pieces fall back to B when you're far away, and wake into A as you come close (a woken cloth starts from its swaying pose).

## Tsukumodō (home and shop)

| Cloth | Tier | Notes |
|---|---|---|
| The shop's noren (the back doorway) | A | done |
| Your room's curtains | A | **built** (`curtain_my_room` on `curtain_rail_2500`, in the layout at the street window; awaiting approval). Opening and closing them is still to do |
| Kenji's room and the living room curtains | A (B when you're not in) | |
| **Your futon**: mattress, quilt, pillow | A while you lay it out, sleep, fold it; D folded in the closet | **built as D** (`futon_folded` on your closet's shelf in the layout, `futon_laid` for the night: its quilt draped by Blender's cloth solver; awaiting approval). Laying it out and folding it (A) is still to do |
| Kenji's futon | D (laid out at night, folded by day) | |
| **Kotatsu quilt** (winter) | A when you sit at it (your legs under it); D otherwise | swaps with the summer low table |
| Bath towels on the rail, the washroom towel | A when you take one; D on the rail | |
| Laundry on the balcony poles | B (A when you're on the balcony) | out on sunny mornings, in by evening |
| The futon over the balcony rail | B | sunny mornings |
| The shop's banner flags (nobori) by the door | B (A when you brush past) | release days, the tournament |
| The awning | D | canvas on a frame: rigid |
| Cushions (zabuton), the uniform on its hanger | D | |

## The town

| Cloth | Tier | Notes |
|---|---|---|
| **Noren of the open shops** (izakaya, soba, ramen, tofu shop, wagashi, coffee shop...) | A near, B far | hung while they're open (from the census's shop hours), taken in at closing |
| Nobori banners (the konbini, petrol station, shops, the shrine) | B | |
| **Laundry and futons on the houses' balconies** | B | by household: washing day and fine weather, so the town looks lived in |
| **Koinobori (carp streamers)** | B | April to Children's Day (5 May), at houses with young boys (from the census) |
| Shide (the zigzag paper on the shrine's rope) | B | |
| Festival banners | B | the summer festival |
| Classroom curtains | B (A when you're in the room) | |
| The gym's stage curtain | D | |
| School flags on the pole | B | |

## People

Fabric on people uses spring bones (tier C): every one of the town's 1,400 can afford a few joints that lag behind and settle. The builder (`tools/blender/make_people.py`, `add_springs`) adds the joints and weights; `crates/anim/src/spring.rs` steps them each frame (`town::people`). **Built** for skirts, aprons and hanging hair. What the legs carry stays with the legs, so a hem still follows a stepping thigh; the springs take the hips' and head's share and add the lag and settle (a walking skirt trails 2 to 4 cm, more on starting and stopping).

| Cloth | Bones | Notes |
|---|---|---|
| **Skirts** (the sailor uniform's pleats, long skirts) | a ring of 8 chains, 2 bones each | done |
| **Aprons** (shopkeepers, Kenji, grandmothers) | one chain of 2 down the front | done (the hem poke-through under aprons is older, still to fix) |
| Kenji's headband tails, the sailor scarf | 2 each | |
| Long coats, the hanten (winter) | 4 round the hem | |
| **Yukata** (the summer festival) | sleeves and hem | |
| Hair (long hair, ponytails, twin tails) | up to 3 chains of 2 behind the head | done for hair hanging below the cheeks; small children's short twin tails have none yet |

## What to build first

1. **Your futon** (tier A): lay it out, sleep, fold it. It's the bed the whole schedule turns on, and the first new tier A piece after the noren.
2. **Your room's curtains** (tier A).
3. **Spring bones for people** (tier C): skirts, aprons, hair. This is the biggest visible gain across the whole town.
4. **Tier B wind sway** and its fallback for tier A pieces far away.
5. **Shop noren in town**, open and closed with the census's hours.
6. **Laundry, futons and koinobori** across town (tier B), placed from the census's households.
7. The kotatsu quilt, towels and the rest.

Each cloth's mesh and pins are made in its Blender builder as a `kzcloth_<name>` node, as the noren is, and each goes through the asset approval like any prop.
