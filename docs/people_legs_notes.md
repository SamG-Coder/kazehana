# People: legs, feet, skirts, shoes, walk — reference notes and rules

Used by `tools/blender/make_people.py` (the `LEG` table, `leg_profile`, `shoe_dims`,
`shoe_style`, `build_shoe`, `FootWork`, the leg block and the skirt in `person`, the leg
parts of `idle`/`walk` in `animate`). The references below are guides only: no specific
character is copied.

## References

- Anime body proportions (head units; legs about half the height; knee about halfway between
  hip and sole, the thigh a touch longer than the shin; anime stretches legs for style):
  - https://easydrawingguides.com/how-to-draw-anime-body-proportions/
  - https://design.tutsplus.com/tutorials/how-to-draw-anime-characters--cms-31864
  - https://animeartacademy.artstation.com/projects/qeLrNN (head-to-body ratio by height and age)
  - https://www.painterartist.com/en/tips/draw-anime/body/
- Ages (toddlers about 4 heads, big head, short round limbs; old people stooped with slightly
  bent knees, shorter steps):
  - https://tips.clip-studio.com/en-us/articles/7022
  - https://tips.clip-studio.com/en-us/articles/6735 (legs and feet)
  - https://drawartwork.blogspot.com/2014/08/childrens-body-proportions.html
- Skirts (pleats as near-vertical lines evenly spaced, "teeth pointing one way" that curve round
  the body, side pleats narrower as they turn away; zigzag at the hem):
  - https://www.animeoutline.com/?p=8192
  - https://easydrawingguides.com/how-to-draw-an-anime-skirt/
  - https://www.clipstudio.net/how-to-draw/archives/157926 (folds and drapery)
- Shoes (draw the foot first; school shoes are loafer-like, black or brown, thin and close to the
  foot with a slightly raised heel; sneakers chunkier with a thick sole; boots: sole, tread,
  shaft):
  - https://www.animeoutline.com/how-to-draw-anime-shoes-step-by-step/
  - https://easydrawingguides.com/how-to-draw-anime-shoes/
  - https://quotetheanime.com/drawing/how-to-draw-anime-shoes-step-by-step/
  - https://tips.clip-studio.com/en-us/articles/17037
  - https://en.wikipedia.org/wiki/Uwabaki (indoor school shoes: white slip-ons, grade colour on
    the toe; not used yet: everyone is outdoors)
- Walk cycle (contact, down, passing, up; weight drops onto the front leg after contact, highest
  at passing as the back foot pushes off its toes):
  - https://en.wikipedia.org/wiki/Walk_cycle
  - https://www.pluralsight.com/blog/tutorials/12-steps-walk-cycle-animation
  - https://download.autodesk.com/us/3dsmax/2011/help/files/WS1a9193826455f5ff-e569a012180ce589112eb.htm

## Rules used (sizes x H, the body height the model is built for)

Legs
- Hip 0.49, knee 0.27, ankle 0.05 (joints unchanged): legs are half the body; thigh and shin equal.
- Shape (`LEG`): thigh .050 wide at the top, tapering to a small knee (.034); a calf swell
  (.036 x .041, its middle .007 behind the bone) just under the knee; thin ankle (.020).
- Women: thigh top +4%, ankle -7%, plus the existing 0.9 width. Under 10: rounder, little taper,
  no calf swell (more so under 6). Old: thighs and calves -6%, half the calf swell.
- The leg's top sinks into the hips (up 0.03, in towards the middle) and tapers there, so the
  torso makes the hip outline and there is no bulge at the join.

Clothes on the legs
- Trousers hang straight from mid-thigh (.040 x .042), slightly wider at the hem, and run past
  the ankle onto the shoe (a break), open at the hem with a lining.
- Shorts: cloth stands off the leg (+8%) with an open hem (a step ring).
- Socks +3% with a cuff step; tights cover the whole leg.
- Farm boots: rubber shaft from the shoe to 0.235 (under the knee), trousers tucked in.

Shoes (`build_shoe`): length .132 (men), .122 (women), +10% for children; the ankle sits 24%
from the heel. Upper on a separate sole, a heel counter, a toe box, toe spring.
- loafer (dark school/office/suit shoes): slim, a low heel block (.017 at the back, .008 under the toes), dark sole;
- sneaker (light-coloured shoes, children, tracksuits): fuller, thick white sole;
- boots (farm wear): taller, flat sole.

Skirts
- From the waist (0.57); flare grows with t^0.65 (fast over the hips, then straighter); always
  clears the hips, the seat and both thighs (a standing check that also allows for old people's
  bent knees); never narrower further down (A-line).
- School (sailor) skirts: 24 knife pleats (kids' skirts 16), stitched flat over the hips and
  opening towards the hem, the fold lines marked sharp so they read; other skirts get a few
  soft folds when long or flared.
- Skinning: the top follows spine/pelvis; lower down up to 70–75% follows the thighs, split
  smoothly left/right, so the hem swings with the legs without tearing.

Walk and idle (`FootWork`)
- Legs are solved from the feet (two-bone IK in the side plane), not keyed by angle.
- Step = 0.74 of hip height (women 0.68, children 0.8, old 0.52); two steps take 1.05 s
  (children 0.86, under-6s 0.78, old 1.35); feet on the ground 62% of the time (old 66%).
- A planted foot moves back at exactly the walk speed (written to the clip's `speed`), so
  feet don't slide. Heel strike with the toe 15° up, rolling on the heel; flat through mid-stance;
  heel up to 24° rolling over the sole's tip as it leaves; the swing foot lifts and clears the
  floor by about a centimetre.
- The hips sit as high as the planted legs allow (98.8% straight), so they dip after contact and
  rise at passing; old people keep their knees bent (95.5%) and stand with the hips 0.007 lower
  (0.012 for a stoop).
- Idle: feet stay put on the floor while the hips sway and breathe; the legs counter the hips'
  roll and turn so they stay upright.

Measured (Blender, sole vertices, walk loop): soles never go below the floor (before: up to
2.7 mm in); a planted foot slides 7–16 mm/s against the walk speed (before: 250–625 mm/s).
