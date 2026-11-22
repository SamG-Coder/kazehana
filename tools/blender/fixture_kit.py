"""Shapes for fixtures (taps, basins, troughs, WCs, urinals, hobs, gas cocks, benches) built into
a kz_common.MeshBuilder's bmesh, smooth-shaded: swept tubes, lofted bowls and shells, rounded
boxes. Used by tools/blender/school_fitout.py.

Frames are Blender's (z up), metres. `b` is a MeshBuilder; every function adds to b.bm and
assigns the material through b._mat_index.
"""

import math

import bmesh
from mathutils import Matrix, Vector

UP = Vector((0, 0, 1))


def _mat(b, faces, mat, smooth=True):
    i = b._mat_index(mat)
    for f in faces:
        f.material_index = i
        f.smooth = smooth


def tube(b, pts, r, mat, seg=12, caps=True, closed=False):
    """A round tube of radius r along a polyline (a pipe, a spout, a handle), its frames carried
    along without twisting."""
    pts = [Vector(p) for p in pts]
    if closed:
        pts = pts + [pts[0]]
    t0 = (pts[1] - pts[0]).normalized()
    n = t0.orthogonal().normalized()
    rings = []
    for i, p in enumerate(pts):
        if i == 0:
            t = (pts[1] - pts[0]).normalized()
        elif i == len(pts) - 1:
            t = (pts[-1] - pts[-2]).normalized() if not closed else ((pts[1] - pts[0]).normalized() + (pts[-1] - pts[-2]).normalized()).normalized()
        else:
            t = ((pts[i + 1] - p).normalized() + (p - pts[i - 1]).normalized()).normalized()
        n = (n - t * n.dot(t)).normalized()
        bv = t.cross(n)
        rings.append([b.bm.verts.new(p + (n * math.cos(a) + bv * math.sin(a)) * r) for a in (2 * math.pi * k / seg for k in range(seg))])
    faces = []
    for r0, r1 in zip(rings, rings[1:]):
        for k in range(seg):
            faces.append(b.bm.faces.new((r0[k], r0[(k + 1) % seg], r1[(k + 1) % seg], r1[k])))
    if caps and not closed:
        faces.append(b.bm.faces.new(rings[0][::-1]))
        faces.append(b.bm.faces.new(rings[-1]))
    _mat(b, faces, mat)


def arc(c, u, v, r, a0, a1, n=8):
    """Points on a circular arc: c + r (u cos a + v sin a), a from a0 to a1."""
    c, u, v = Vector(c), Vector(u), Vector(v)
    return [c + (u * math.cos(a0 + (a1 - a0) * k / n) + v * math.sin(a0 + (a1 - a0) * k / n)) * r for k in range(n + 1)]


def ring(c, rx, ry, e=2.0, n=24, a0=0.0, a1=2 * math.pi, closed=True, mod=None):
    """A superellipse ring in a horizontal plane (e = 2 an ellipse, larger towards a rounded
    rectangle), angles from +X anticlockwise seen from above; `mod(a) -> scale` dents it."""
    out = []
    count = n if closed else n + 1
    for k in range(count):
        a = a0 + (a1 - a0) * k / n
        ca, sa = math.cos(a), math.sin(a)
        x = math.copysign(abs(ca) ** (2 / e), ca) * rx
        y = math.copysign(abs(sa) ** (2 / e), sa) * ry
        s = mod(a) if mod else 1.0
        out.append(Vector(c) + Vector((x * s, y * s, 0)))
    return out


def loft(b, rings_pts, mat, closed=True, outward=False, cap_last=False, cap_first=False):
    """Faces between rings of points (each the same count, top to bottom, anticlockwise seen
    from above): facing inwards (a bowl's inside) or outwards (a shell)."""
    rows = [[b.bm.verts.new(p) for p in r] for r in rings_pts]
    m = len(rows[0])
    faces = []
    span = m if closed else m - 1
    for r0, r1 in zip(rows, rows[1:]):
        for k in range(span):
            q = (r0[k], r0[(k + 1) % m], r1[(k + 1) % m], r1[k])
            faces.append(b.bm.faces.new(q[::-1] if outward else q))
    if cap_last:
        last = rows[-1]
        faces.append(b.bm.faces.new(last[::-1] if outward else last))
    if cap_first:
        first = rows[0]
        faces.append(b.bm.faces.new(first if outward else first[::-1]))
    _mat(b, faces, mat)
    return rows


def rbox(b, mn, mx, mat, r=0.01, seg=3):
    """A box with rounded edges (a worktop, a cabinet, a cistern)."""
    mn, mx = Vector(mn), Vector(mx)
    res = bmesh.ops.create_cube(b.bm, size=1.0)
    verts = res["verts"]
    bmesh.ops.transform(b.bm, matrix=Matrix.Translation((mn + mx) / 2) @ Matrix.Diagonal((*(mx - mn), 1.0)), verts=verts)
    edges = list({e for v in verts for e in v.link_edges})
    out = bmesh.ops.bevel(b.bm, geom=edges, offset=min(r, min(mx - mn) * 0.45), segments=seg, profile=0.5, affect="EDGES")
    faces = list({f for v in list(verts) + list(out["verts"]) if v.is_valid for f in v.link_faces})
    _mat(b, faces, mat)


def cyl(b, c, r, h, mat, seg=24, axis=UP):
    """A cylinder from c along `axis` for h (a knob, a post, a drain)."""
    c, axis = Vector(c), Vector(axis).normalized()
    tube(b, [c, c + axis * h], r, mat, seg=seg)


def basin_bowl(b, c, w, d, depth, mat, e_top=6.0, e_bottom=3.0, flange=0.02, n=40, a0=0.0, a1=2 * math.pi, closed=True):
    """A sunken bowl whose rim is at c (its top): a flat flange round the opening, the bowl's
    inside curving down to a flat bottom (its faces looking in), a drain."""
    rings = [ring(c, w / 2 + flange, d / 2 + flange, e_top, n, a0, a1, closed), ring(c, w / 2, d / 2, e_top, n, a0, a1, closed)]
    for k in range(1, 7):
        t = k / 6
        z = depth * (1 - math.cos(t * math.pi / 2))
        s = 1 - 0.35 * t ** 1.5
        e = e_top + (e_bottom - e_top) * t
        rings.append(ring(Vector(c) - UP * z, w / 2 * s, d / 2 * s, e, n, a0, a1, closed))
    loft(b, rings, mat, closed=closed, cap_last=closed)
    return rings


def swan_tap(b, base, toward, mat, h=0.22, reach=0.16, r=0.011):
    """A gooseneck tap on a worktop at `base`, its spout curving over towards `toward`; a lever."""
    base, toward = Vector(base), Vector(toward).normalized()
    cyl(b, base, 0.026, 0.012, mat, seg=20)                                   # escutcheon
    cyl(b, base, 0.018, 0.07, mat, seg=16)                                    # body
    top = base + UP * h
    pts = [base + UP * 0.06, base + UP * (h - reach / 2)]
    pts += arc(top - UP * (reach / 2) + toward * (reach / 2), -toward, UP, reach / 2, 0.0, math.pi, n=10)[1:]
    pts += [base + toward * reach + UP * (h - reach / 2 - 0.05)]
    tube(b, pts, r, mat, seg=12)
    cyl(b, pts[-1] + UP * 0.002, r + 0.003, -0.025, mat, seg=12)              # aerator
    side = toward.cross(UP)
    tube(b, [base + UP * 0.05 + side * 0.012, base + UP * 0.085 + side * 0.04 - toward * 0.05], 0.006, mat, seg=8)   # lever


def bib_tap(b, wall_pt, out, mat, handle_mat=None):
    """A wall tap over a trough (the corridor's): a short arm out of the pipe, a body, the spout
    turning down, a round cross-handle on top."""
    p, out = Vector(wall_pt), Vector(out).normalized()
    tube(b, [p, p + out * 0.06], 0.012, mat, seg=12)
    body = p + out * 0.08
    tube(b, [body - out * 0.025, body + out * 0.025], 0.02, mat, seg=16)
    tube(b, [body + out * 0.02, body + out * 0.07, body + out * 0.085 - UP * 0.015, body + out * 0.088 - UP * 0.05], 0.009, mat, seg=10)
    cyl(b, body + UP * 0.02, 0.007, 0.03, mat, seg=8)
    hm = handle_mat or mat
    side = out.cross(UP)
    for v in (out, side):
        tube(b, [body + UP * 0.05 - v * 0.03, body + UP * 0.05 + v * 0.03], 0.006, hm, seg=8)
    cyl(b, body + UP * 0.045, 0.012, 0.012, hm, seg=12)


def gas_cock(b, base, out, mat, lever_mat):
    """A bench gas cock: a body on its plate, a hose nozzle out to the side, a lever."""
    base, out = Vector(base), Vector(out).normalized()
    cyl(b, base, 0.025, 0.006, mat, seg=16)
    cyl(b, base, 0.014, 0.06, mat, seg=12)
    tube(b, [base + UP * 0.04, base + UP * 0.04 + out * 0.06], 0.007, mat, seg=8)
    for k in range(3):
        cyl(b, base + UP * 0.04 + out * (0.03 + k * 0.012) - out * 0.003, 0.009, 0.006, mat, seg=8, axis=out)
    tube(b, [base + UP * 0.062, base + UP * 0.07 - out.cross(UP) * 0.05], 0.006, lever_mat, seg=8)


def burner(b, c, mat, knob_mat, r=0.1):
    """A gas hob burner: the pan support's ring and spokes over the burner cap."""
    c = Vector(c)
    cyl(b, c, r * 0.45, 0.02, mat, seg=20)
    tube(b, ring(c + UP * 0.035, r, r, 2.0, 20), 0.006, mat, seg=6, closed=True)
    for k in range(4):
        a = math.pi / 4 + k * math.pi / 2
        d = Vector((math.cos(a), math.sin(a), 0))
        tube(b, [c + d * r * 0.5 + UP * 0.035, c + d * r * 1.2 + UP * 0.035], 0.005, mat, seg=6)


def wc(b, c, back, mat, seat_mat, dark):
    """A WC: the pedestal and bowl as one moulded shell, its inside, the seat, the lid up against
    the cistern, the cistern and its button. c: the bowl's centre on the floor; back: towards the
    wall."""
    c, back = Vector(c), Vector(back).normalized()
    side = back.cross(UP)
    ang = math.atan2(back.y, back.x) + math.pi            # the bowl's front direction as an angle

    def rot(pts):
        R = Matrix.Rotation(ang, 3, "Z")
        return [c + R @ (p - c) for p in pts]
    prof = [(0.40, 0.19, 0.24, 2.3), (0.37, 0.185, 0.235, 2.3), (0.28, 0.16, 0.2, 2.2), (0.16, 0.13, 0.17, 2.4), (0.03, 0.12, 0.16, 2.6), (0.0, 0.125, 0.165, 2.6)]
    rings = [rot(ring(c + UP * z, rx, ry, e, 32)) for (z, ry, rx, e) in prof]
    loft(b, rings, mat, outward=True)
    inner = [rot(ring(c + UP * 0.40, 0.165, 0.215, 2.3, 32)), rot(ring(c + UP * 0.33, 0.13, 0.17, 2.2, 32)), rot(ring(c + UP * 0.24, 0.07, 0.09, 2.0, 32)), rot(ring(c + UP * 0.2, 0.05, 0.06, 2.0, 32))]
    loft(b, [rings[0]] + inner, mat, cap_last=False)
    loft(b, [inner[-1], rot(ring(c + UP * 0.199, 0.0, 0.0, 2.0, 32))], dark)
    seat = rot(ring(c + UP * 0.415, 0.17, 0.22, 2.3, 28))
    tube(b, seat, 0.018, seat_mat, seg=8, closed=True)
    cistern = c + back * 0.33 + UP * 0.6
    rbox(b, cistern - side * 0.2 - back * 0.1 - UP * 0.2, cistern + side * 0.2 + back * 0.1 + UP * 0.2, mat, r=0.02)
    rbox(b, cistern - side * 0.19 - back * 0.13 - UP * 0.17, cistern + side * 0.19 - back * 0.1 + UP * 0.25, seat_mat, r=0.015)   # the lid, up
    cyl(b, cistern + UP * 0.2, 0.02, 0.008, seat_mat, seg=12)


def urinal(b, wall_pt, out, mat, dark, steel):
    """A wall-hung urinal: a moulded shell out of the wall, dished in at the front (its basin),
    a drain at the bottom of the dish, the flush pipe and valve above."""
    p, out = Vector(wall_pt), Vector(out).normalized()
    ang = math.atan2(out.y, out.x)
    R = Matrix.Rotation(ang, 3, "Z")
    prof = [(1.12, 0.10, 0.15, 0.0), (1.05, 0.22, 0.18, 0.15), (0.9, 0.32, 0.2, 0.5), (0.72, 0.36, 0.2, 0.7), (0.56, 0.34, 0.18, 0.55), (0.46, 0.27, 0.15, 0.2), (0.4, 0.14, 0.09, 0.0)]
    rings = []
    for (z, rx, ry, dish) in prof:
        def mod(a, dish=dish):
            f = max(0.0, math.cos(a)) ** 4
            return 1.0 - dish * 0.55 * f
        pts = ring(Vector((0, 0, 0)), rx, ry, 2.4, 24, -math.pi / 2, math.pi / 2, closed=False, mod=mod)
        rings.append([p + R @ q + UP * z for q in pts])
    loft(b, rings, mat, closed=False, outward=True)
    cyl(b, p + R @ Vector((0.14, 0, 0)) + UP * 0.47, 0.03, 0.004, dark, seg=12)
    tube(b, [p + R @ Vector((0.03, 0, 0)) + UP * 1.12, p + R @ Vector((0.03, 0, 0)) + UP * 1.38, p + R @ Vector((0.05, 0, 0)) + UP * 1.42], 0.012, steel, seg=10)
    cyl(b, p + R @ Vector((0.06, 0, 0)) + UP * 1.38, 0.025, 0.08, steel, seg=14)


def wall_basin(b, wall_pt, out, mat, dark, w=0.5, d=0.4, h=0.8):
    """A wall-hung wash basin, half round out of the wall: its outside, rim and bowl, a drain, the
    pedestal trap cover under it."""
    p, out = Vector(wall_pt), Vector(out).normalized()
    ang = math.atan2(out.y, out.x)
    R = Matrix.Rotation(ang, 3, "Z")

    def half(z, rx, ry, e=2.6):
        return [p + R @ q + UP * z for q in ring(Vector((0, 0, 0)), rx, ry, e, 24, -math.pi / 2, math.pi / 2, closed=False)]
    outside = [half(h, d, w / 2), half(h - 0.03, d - 0.005, w / 2 - 0.005), half(h - 0.12, d - 0.05, w / 2 - 0.04), half(h - 0.18, d - 0.14, w / 2 - 0.1)]
    loft(b, outside, mat, closed=False, outward=True)
    inside = [half(h, d - 0.04, w / 2 - 0.04), half(h - 0.06, d - 0.08, w / 2 - 0.08), half(h - 0.11, d - 0.15, w / 2 - 0.13), half(h - 0.13, d - 0.2, w / 2 - 0.17)]
    loft(b, [outside[0]] + inside, mat, closed=False)
    cyl(b, p + R @ Vector((d * 0.45, 0, 0)) + UP * (h - 0.129), 0.025, 0.002, dark, seg=12)
    tube(b, [p + R @ Vector((0.12, 0, 0)) + UP * 0.0, p + R @ Vector((0.12, 0, 0)) + UP * (h - 0.18)], 0.06, mat, seg=16)
