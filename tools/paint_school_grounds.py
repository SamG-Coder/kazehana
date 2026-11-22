"""The junior high's grounds in the land-use map (assets/town/landuse.png), as a school yard is:
packed earth (masa-do) everywhere but the sports ground's own earth; asphalt paths from the gate
to the main building's entrance and to the gym; concrete aprons round the buildings.

    python tools/paint_school_grounds.py

tools/blender/town_terrain.py paints the same when the town is rebuilt (`school_grounds`): this
applies it to the land-use map as it is, without rebuilding the town.
"""

import json
import os
import sys

import numpy as np
from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.join(ROOT, "tools", "blender"))
LANDUSE = {"grass": 0, "paddy": 1, "hatake": 2, "gravel": 3, "concrete": 4, "asphalt": 5, "forest": 6,
           "riverbed": 7, "levee": 8, "garden": 9, "dirt": 10, "shrine": 11, "moss": 12, "ballast": 13}


def site_frame(site):
    """The site's frame from its main building's footprint: origin at the site's north-west
    corner, e along the building (east), s towards the sports ground (south)."""
    b = next(x for x in site["buildings"] if x["name"] == "school building")
    q = [np.array(p) for p in b["poly"]]
    e = (q[1] - q[0]) / np.linalg.norm(q[1] - q[0])
    s = (q[3] - q[0]) / np.linalg.norm(q[3] - q[0])
    # (the building's rect starts 8 m along and 10 m in from the site's corner: town_plan.py)
    o = q[0] - e * 8.0 - s * 10.0
    return o, e, s


def school_grounds(site, gate):
    """(polygon, land use) pairs to paint over the site, in plan coordinates, in order."""
    o, e, s = site_frame(site)
    R = lambda a0, a1, b0, b1: [list(o + e * a + s * b) for a, b in ((a0, b0), (a1, b0), (a1, b1), (a0, b1))]
    out = [(site["poly"], "dirt")]
    g = np.array(gate)
    ga = float(np.dot(g - o, e))
    # the gate's apron, the path from the gate west along the front of the main building to its
    # entrance and east to the gym; aprons round the main building and the gym
    out += [(R(ga - 5, ga + 5, 0, 8), "asphalt"), (R(46, ga + 4, 3.5, 7.5), "asphalt"), (R(ga - 4, 258, 3.5, 7.5), "asphalt")]
    out += [(R(4, 92, 6.5, 30.5), "concrete"), (R(246, 294, 6.5, 46), "concrete"), (R(246, 279, 46, 70), "concrete")]
    out += [(b["poly"], "dirt" if b["name"] == "sports ground" else "concrete") for b in site["buildings"]]
    return out


def main():
    town = json.load(open(os.path.join(ROOT, "assets", "town", "town.json"), encoding="utf8"))
    t = town["terrain"]
    path = os.path.join(ROOT, "assets", "town", t["landuse"])
    img = Image.open(path)
    from PIL import ImageDraw
    d = ImageDraw.Draw(img)
    x0, y1 = t["origin"][0], t["origin"][1] + (t["ny"] - 1) * t["cell"]
    P = lambda p: ((p[0] - x0) / t["cell"], (y1 - p[1]) / t["cell"])
    site = next(s for s in town["sites"] if "Junior High" in s["name"])
    for poly, use in school_grounds(site, town["school_gate"]):
        d.polygon([P(p) for p in poly], fill=LANDUSE[use])
    img.save(path)
    print("painted the school grounds into", path)


if __name__ == "__main__":
    main()
