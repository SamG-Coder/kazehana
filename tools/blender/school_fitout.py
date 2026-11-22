"""Kazehana Junior High's interior fit-out (docs/school_plan.md): what's built into each room, the
corridors, the stair halls and the toilets of the main building, around its shell
(tools/blender/build_school.py runs this with its own names: `fitout(globals())`).

Built in (fixtures, not furniture; furniture is props, each approved on its own):
- classrooms: a wood wainscot round the walls with its rail, deep window sills, curtain rails, a
  tiled ceiling on its T-bar grid, two ceiling air conditioners, the clock and the speaker over
  the blackboard, notice boards either side of it, light switches
- corridors: the long hand-washing sinks with their taps, the fire hydrant box and its red lamp,
  notice boards between the classroom doors, the room plates over every door (室名札), the
  green exit signs at the stair halls, a skirting
- stair halls: a wall closing the hall behind each landing with the full-length mirror on it,
  handrails on both sides of each flight on brackets, the floor's sign
- toilets: tiled walls; basins and a mirror by the door; urinals and cubicles (boys' at the west
  end, girls' at the east), the cubicles' partitions and doors (left ajar), the WCs
- the entrance: the earth-floored doma a step below the hall, the step's wooden edge
- the staff room: the month's whiteboard, a kitchenette sink, the key cabinet
- special rooms: the science room's lab benches with sinks and gas taps and the teacher's demo
  bench; the home economics room's kitchen islands (sinks, hobs); the art room's long sink; the
  technology room's tool board; the music room's staved board and acoustic panels; a board in
  every room; the nurse's sink
"""

import json
import math
import os

from mathutils import Matrix, Vector

import fixture_kit as fk

JP = {"1-A": "1年A組", "1-B": "1年B組", "2-A": "2年A組", "2-B": "2年B組", "3-A": "3年A組", "3-B": "3年B組", "spare classroom": "多目的室",
      "science room": "理科室", "science prep": "理科準備室", "lunch prep": "給食準備室", "nurse": "保健室", "counselling": "相談室",
      "principal": "校長室", "broadcast": "放送室", "staff room": "職員室", "library": "図書室", "computer room": "パソコン室",
      "home economics": "家庭科室", "music room": "音楽室", "music prep": "音楽準備室", "art prep": "美術準備室", "art room": "美術室",
      "technology room": "技術室", "entrance": "昇降口"}


def fitout(g):
    kz, P, DETAIL = g["kz"], g["P"], g["DETAIL"]
    fb, wall, XF, YF = g["fb"], g["wall"], g["XF"], g["YF"]
    inside, fit, floors = g["inside"], g["fit"], g["floors"]
    FLOORS, CEIL, HT, YC, Y0, Y1, WC = g["FLOORS"], g["CEIL"], g["HT"], g["YC"], g["Y0"], g["Y1"], g["WC"]
    PLAN, slot, STAIR_X, STOREY = g["PLAN"], g["slot"], g["STAIR_X"], g["STOREY"]
    wood, board, cork, alu, steel, wc_tile, glass = g["wood"], g["board"], g["cork"], g["alu"], g["steel"], g["wc_tile"], g["glass"]
    sign_faces, image_material, clock_mat, lamp = g["sign_faces"], g["image_material"], g["clock_mat"], g["lamp"]
    SIGNS, ROOT = g["SIGNS"], g["ROOT"]

    panel = P("school_wainscot", (0.62, 0.48, 0.32), "Wood_Planks", roughness=0.55, normal_strength=0.35)
    rail_m = P("school_wainscot_rail", (0.42, 0.30, 0.18), "Wood_Planks", roughness=0.45, normal_strength=0.3)
    tbar = P("school_ceiling_grid", (0.86, 0.86, 0.84), DETAIL, roughness=0.5, metallic=0.3, normal_strength=0.05, grain=0.05)
    white = P("school_fixture_white", (0.90, 0.90, 0.88), DETAIL, roughness=0.35, normal_strength=0.05, grain=0.05)
    grille = P("school_grille", (0.20, 0.20, 0.20), DETAIL, roughness=0.6, normal_strength=0.1)
    stainless = P("school_stainless", (0.70, 0.71, 0.72), DETAIL, roughness=0.25, metallic=0.95, normal_strength=0.05, grain=0.08)
    porcelain = P("school_porcelain", (0.93, 0.93, 0.92), DETAIL, roughness=0.12, normal_strength=0.02, grain=0.02)
    red = P("school_hydrant_red", (0.62, 0.05, 0.04), DETAIL, roughness=0.4, metallic=0.3, normal_strength=0.05)
    red_lamp = kz.plain_material("kz_neon_school_red_lamp", (0.9, 0.1, 0.08), roughness=0.3, emission=(1.0, 0.05, 0.02), emission_strength=1.0)
    mirror = P("school_mirror", (0.85, 0.86, 0.86), DETAIL, roughness=0.03, metallic=1.0, normal_strength=0.0, grain=0.0)
    partition_m = P("school_wc_partition", (0.60, 0.68, 0.70), DETAIL, roughness=0.35, normal_strength=0.05, grain=0.05)
    brass = P("school_brass", (0.62, 0.48, 0.22), DETAIL, roughness=0.3, metallic=0.9, normal_strength=0.05, grain=0.05)
    lab_top = P("school_lab_bench_top", (0.06, 0.06, 0.06), DETAIL, roughness=0.3, normal_strength=0.1)
    cabinet = P("school_cabinet", (0.78, 0.76, 0.70), "Wood_Planks", roughness=0.5, normal_strength=0.2)
    whiteboard = P("school_whiteboard", (0.93, 0.94, 0.94), DETAIL, roughness=0.08, normal_strength=0.02, grain=0.02)
    acoustic = P("school_acoustic_panel", (0.80, 0.76, 0.66), "Concrete_Floor-f4b85cb0", roughness=0.9, normal_strength=0.6, grain=0.3)
    doma = kz.surface_material("school_doma_tile", "Japanese_Shrine_Stone_Floor", tint=(0.62, 0.60, 0.56), desaturate=0.4)
    plates = image_material("school_room_plates", os.path.join(SIGNS, "school_plates.png"), DETAIL, roughness=0.3, normal_strength=0.02)
    music_board = image_material("school_music_board", os.path.join(SIGNS, "school_music_board.png"), DETAIL, roughness=0.8, normal_strength=0.1)
    cells = json.load(open(os.path.join(SIGNS, "school_plates.json"), encoding="utf8"))

    def plate_x(x, y0, y1, z0, z1, name):
        """A plate in the plane x = const, readable from both sides (sticking out of a wall)."""
        u0, v0, u1, v1 = cells[name]
        uv = [(u0, v0), (u1, v0), (u1, v1), (u0, v1)]
        lo, hi = min(y0, y1), max(y0, y1)
        sign_faces.append(([(x - 0.007, hi, z0), (x - 0.007, lo, z0), (x - 0.007, lo, z1), (x - 0.007, hi, z1)], uv, plates))
        sign_faces.append(([(x + 0.007, lo, z0), (x + 0.007, hi, z0), (x + 0.007, hi, z1), (x + 0.007, lo, z1)], uv, plates))
        fit.box((x - 0.006, lo - 0.01, z0 - 0.01), (x + 0.006, hi + 0.01, z1 + 0.01), alu)

    def plate_y(y, x0, x1, z0, z1, name, facing):
        """A plate flat on a wall at y, facing +Y (facing=1) or -Y (-1)."""
        u0, v0, u1, v1 = cells[name]
        uv = [(u0, v0), (u1, v0), (u1, v1), (u0, v1)]
        yy = y + facing * 0.012
        if facing > 0:
            corners = [(x1, yy, z0), (x0, yy, z0), (x0, yy, z1), (x1, yy, z1)]
        else:
            corners = [(x0, yy, z0), (x1, yy, z0), (x1, yy, z1), (x0, yy, z1)]
        sign_faces.append((corners, uv, plates))
        back = (y, yy - 0.002) if facing > 0 else (yy + 0.002, y)
        fit.box((x0 - 0.01, back[0], z0 - 0.01), (x1 + 0.01, back[1], z1 + 0.01), alu)

    def disc(c, r, facing_x, mat, n=32):
        """A round face (a clock) on a wall at x = c.x, facing +X (1) or -X (-1)."""
        pts = []
        for k in range(n):
            a = 2 * math.pi * k / n
            pts.append(((c.x, c.y + math.cos(a) * r, c.z + math.sin(a) * r), (0.5 + 0.5 * math.cos(a), 0.5 + 0.5 * math.sin(a))))
        for k in range(n):
            (p0, t0), (p1, t1) = pts[k], pts[(k + 1) % n]
            # (facing -X the viewer's right is +Y)
            if facing_x < 0:
                sign_faces.append(([tuple(c), p0, p1], [(0.5, 0.5), t0, t1], mat))
            else:
                sign_faces.append(([tuple(c), p1, p0], [(0.5, 0.5), (1 - t1[0], t1[1]), (1 - t0[0], t0[1])], mat))

    def ac_cassette(x, y, fc):
        fit.box((x - 0.42, y - 0.42, fc - 0.035), (x + 0.42, y + 0.42, fc), white)
        fit.box((x - 0.18, y - 0.18, fc - 0.04), (x + 0.18, y + 0.18, fc - 0.035), grille)
        for (dx, dy) in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            cx, cy = x + dx * 0.3, y + dy * 0.3
            hx, hy = (0.03, 0.25) if dx else (0.25, 0.03)
            fit.box((cx - hx, cy - hy, fc - 0.042), (cx + hx, cy + hy, fc - 0.035), grille)

    def ceiling_grid(x0, x1, y0, y1, fc):
        x = x0 + 0.6
        while x < x1 - 0.1:
            inside.box((x - 0.012, y0, fc - 0.006), (x + 0.012, y1, fc), tbar)
            x += 0.6
        y = y0 + 0.6
        while y < y1 - 0.1:
            inside.box((x0, y - 0.012, fc - 0.006), (x1, y + 0.012, fc), tbar)
            y += 0.6

    def counter(bld, x0, x1, y0, y1, z, hole, top_mat, thick=0.04):
        """A worktop (its top at z) with a rectangular opening for a bowl: four pieces round it."""
        hx0, hx1, hy0, hy1 = hole
        for (a0, a1, b0, b1) in ((x0, x1, y0, hy0), (x0, x1, hy1, y1), (x0, hx0, hy0, hy1), (hx1, x1, hy0, hy1)):
            if a1 - a0 > 1e-3 and b1 - b0 > 1e-3:
                bld.box((a0, b0, z - thick), (a1, b1, z), top_mat)     # (flush pieces: no seams)

    def cupboard_doors(bld, x0, x1, y_face, facing, z0, z1, n):
        """Cupboard doors on a cabinet's face (at y_face, looking along facing): panels standing
        a little proud, a bar handle on each."""
        w = (x1 - x0) / n
        for k in range(n):
            a0, a1 = x0 + k * w + 0.004, x0 + (k + 1) * w - 0.004
            ys = (y_face, y_face + facing * 0.012)
            fk.rbox(bld, (a0, min(ys), z0 + 0.06), (a1, max(ys), z1 - 0.02), cabinet, r=0.003, seg=1)
            hx = a1 - 0.06 if k % 2 == 0 else a0 + 0.06
            yh = y_face + facing * 0.04
            fk.tube(bld, [(hx, y_face + facing * 0.012, z1 - 0.12), (hx, yh, z1 - 0.12), (hx, yh, z1 - 0.28), (hx, y_face + facing * 0.012, z1 - 0.28)], 0.006, stainless, seg=8, caps=False)

    def sink_trough(x0, x1, y_wall, F0, taps, depth=0.55, facing=1):
        """A long hand-washing sink against a wall at y_wall (the corridor on its +Y side if
        facing is 1): a tiled body, a stainless worktop with a long rounded trough sunk in it, a
        pipe along the wall with bib taps over the trough."""
        ya, yb = (y_wall, y_wall + facing * depth)
        lo, hi = min(ya, yb), max(ya, yb)
        fk.rbox(fit, (x0, lo, F0), (x1, hi, F0 + 0.74), wc_tile, r=0.012)
        top = F0 + 0.79
        h0, h1 = sorted((y_wall + facing * 0.13, y_wall + facing * (depth - 0.06)))
        hole = (x0 + 0.07, x1 - 0.07, h0, h1)
        counter(fit, x0 - 0.02, x1 + 0.02, lo, hi + 0.02, top, hole, stainless, thick=0.05)
        fk.basin_bowl(fit, ((x0 + x1) / 2, (h0 + h1) / 2, top), hole[1] - hole[0], h1 - h0, 0.17, stainless, e_top=12, e_bottom=5, flange=0.012, n=48)
        yp = y_wall + facing * 0.05
        fk.tube(fit, [(x0 - 0.02, y_wall, F0 + 1.0), (x0 + 0.02, yp, F0 + 1.0), (x1 - 0.02, yp, F0 + 1.0), (x1 + 0.02, y_wall, F0 + 1.0)], 0.014, stainless, seg=12)
        for k in range(taps):
            x = x0 + (x1 - x0) * (k + 0.5) / taps
            fk.bib_tap(fit, (x, yp, F0 + 1.0), (0, facing, 0), stainless)

    def blackboard(xw, y_mid, F0, face, w=3.6, mat=None):
        """A board on the wall at x = xw facing -X (face=-1) or +X (face=1), its wood frame and
        chalk tray."""
        def bx(a, b, z0, z1, d_in, d_out, m):
            xs = (xw + face * d_in, xw + face * d_out)
            inside.box((min(xs), y_mid + a, z0), (max(xs), y_mid + b, z1), m)
        bx(-w / 2 - 0.05, w / 2 + 0.05, F0 + 0.85, F0 + 2.1, 0.0, 0.03, wood)
        bx(-w / 2, w / 2, F0 + 0.9, F0 + 2.05, 0.03, 0.035, mat or board)
        bx(-w / 2, w / 2, F0 + 0.85, F0 + 0.88, 0.0, 0.1, wood)

    def lab_bench(cx, cy, F0, w=1.8, d=1.2, sink=True, hobs=0):
        """A bench with cupboards on both sides; a sink sunk in its top with a gooseneck tap, gas
        cocks at its ends (science), or hobs either side of the sink (home economics)."""
        x0, x1, y0, y1 = cx - w / 2, cx + w / 2, cy - d / 2, cy + d / 2
        fk.rbox(inside, (x0 + 0.04, y0 + 0.04, F0), (x1 - 0.04, y1 - 0.04, F0 + 0.76), cabinet, r=0.01)
        inside.box((x0 + 0.06, y0 + 0.06, F0), (x1 - 0.06, y1 - 0.06, F0 + 0.08), grille)       # the kick plinth
        cupboard_doors(inside, x0 + 0.06, x1 - 0.06, y0 + 0.04, -1, F0 + 0.08, F0 + 0.74, 4)
        cupboard_doors(inside, x0 + 0.06, x1 - 0.06, y1 - 0.04, 1, F0 + 0.08, F0 + 0.74, 4)
        top = F0 + 0.8
        mat = stainless if hobs else lab_top
        hole = (cx - 0.22, cx + 0.22, cy - 0.17, cy + 0.17)
        counter(inside, x0, x1, y0, y1, top, hole, mat, thick=0.04)
        fk.basin_bowl(inside, (cx, cy, top), 0.44, 0.34, 0.2, stainless, e_top=8, e_bottom=4, flange=0.015)
        fk.cyl(inside, (cx, cy, top - 0.2), 0.03, 0.002, grille, seg=16)
        fk.swan_tap(inside, (cx, cy + 0.27, top), (0, -1, 0), stainless)
        if not hobs:
            for sx in (-1, 1):
                fk.gas_cock(inside, (cx + sx * (w / 2 - 0.12), cy - 0.2, top), (sx, 0, 0), brass, red)
                fk.gas_cock(inside, (cx + sx * (w / 2 - 0.12), cy + 0.2, top), (sx, 0, 0), brass, red)
        for k in range(hobs):
            hx = cx + (-0.68 if k == 0 else 0.68)
            fk.rbox(inside, (hx - 0.3, cy - 0.38, top), (hx + 0.3, cy + 0.38, top + 0.012), grille, r=0.006, seg=2)
            for by in (cy - 0.18, cy + 0.18):
                fk.burner(inside, (hx, by, top + 0.012), steel, steel)
            for kk, kx in enumerate((hx - 0.08, hx + 0.08)):
                fk.cyl(inside, (kx, y0 + 0.04, top - 0.1), 0.022, -0.025, white, seg=16, axis=(0, 1, 0))
                fk.rbox(inside, (kx - 0.004, y0 + 0.012, top - 0.118), (kx + 0.004, y0 + 0.02, top - 0.082), grille, r=0.002, seg=1)

    NORTH, SOUTH, WEST, EAST, T, XE, YS = g["NORTH"], g["SOUTH"], g["WEST"], g["EAST"], g["T"], g["XE"], g["YS"]
    inwall = g["inwall"]

    def lining(F, u0, u1, F0, holes):
        """Plaster on an outer wall's inside (it's concrete right through), and on the sides,
        head and sill of each window's opening back to the sash."""
        wall(inside, F, u0, u1, F0, F0 + CEIL, holes, inwall, -T - 0.012, -T)
        for (a, b, z0, z1) in holes:
            if z0 < F0 + 0.05 or z1 > F0 + CEIL:
                continue
            fb(inside, F, a, a + 0.012, -T, -0.13, z0, z1, inwall)
            fb(inside, F, b - 0.012, b, -T, -0.13, z0, z1, inwall)
            fb(inside, F, a, b, -T, -0.13, z1 - 0.012, z1, inwall)

    for fi, F0 in enumerate(FLOORS):
        lining(SOUTH, -XE + T, XE - T, F0, g["OUTER_HOLES"][(fi, "S")])
        lining(NORTH, -XE + T, XE - T, F0, g["OUTER_HOLES"][(fi, "N")])
        lining(WEST, -YS + T, YS - T, F0, g["OUTER_HOLES"][(fi, "W")])
        lining(EAST, -YS + T, YS - T, F0, g["OUTER_HOLES"][(fi, "E")])

    for fi, F0 in enumerate(FLOORS):
        fc = F0 + CEIL
        ry0 = YC + HT          # rooms' north face (the corridor wall's room side)
        for k, room in enumerate(PLAN[fi]):
            if room is None:
                continue
            a, b = slot(k)
            if room[2] == 2:
                b = slot(k + 1)[1]
            name, kind = room[0], room[1]
            xa, xb = a + HT, b - HT
            yc = (ry0 + Y1) / 2
            # ---- every room: the ceiling's T-bar grid, the wainscot and its rail
            if kind != "entrance":
                ceiling_grid(xa, xb, ry0, Y1, fc)
                for (x0, x1, y0, y1) in ((xa, xa + 0.015, ry0, Y1), (xb - 0.015, xb, ry0, Y1), (xa, xb, Y1 - 0.015, Y1)):
                    inside.box((x0, y0, F0), (x1, y1, F0 + 0.85), panel)
                for (x0, x1, y0, y1) in ((xa, xa + 0.035, ry0, Y1), (xb - 0.035, xb, ry0, Y1), (xa, xb, Y1 - 0.035, Y1)):
                    inside.box((x0, y0, F0 + 0.85), (x1, y1, F0 + 0.89), rail_m)
                # the corridor wall's room side, round its doors
                doors = []
                if kind == "half":
                    m = (a + b) / 2
                    doors = [(m - 1.2, m - 0.3), (m + 0.3, m + 1.2)]
                else:
                    doors = [(b - 1.6, b - 0.7)] + ([(a + 0.7, a + 1.6)] if kind in ("class", "staff", "room") else [])
                wall(inside, YF(YC), xa, xb, F0, F0 + 0.85, [(u0, u1, F0, F0 + 2.0) for (u0, u1) in doors], panel, HT, HT + 0.015)
                wall(inside, YF(YC), xa, xb, F0 + 0.85, F0 + 0.89, [(u0, u1, F0, F0 + 2.0) for (u0, u1) in doors], rail_m, HT, HT + 0.035)
                # window sills and curtain rails over the south windows
                for (c0, c1) in g["bays"](a, b):
                    w0, w1 = c0 + g["COL_W"] / 2 + 0.12, c1 - g["COL_W"] / 2 - 0.12
                    inside.box((w0 - 0.05, Y1 - 0.2, F0 + 0.83), (w1 + 0.05, Y1, F0 + 0.865), rail_m)
                    inside.box((w0 - 0.1, Y1 - 0.13, F0 + 2.82), (w1 + 0.1, Y1 - 0.07, F0 + 2.86), alu)
                # the room plate over its (front) door, sticking out into the corridor
                names = name.split("|")
                if kind == "half":
                    for nm, (u0, u1) in zip(names, doors):
                        plate_x((u0 + u1) / 2, YC - HT - 0.03, YC - HT - 0.5, F0 + 2.08, F0 + 2.2, JP[nm])
                else:
                    u0, u1 = doors[0]
                    plate_x((u0 + u1) / 2, YC - HT - 0.03, YC - HT - 0.5, F0 + 2.08, F0 + 2.2, JP[names[0]])
                # light switches by the front door
                fit.box((b - 0.55, ry0, F0 + 1.15), (b - 0.45, ry0 + 0.015, F0 + 1.3), white)
            # ---- classrooms (and the spare one): the front and the back
            if kind == "class":
                for x in (xa + 2.6, xb - 2.6):
                    ac_cassette(x, yc, fc)
                # the clock over the blackboard, the speaker beside it
                cx = xb - 0.02
                fit.box((xb - 0.025, yc - 0.21, F0 + 2.27), (xb, yc + 0.21, F0 + 2.69), alu)
                disc(Vector((xb - 0.028, yc, F0 + 2.48)), 0.19, -1, clock_mat)
                fit.box((xb - 0.18, yc + 1.4, F0 + 2.35), (xb, yc + 1.85, F0 + 2.65), wood)
                fit.box((xb - 0.181, yc + 1.45, F0 + 2.4), (xb - 0.17, yc + 1.8, F0 + 2.6), grille)
                # notice boards either side of the blackboard
                for s in (-1, 1):
                    y0b = yc + s * 2.15
                    inside.box((xb - 0.025, min(y0b, y0b + s * 1.1), F0 + 1.0), (xb, max(y0b, y0b + s * 1.1), F0 + 2.0), cork)
                    inside.box((xb - 0.03, min(y0b, y0b + s * 1.1) - 0.02, F0 + 0.98), (xb - 0.024, max(y0b, y0b + s * 1.1) + 0.02, F0 + 2.02), rail_m)
            # ---- special rooms
            if kind in ("room", "staff") or name == "spare classroom":
                for x in (xa + 2.6, xb - 2.6):
                    ac_cassette(x, yc, fc)
            if kind == "room":
                blackboard(xb, yc, F0, -1, mat=music_board if name == "music room" else None)
                inside.box((xb - 1.0, yc - 2.3, F0), (xb, yc + 2.3, F0 + 0.2), wood)        # the platform
            if name == "science room":
                for row, y in enumerate((yc - 1.9, yc + 0.4, yc + 2.7)):
                    for x in (xa + 1.7, xa + 4.4):
                        lab_bench(x, y, F0)
                lab_bench(xb - 1.8, yc, F0, w=2.4, d=0.8)                                   # the teacher's demo bench
                inside.box((xa, ry0, F0), (xa + 0.6, ry0 + 3.0, F0 + 2.0), cabinet)        # the specimen cabinets
                for z in (F0 + 0.6, F0 + 1.2, F0 + 1.8):
                    inside.box((xa + 0.58, ry0 + 0.05, z), (xa + 0.605, ry0 + 2.95, z + 0.02), glass)
            if name == "home economics":
                for y in (yc - 2.2, yc + 0.2, yc + 2.6):
                    for x in (xa + 1.7, xa + 4.6):
                        lab_bench(x, y, F0, w=2.2, d=1.0, hobs=2)
            if name == "art room":
                # the long sink on the west wall: tiled body, stainless top and trough, bib taps
                y0s, y1s = ry0 + 0.3, ry0 + 4.3
                fk.rbox(inside, (xa, y0s, F0), (xa + 0.56, y1s, F0 + 0.74), wc_tile, r=0.012)
                hole = (xa + 0.13, xa + 0.5, y0s + 0.07, y1s - 0.07)
                counter(inside, xa, xa + 0.58, y0s - 0.02, y1s + 0.02, F0 + 0.79, hole, stainless, thick=0.05)
                fk.basin_bowl(inside, ((hole[0] + hole[1]) / 2, (y0s + y1s) / 2, F0 + 0.79), hole[1] - hole[0], hole[3] - hole[2], 0.17, stainless, e_top=12, e_bottom=5, flange=0.012, n=48)
                fk.tube(inside, [(xa, y0s, F0 + 1.0), (xa + 0.05, y0s + 0.04, F0 + 1.0), (xa + 0.05, y1s - 0.04, F0 + 1.0), (xa, y1s, F0 + 1.0)], 0.014, stainless)
                for k2 in range(6):
                    fk.bib_tap(inside, (xa + 0.05, y0s + 0.35 + k2 * 0.66, F0 + 1.0), (1, 0, 0), stainless)
            if name == "technology room":
                inside.box((xa, yc - 2.0, F0 + 1.1), (xa + 0.02, yc + 2.0, F0 + 2.3), cork)     # the tool board
                for k2 in range(14):
                    y = yc - 1.8 + k2 * 0.28
                    inside.box((xa + 0.02, y - 0.01, F0 + 1.8), (xa + 0.1, y + 0.01, F0 + 1.82), steel)
            if name == "music room":
                inside.box((xa, ry0, F0 + 0.9), (xa + 0.04, Y1, F0 + 2.8), acoustic)
            if kind == "staff":
                inside.box((xb - 0.03, yc - 2.2, F0 + 0.9), (xb, yc + 2.2, F0 + 2.1), whiteboard)   # the month's schedule
                inside.box((xb - 0.04, yc - 2.25, F0 + 0.87), (xb - 0.025, yc + 2.25, F0 + 2.13), alu)
                for d in range(1, 6):                                                           # its grid of days
                    inside.box((xb - 0.031, yc - 2.2, F0 + 0.9 + d * 0.2), (xb - 0.03, yc + 2.2, F0 + 0.905 + d * 0.2), grille)
                fit.box((xb - 0.15, ry0 + 0.4, F0 + 1.3), (xb, ry0 + 1.0, F0 + 1.9), steel)      # the key cabinet
                # the kitchenette: a cupboard with a stainless top and sink, a gooseneck tap
                fk.rbox(inside, (xb - 0.62, Y1 - 1.9, F0), (xb, Y1, F0 + 0.82), cabinet, r=0.008)
                hole = (xb - 0.5, xb - 0.12, Y1 - 1.35, Y1 - 0.75)
                counter(inside, xb - 0.64, xb, Y1 - 1.92, Y1, F0 + 0.87, hole, stainless, thick=0.05)
                fk.basin_bowl(inside, ((hole[0] + hole[1]) / 2, (hole[2] + hole[3]) / 2, F0 + 0.87), 0.38, 0.6, 0.18, stainless, e_top=8, e_bottom=4)
                fk.swan_tap(inside, (xb - 0.06, Y1 - 1.05, F0 + 0.87), (-1, 0, 0), stainless)
            if name.startswith("nurse"):
                fk.wall_basin(inside, (xa, ry0 + 0.75, F0), (1, 0, 0), porcelain, grille)
                fk.swan_tap(inside, (xa + 0.06, ry0 + 0.75, F0 + 0.8), (1, 0, 0), stainless, h=0.14, reach=0.12, r=0.008)
                inside.box((xa, ry0 + 0.45, F0 + 1.0), (xa + 0.01, ry0 + 1.05, F0 + 1.8), mirror)
            # ---- the entrance: the earth floor (doma) a step down, the step's edge
            if kind == "entrance":
                ceiling_grid(a, b, Y0, Y1, fc)
                inside.box((a, -1.25, F0 - 0.16), (b, -1.15, F0 + 0.004), rail_m)
        # ---- the corridor: skirting, sinks, the hydrant box, notice boards, exit signs
        inside.box((-35.5, Y0, F0), (35.5, Y0 + 0.012, F0 + 0.08), g["dado"])
        for (x0, x1) in ((-27.6, -24.4), (24.4, 27.6)):
            sink_trough(x0, x1, Y0, F0, 5)
        hx = 9.0 if fi == 0 else 0.0
        fit.box((hx - 0.4, Y0, F0 + 0.6), (hx + 0.4, Y0 + 0.17, F0 + 1.8), red)
        fit.box((hx - 0.36, Y0 + 0.17, F0 + 0.64), (hx + 0.36, Y0 + 0.175, F0 + 1.76), red)
        fit.cylinder((hx, Y0 + 0.08, F0 + 1.9), 0.06, 0.12, red_lamp, segments=16)
        for k, room in enumerate(PLAN[fi]):
            if room is None or room[1] not in ("class", "room"):
                continue
            a, b = slot(k)
            yb = YC - HT
            inside.box((a + 1.9, yb - 0.02, F0 + 1.0), (b - 1.9, yb, F0 + 1.95), cork)
            inside.box((a + 1.86, yb - 0.025, F0 + 0.97), (b - 1.86, yb - 0.018, F0 + 1.98), rail_m)
        for x in (-35.3, 35.3):
            fit.box((x - 0.01, -5.3, fc - 0.05), (x + 0.01, -5.25, fc), alu)
            plate_x(x, -5.6, -4.95, fc - 0.35, fc - 0.2, "非常口")
        # ---- the stair halls: the wall behind each landing with its mirror, the floor's sign,
        # the rails on the inside of each flight
        for (xw, xe) in ((STAIR_X[0], STAIR_X[1]), (-STAIR_X[1], -STAIR_X[0])):
            mid = (xw + xe) / 2
            inside.box((xw, 2.7, F0 - 0.25), (xe, 2.85, F0 + STOREY - 0.25), g["inwall"])
            land = F0 + 1.8
            inside.box((mid - 0.5, 2.69, land + 0.15), (mid + 0.5, 2.7, land + 1.95), mirror)
            inside.box((mid - 0.53, 2.685, land + 0.12), (mid + 0.53, 2.7, land + 1.98), alu)
            plate_y(Y0, mid - 0.35, mid + 0.35, F0 + 1.9, F0 + 2.08, f"{fi + 1}F", 1)
            for (x, z0, z1, ya, yb) in ((mid - 0.12, F0 + 0.16 + 0.85, land + 0.85, -2.0, 0.7), (mid + 0.12, land + 0.16 + 0.85, land + 1.8 + 0.85, 0.7, -2.0)):
                n = 10
                for i in range(n):
                    t0, t1 = i / n, (i + 1) / n
                    p0 = Vector((x, ya + (yb - ya) * t0, z0 + (z1 - z0) * t0))
                    p1 = Vector((x, ya + (yb - ya) * t1, z0 + (z1 - z0) * t1))
                    dv = p1 - p0
                    inside.oriented_box((p0 + p1) / 2, (0.04, dv.length, 0.04), Vector((0, 1, 0)).rotation_difference(dv.normalized()).to_matrix(), steel)
                for t in (0.15, 0.5, 0.85):
                    p = Vector((x, ya + (yb - ya) * t, z0 + (z1 - z0) * t))
                    s = 1 if x > mid else -1
                    inside.box((min(mid + s * 0.05, x), p.y - 0.01, p.z - 0.07), (max(mid + s * 0.05, x), p.y + 0.01, p.z - 0.02), steel)
        # ---- the toilets: tiled walls, basins and a mirror by the door, urinals or cubicles
        for side, a in (("boys", -35.5), ("girls", 31.5)):
            xa, xb = a + HT, a + WC - HT
            ry0 = YC + HT
            for (x0, x1, y0, y1) in ((xa, xa + 0.012, ry0, Y1), (xb - 0.012, xb, ry0, Y1), (xa, xb, Y1 - 0.012, Y1), (xa, xb, ry0, ry0 + 0.012)):
                inside.box((x0, y0, F0), (x1, y1, F0 + 1.4), wc_tile)
            for y in (ry0 + 0.9, ry0 + 1.6):
                fk.wall_basin(inside, (xb, y, F0), (-1, 0, 0), porcelain, grille)
                fk.swan_tap(inside, (xb - 0.06, y, F0 + 0.8), (-1, 0, 0), stainless, h=0.14, reach=0.12, r=0.008)
            inside.box((xb - 0.012, ry0 + 0.6, F0 + 1.05), (xb - 0.006, ry0 + 1.9, F0 + 1.75), mirror)
            # the cubicles along the south wall
            nc = 2 if side == "boys" else 3
            cw = (xb - xa) / nc
            cy0 = Y1 - 1.6
            for c in range(nc + 1):
                x = xa + c * cw
                if 0 < c < nc:
                    fk.rbox(inside, (x - 0.015, cy0, F0 + 0.15), (x + 0.015, Y1, F0 + 2.0), partition_m, r=0.008, seg=2)
            for c in range(nc):
                x0 = xa + c * cw
                fk.rbox(inside, (x0, cy0 - 0.015, F0 + 0.15), (x0 + 0.15, cy0 + 0.015, F0 + 2.0), partition_m, r=0.008, seg=2)
                fk.rbox(inside, (x0 + cw - 0.15, cy0 - 0.015, F0 + 0.15), (x0 + cw, cy0 + 0.015, F0 + 2.0), partition_m, r=0.008, seg=2)
                # the door, left ajar (swung 70 degrees into the cubicle)
                hinge = Vector((x0 + 0.15, cy0, F0 + 0.15))
                dw = cw - 0.3
                ang = math.radians(70)
                cen = hinge + Vector((math.cos(ang) * dw / 2, math.sin(ang) * dw / 2, 0.925))
                inside.oriented_box(cen, (dw, 0.03, 1.85), Matrix.Rotation(ang, 3, "Z"), partition_m)
                fk.wc(inside, (x0 + cw / 2, Y1 - 0.42, F0), (0, 1, 0), porcelain, white, grille)
                fk.tube(inside, [(x0 + 0.12, Y1 - 0.015, F0 + 0.75), (x0 + 0.12, Y1 - 0.12, F0 + 0.75)], 0.008, stainless, seg=8)   # the paper holder's arm
                fk.cyl(inside, (x0 + 0.06, Y1 - 0.12, F0 + 0.75), 0.055, 0.11, white, seg=20, axis=(1, 0, 0))                     # the roll
            if side == "boys":
                for y in (ry0 + 2.6, ry0 + 3.4, ry0 + 4.2):
                    fk.urinal(inside, (xa, y, F0), (1, 0, 0), porcelain, grille, stainless)
                    if y < ry0 + 4.0:
                        fk.rbox(inside, (xa, y + 0.37, F0 + 0.5), (xa + 0.45, y + 0.41, F0 + 1.4), partition_m, r=0.015)
            plate_x(a + 1.85, YC - HT - 0.03, YC - HT - 0.5, F0 + 2.08, F0 + 2.2, "男子トイレ" if side == "boys" else "女子トイレ")
