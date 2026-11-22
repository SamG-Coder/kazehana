# Anime faces: the rules the townspeople's heads follow

`tools/blender/make_people.py` builds every head as a deformed sphere (`head_shape`) with the face
painted on a texture (`paint_face`). These are the construction rules it follows, taken from general
anime/manga drawing guides (no particular character copied). Texture coordinates: `u` across the
face (0.5 = middle), `v = 0.5 + z/2` up the unit head before it is shaped (z = -1 the bottom).

## What the guides say

| Rule | Sources |
|---|---|
| Head = a circle (the skull), with the jaw a "U" or wedge under it down to the chin | [animeoutline][ao], [easydrawingguides][edg], [tutsplus][tp] |
| Eyes on the head's halfway line or just below it | [animeoutline][ao], [tutsplus][tp], [easydrawingguides][edg] |
| One eye's width between the eyes | [animeoutline][ao], [tutsplus][tp] |
| Nose halfway between the top of the eyes and the chin; in front view only a dot, dash or small shadow | [animeoutline][ao], [tutsplus][tp] |
| Mouth a little above halfway from nose to chin; a small curve, usually no lips drawn | [animeoutline][ao], [tutsplus][tp], [easydrawingguides][edg] |
| Ears from just under the eye line down to the nose line, set just behind the middle of the side of the head | [animeoutline][ao], [tutsplus][tp] |
| Eyes: no outline; the lids make the shape. Heavy, thick upper lash line (thicker at the outer corner), lighter short lower line | [easydrawingguides][edg], [tutsplus][tp], [animeoutline][ao] |
| Iris tall (vertical oval), pupil the same shape; a big highlight for the main light and a smaller one opposite, placed the same way in both eyes; the lid shades the top of the iris | [animeoutline][ao], [tutsplus][tp] |
| A lid crease line above the eye makes it read bigger | [tutsplus][tp] |
| Men: brows lower, thicker, straighter; eyes narrower, lighter lashes; jaw and nose more defined. Women: rounder face, lashes more exaggerated | [tutsplus][tp], [easydrawingguides][edg] |
| Children: round face (a circle), features low on the head (a big forehead), big round eyes with big irises, tiny nose, no lips, thin brows set higher/apart, round ears | [clip-studio 7042][cs1], [clip-studio 7069][cs2], [easydrawingguides][edg] |
| Teens: longer face than a child's, still rounder than an adult's; chin pointier, features fill more of the face | [clip-studio 7069][cs2], [easydrawingguides][edg] |
| Adults: narrower eyes, squarer chin, a longer brow-to-chin distance | [clip-studio 7042][cs1], [clip-studio 7069][cs2] |
| The elderly: a squarer face that has lost height; thinner eyes with bags; thinner brows; bigger nose and ears; thinner mouth; lines at the eyes, mouth, forehead | [clip-studio 7042][cs1], [clip-studio 7069][cs2] |

[ao]: https://www.animeoutline.com/how-to-draw-anime-face-structure/
[tp]: https://design.tutsplus.com/tutorials/how-to-draw-anime-heads-and-faces--cms-31884
[edg]: https://easydrawingguides.com/how-to-draw-an-anime-head-and-face-in-front-view/
[cs1]: https://tips.clip-studio.com/en-us/articles/7042
[cs2]: https://tips.clip-studio.com/en-us/articles/7069

## The numbers used

`kid` = 1 for small children, 0 from 14 up; `grown` ramps 18→40; `elder` ramps 55→80.

- Head shape: top half a plain sphere (the hair follows it). Lower half: width × `1 − (0.40 − 0.14·kid)·t^1.8`
  (cheeks full, then the jaw in to the chin); front pulled forward `× 1 + (0.14 − 0.06·kid)·t²`;
  chin dropped `(0.17 − 0.1·kid)·t²·front²`; all × 1.1 tall.
- Eyes: centre `v = 0.405 − 0.035·kid` (just under halfway; lower on children), at `u = 0.5 ± (0.17 + 0.03·kid)`.
  Half width 0.095 (women) / 0.09 (men), +14% on children, −10% on the old. Half height 0.074 / 0.06,
  +40% on children, −35% on the old, −6% / −12% on grown women / men.
- Nose tip `v = 0.245` (halfway from the top of the eyes to the chin), a shadow on one side plus a short tip line.
- Mouth `v = 0.162` (+0.022 on children), half width 0.034 / 0.04.
- Ears: shells 0.4 head-radii tall, centred at z ≈ −0.35 (between the eye line and the nose line), 0.12
  behind the middle, leaning back 12°; none under Long, Shoulder or Bob hair.
