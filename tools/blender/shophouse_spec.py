"""The town's shophouses: the sizes and styles built (tools/blender/build_shophouse.py), and where
each one's signboard goes (tools/blender/build_shop_signs.py). Shared so a sign always lands on its
board.

Local frame (Blender), as the houses: origin at ground level in the middle of the footprint, +X
across the frontage, +Y the street (the shopfront), -Y the back (the plan's garden side).
"""

WIDTHS = (6.0, 7.0, 8.0, 9.0)   # frontage classes (the plan's shophouses are 6.1 - 9.2 m wide)
DEPTH = 11.0                    # all of them about 11 m deep (10.5 - 13)
STYLES = ("k", "t")             # "k": kanban kenchiku (a flat metal front to a parapet); "t":
                                # traditional (timber front, a tiled pent roof, lattice windows)
F0 = 0.15                       # the shop floor, a step above the street
F1 = 3.3                        # the upper floor
TOP = 6.0                       # the side walls' top (eaves)


def width_class(w):
    """The frontage class a plot's footprint width gets (the widest that fits, else the narrowest)."""
    fit = [c for c in WIDTHS if c <= w + 0.15]
    return max(fit) if fit else WIDTHS[0]


def style_of(house_id):
    """Which style a shophouse is (by its plan id; about a third traditional)."""
    return "t" if (house_id * 2654435761 >> 7) % 3 == 0 else "k"


def asset_id(w, style):
    return f"shophouse_{style}{int(w)}"


def board(w, style):
    """The signboard's face: (x0, x1, y, z0, z1) in the local frame. 8:1, the frontage less 0.5 m."""
    bw = w - 0.5
    bh = bw / 8.0
    if style == "k":
        z0 = 2.95                   # over the shopfront's shutter box
        y = DEPTH / 2 + 0.115
    else:
        z0 = 3.42                   # on the front above the pent roof
        y = DEPTH / 2 + 0.115
    return (-bw / 2, bw / 2, y, z0, z0 + bh)
