"""The junior high's classroom furniture (docs/school_plan.md, docs/lessons_plan.md): where each
approved prop stands in the main building, in its model frame (Blender axes, metres: +X east,
+Y south towards the sports ground, z up from the yard), for crates/town/src/plots.rs.

    python tools/school_layout.py
-> assets/buildings/school_main/layout.json

The room geometry is tools/blender/build_school.py's (its constants are repeated here so the
approved shell isn't rebuilt to place furniture).

1-A (2nd floor, the first bay from the west, x -31.5 .. -22.5): 17 students (16 in the census and
you), so 18 single desks, six columns of three rows facing the blackboard on the east wall, the
teacher's platform in front of it. Each desk's chair is pulled out behind it. Seat 1 is the front
row by the corridor; numbered down each column from the front, column by column towards the
windows (+Y, the south). Your seat (`"you": true`) is the back row by the window. `spot:` entries
aren't props but places people stand (the teacher's at the front), in the same frame; a person
there faces the way a prop's front would (-Y turned by `yaw`).
"""

import json
import os

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
OUT = os.path.join(ROOT, "assets", "buildings", "school_main", "layout.json")

# --- build_school.py's frame
HT = 0.075                      # half an inner wall
FLOORS = [0.3, 3.9, 7.5]        # finished floor of each storey
YS = 12.5 / 2
T = 0.25
Y1 = YS - T                     # inside the south wall
YC = -2.925                     # the corridor wall's line
SLOT0, BAY = -31.5, 9.0
PLATFORM = 1.0                  # the teacher's platform's depth from the front wall


def classroom(k, floor, name, students, you_seat):
    a, b = SLOT0 + BAY * k, SLOT0 + BAY * (k + 1)
    f0 = FLOORS[floor]
    y0, y1 = YC + HT, Y1
    yc = (y0 + y1) / 2
    cols, rows = 6, 3
    col_pitch, row_pitch = 1.22, 1.18
    front = b - HT - PLATFORM - 1.55          # the front row's desks, 1.55 m from the platform
    out = []
    seat = 0
    for c in range(cols):
        y = yc + (c - (cols - 1) / 2) * col_pitch
        for r in range(rows):
            seat += 1
            if seat > students + 1:
                continue  # (a smaller class: the room keeps one spare desk)
            x = front - r * row_pitch
            # the desk turned to face the board (+X): its student side (-Y) to the west; the chair
            # 0.36 m behind it, pulled out a little
            out.append({"id": "school_desk", "at": [round(x, 3), round(y, 3), f0], "yaw": -90, "room": name, "seat": seat, "you": seat == you_seat})
            out.append({"id": "school_chair", "at": [round(x - 0.40, 3), round(y, 3), f0], "yaw": -90, "room": name, "seat": seat})
    # the back lockers (教室後方ロッカー: tools/blender/build_school_lockers.py) against the back
    # (west) wall, their doors to the class (+X: the prop's front, -Y, turned by +90); and a spot
    # in front of your locker, number students + 1 (the census students 1..16, then you)
    LW, LD, CW, CH, PL = 0.40 * 9 + 0.024, 0.45, 0.40, 0.42, 0.10
    lx = a + HT + LD / 2 + 0.01
    out.append({"id": "school_lockers", "at": [round(lx, 3), round(yc, 3), f0], "yaw": 90, "room": name})
    n = students  # (you: the last on the roll)
    col, row = (n - 1) % 9, (n - 1) // 9
    ly = -LW / 2 + 0.012 + (col + 0.5) * CW      # along the cabinet (its local x)
    # (local (x, y) turned by +90 is (-y, x): the locker's local x runs along the wall, +Y)
    out.append({"id": "spot:locker", "at": [round(lx + LD / 2 + 0.55, 3), round(yc + ly, 3), f0], "yaw": 90, "room": name, "door": f"locker{n}"})
    # the teacher's place: on the platform in front of the board, facing the class (-X); a spot,
    # not a prop (people stand there: sim::presence, game::crowd)
    out.append({"id": "spot:front", "at": [round(b - HT - PLATFORM * 0.55, 3), round(yc, 3), round(f0 + 0.2, 3)], "yaw": -90, "room": name})
    return out


# 1-A: 17 students and a spare desk; you at seat 18, the back row by the window
LAYOUT = classroom(0, 1, "1-A", students=17, you_seat=18)

os.makedirs(os.path.dirname(OUT), exist_ok=True)
with open(OUT, "w") as f:
    json.dump(LAYOUT, f, indent=1)
print(f"wrote {OUT}: {len(LAYOUT)} props")
