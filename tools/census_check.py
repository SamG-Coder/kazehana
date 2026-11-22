"""Does the town as built match the census? (assets/town/census.json against assets/town/town.json
and the building models the game loads.)

    python tools/census_check.py            -> a report; shots/grid/census.json for the grid review

Checks:
- every census house is a plan house (same id), of the matching type, its point inside its plot
- every house type has a building model (or it stands empty)
- every census place other than a home has its site (or its shophouse / farm) on the plan
- every household lives in a house that's there; workplaces are places that exist
- per 200 x 112.5 m grid cell (gridtour.rs): the houses and places in it, and which stand empty
"""

import json
import os
import sys
from collections import Counter, defaultdict

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
c = json.load(open(os.path.join(ROOT, "assets/town/census.json"), encoding="utf8"))
t = json.load(open(os.path.join(ROOT, "assets/town/town.json"), encoding="utf8"))
B = os.path.join(ROOT, "assets/buildings")

# the plan's house types and the model family the game loads for each (crates/town/src/scene.rs)
FAMILY = {"modern_gable": "house_modern_gable", "modern_hip": "house_modern_hip", "small_house": "house_modern_small", "shophouse": "shophouse_k7", "farmhouse": "farmhouse", "apartment": "apartment"}
CENSUS_TYPE = {"Modern": ("modern_gable", "modern_hip"), "Small": ("small_house",), "Shophouse": ("shophouse",), "Farmhouse": ("farmhouse",), "Apartment": ("apartment",), "ShopHome": ("shophouse",)}
# a site model per place kind, if one's built (the junior high's four)
SITE_MODEL = {"JuniorHigh": ["school_main", "school_gym", "school_pool", "school_grounds"], "Elementary": ["school_main", "school_gym"],
              "Station": ["site_station"], "StationPlaza": ["site_plaza"], "Shrine": ["site_shrine"], "Temple": ["site_temple"],
              "PostOffice": ["site_post_office"], "TownOffice": ["site_town_office"], "InariShrine": ["site_inari"], "Konbini": ["site_konbini"],
              "CoOp": ["site_coop"], "Petrol": ["site_petrol"], "Drugstore": ["site_drugstore"], "CarRepair": ["site_car_repair"]}
X0, Y0, CW, CH, COLS, ROWS = -1500.0, -562.5, 200.0, 112.5, 15, 12


def has_model(mid):
    return os.path.exists(os.path.join(B, mid, mid + ".gltf"))


def inside(p, poly):
    x, y = p
    n, ins = len(poly), False
    for i in range(n):
        (x1, y1), (x2, y2) = poly[i], poly[(i + 1) % n]
        if (y1 > y) != (y2 > y) and x < (x2 - x1) * (y - y1) / (y2 - y1) + x1:
            ins = not ins
    return ins


def cell_of(p):
    col = int((p[0] - X0) // CW)
    row = int((Y0 + ROWS * CH - p[1]) // CH)
    return (row, col) if 0 <= row < ROWS and 0 <= col < COLS else None


problems = []
plan = {h["id"]: h for h in t["houses"]}
built = {}
for h in c["houses"]:
    ph = plan.get(h["id"])
    if ph is None:
        problems.append(f"census house {h['id']} ({h['kind']}) has no plan house")
        continue
    if ph["type"] not in CENSUS_TYPE.get(h["kind"], ()):
        problems.append(f"house {h['id']}: census {h['kind']}, plan {ph['type']}")
    if not inside(h["at"], ph["plot"]):
        problems.append(f"house {h['id']}: census point {h['at']} outside its plot")
    fam = FAMILY.get(ph["type"])
    built[h["id"]] = h["kind"] == "ShopHome" and has_model("shop_tsukumodo") or bool(fam and has_model(fam))

places = {p["id"]: p for p in c["places"]}
sites = {s["name"]: s for s in t["sites"]}
place_built = {}
for p in c["places"]:
    if p["kind"] == "Home":
        place_built[p["id"]] = built.get(p["house"], False)
        continue
    if p["house"] is not None:  # a shop in a shophouse, a farm's fields
        place_built[p["id"]] = built.get(p["house"], False)
        continue
    s = sites.get(p["name"])
    if s is None:
        problems.append(f"place {p['id']} {p['name']} ({p['kind']}) has no site on the plan")
        place_built[p["id"]] = False
        continue
    if not inside(p["at"], s["poly"]):
        problems.append(f"place {p['name']}: census point outside its site")
    place_built[p["id"]] = all(has_model(m) for m in SITE_MODEL.get(p["kind"], ["-"]))

for hh in c["households"]:
    if hh["house"] not in plan:
        problems.append(f"household {hh['id']} {hh['family_name']}: house {hh['house']} missing")
work = Counter()
for person in c["people"]:
    wp = person.get("workplace")
    if wp is not None:
        if wp not in places:
            problems.append(f"{person['given_name']} {person['family_name']}: workplace {wp} isn't a place")
        else:
            work[wp] += 1

# per grid cell
cells = defaultdict(lambda: {"houses": Counter(), "empty_houses": Counter(), "places": [], "empty_places": [], "people": 0})
for h in c["houses"]:
    k = cell_of(h["at"])
    if not k:
        continue
    kind = plan[h["id"]]["type"] if h["id"] in plan else h["kind"]
    cells[k]["houses"][kind] += 1
    if not built.get(h["id"]):
        cells[k]["empty_houses"][kind] += 1
    cells[k]["people"] += sum(len(c["households"][i]["members"]) for i in h["households"])
for p in c["places"]:
    if p["kind"] == "Home":
        continue
    k = cell_of(p["at"])
    if not k:
        continue
    label = f"{p['name']} ({p['kind']}, {work.get(p['id'], 0)} work here)"
    cells[k]["places"].append(label)
    if not place_built[p["id"]]:
        cells[k]["empty_places"].append(label)

os.makedirs(os.path.join(ROOT, "shots/grid"), exist_ok=True)
out = {f"r{r:02}_c{col:02}": {"houses": dict(v["houses"]), "empty_houses": dict(v["empty_houses"]), "places": v["places"], "empty_places": v["empty_places"], "people": v["people"]} for (r, col), v in sorted(cells.items())}
json.dump(out, open(os.path.join(ROOT, "shots/grid/census.json"), "w", encoding="utf8"), ensure_ascii=False, indent=1)

kinds = Counter(plan[h["id"]]["type"] for h in c["houses"] if not built.get(h["id"]))
empty_places = [p for p in c["places"] if p["kind"] not in ("Home",) and p["house"] is None and not place_built[p["id"]]]
print(f"census: {len(c['houses'])} houses, {len(c['households'])} households, {len(c['people'])} people, {len(c['places'])} places")
print(f"houses standing empty (no model for their type): {sum(kinds.values())} -> {dict(kinds)}")
print(f"shops with no shophouse built: {sum(1 for p in c['places'] if p['kind'] == 'Shop' and not place_built[p['id']])} of {sum(1 for p in c['places'] if p['kind'] == 'Shop')}")
print(f"sites with nothing built: {len(empty_places)}: " + ", ".join(f"{p['name']} ({work.get(p['id'], 0)} work there)" for p in empty_places))
print(f"grid cells with something in them: {len(cells)}; with empty houses or places: {sum(1 for v in cells.values() if v['empty_houses'] or v['empty_places'])}")
print(f"{len(problems)} mismatches between the census and the plan")
for p in problems[:40]:
    print("  " + p)
sys.exit(0)
