// Controller button drawings, as SVG files: an Xbox controller's A B X Y in their colours, its
// bumpers, triggers, sticks, d-pad, View and Menu; a PlayStation's cross circle square
// triangle, L1 to R3, Create and Options; the mouse. The keyboard's keys are drawn by the
// game itself (crates/game/src/draw.rs, keycaps).
//
// The drawings are Rust4x4's (D:/AussieOutback4x4/engine/ui/ui.js, "Buttons"), with its
// classes (ui.css) resolved to plain colours so they render anywhere.
//
//     node tools/buttons/make_button_svgs.mjs
//     cargo run --release -p buttons            (rasterises them: assets/ui/buttons/*.png)
//
// -> tools/buttons/svg/<name>.svg, names: <kit>_<control> (kit xbox | playstation), dpad_<ways>,
//    mouse_<button>

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const OUT = join(dirname(fileURLToPath(import.meta.url)), "svg");
const INK = "#17120f", BONE = "#f2eadb", EDGE = "rgba(242,234,219,0.7)";
const STYLE = `<style>
.g-b { fill: ${INK}; stroke: ${EDGE}; stroke-width: 1.6; stroke-linejoin: round; }
.g-f { fill: ${BONE}; }
.g-c { fill: rgba(242,234,219,0.17); stroke: ${EDGE}; stroke-width: 1.2; }
.g-l { fill: none; stroke: ${EDGE}; stroke-width: 1.4; }
.g-s { fill: none; stroke: ${BONE}; stroke-width: 2.3; stroke-linecap: round; stroke-linejoin: round; }
.g-t { fill: ${BONE}; font-family: Bahnschrift, 'Arial Narrow', Arial, sans-serif; font-weight: 700; text-anchor: middle; dominant-baseline: central; }
</style>`;

// (each is 32 high and as wide as it needs; a margin of 2 round it for the strokes)
const gSvg = (width, name, inner) =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="-2 -2 ${width + 4} 36" width="${width + 4}" height="36"><title>${name}</title>${STYLE}${inner.replaceAll("var(--g-ink)", INK)}</svg>`;
const gText = (x, y, size, text, more = "") => `<text class="g-t" x="${x}" y="${y}" font-size="${size}"${more}>${text}</text>`;
const G_DISC = '<circle class="g-b" cx="16" cy="16" r="13.5"/>';
const G_FACE = {
  xbox: { south: ["A", "#6cc24a"], east: ["B", "#ea5a44"], west: ["X", "#4a9fee"], north: ["Y", "#f2b632"] },
  playstation: {
    south: ["Cross", "#86aef5", '<path d="M10.4 10.4 L21.6 21.6 M21.6 10.4 L10.4 21.6"/>'],
    east: ["Circle", "#f2756e", '<circle cx="16" cy="16" r="6.6"/>'],
    west: ["Square", "#e99fd3", '<rect x="10.2" y="10.2" width="11.6" height="11.6" rx="0.8"/>'],
    north: ["Triangle", "#55d3a4", '<path d="M16 8.8 L23.2 21.2 H8.8 Z"/>'],
  },
};
function gFace(kit, which) {
  const [name, colour, shape] = G_FACE[kit][which];
  if (shape) return gSvg(32, name, `${G_DISC}<g class="g-s" style="stroke:${colour}">${shape}</g>`);
  return gSvg(32, name, `<circle class="g-b" cx="16" cy="16" r="13.5" style="stroke:${colour}"/>${gText(16, 16.8, 17, name, ` style="fill:${colour}"`)}`);
}
const G_SHOULDER = { xbox: { l1: "LB", r1: "RB", l2: "LT", r2: "RT" }, playstation: { l1: "L1", r1: "R1", l2: "L2", r2: "R2" } };
function gShoulder(kit, which) {
  const name = G_SHOULDER[kit][which];
  if (which.endsWith("2")) return gSvg(34, name, `<path class="g-b" d="M8 29.5 L6 14.5 A11 11.5 0 0 1 28 14.5 L26 29.5 Z"/>${gText(17, 18.6, 11.5, name)}`);
  const turned = which === "r1" ? ' transform="matrix(-1 0 0 1 44 0)"' : "";
  return gSvg(44, name, `<path class="g-b" d="M4.5 24.5 V15.5 A8 8 0 0 1 12.5 7.5 H36 A3.5 3.5 0 0 1 39.5 11 V24.5 Z"${turned}/>${gText(22, 16.6, 11.5, name)}`);
}
const G_WAYS = { left: "M4.2 16 L7.3 13.1 V18.9 Z", right: "M27.8 16 L24.7 13.1 V18.9 Z", up: "M16 4.2 L13.1 7.3 H18.9 Z", down: "M16 27.8 L13.1 24.7 H18.9 Z" };
function gStick(kit, side, how) {
  const letter = side === "left" ? "L" : "R", pressed = how === "press";
  const label = pressed && kit === "playstation" ? `${letter}3` : letter;
  const name = `${side === "left" ? "Left" : "Right"} stick${pressed ? ", pressed" : ""}`;
  const arrows = how === "x" ? [G_WAYS.left, G_WAYS.right] : how === "y" ? [G_WAYS.up, G_WAYS.down] : pressed ? ["M16 7.6 L13.1 4.5 H18.9 Z"] : [];
  return gSvg(32, name, `${G_DISC}<circle class="${pressed ? "g-f" : "g-c"}" cx="16" cy="16" r="8"/>`
    + gText(16, 16.5, label.length > 1 ? 8.6 : 10.5, label, pressed ? ` style="fill:${INK}"` : "")
    + arrows.map((arrow) => `<path class="g-f" d="${arrow}"/>`).join(""));
}
const G_ARMS = { up: "M12.9 4.2 H19.1 V10.9 L16 14 L12.9 10.9 Z", down: "M12.9 27.8 H19.1 V21.1 L16 18 L12.9 21.1 Z",
  left: "M4.2 12.9 V19.1 H10.9 L14 16 L10.9 12.9 Z", right: "M27.8 12.9 V19.1 H21.1 L18 16 L21.1 12.9 Z" };
const gDpad = (ways) => gSvg(32, `D-pad ${ways.join(" and ")}`,
  '<path class="g-b" d="M11.5 2.5 H20.5 V11.5 H29.5 V20.5 H20.5 V29.5 H11.5 V20.5 H2.5 V11.5 H11.5 Z"/>'
  + ways.map((way) => `<path class="g-f" d="${G_ARMS[way]}"/>`).join(""));
const G_LINES = '<path class="g-s" d="M10.5 11.5 H21.5 M10.5 16 H21.5 M10.5 20.5 H21.5"/>';
const G_MIDDLE = {
  xbox: { select: ["View", `<rect class="g-s" x="9.5" y="9.5" width="9.5" height="8" rx="1"/><rect class="g-s" x="13" y="14.5" width="9.5" height="8" rx="1" style="fill:${INK}"/>`],
    start: ["Menu", G_LINES] },
  playstation: { select: ["Create", '<rect class="g-s" x="13" y="13.5" width="6" height="10.5" rx="3"/><path class="g-s" d="M16 6.8 V10 M10.6 8.6 L12.4 11.2 M21.4 8.6 L19.6 11.2"/>'],
    start: ["Options", G_LINES] },
};
const gMiddle = (kit, which) => gSvg(32, G_MIDDLE[kit][which][0], G_DISC + G_MIDDLE[kit][which][1]);
function gMouse(which) {
  const moved = which === "move", at = moved ? 9 : 0;
  const lit = { left: `<path class="g-f" d="M${at + 10.1} 3.5 V12.1 H${at + 4} V10.4 A7 7 0 0 1 ${at + 10.1} 3.5 Z"/>`,
    right: `<path class="g-f" d="M${at + 11.9} 3.5 V12.1 H${at + 18} V10.4 A7 7 0 0 0 ${at + 11.9} 3.5 Z"/>`,
    middle: `<rect class="g-f" x="${at + 9.2}" y="4.8" width="3.6" height="7" rx="1.8" style="stroke:${INK};stroke-width:1.2"/>`,
    move: '<path class="g-s" d="M6 11.6 L2 16 L6 20.4 M34 11.6 L38 16 L34 20.4"/>' }[which];
  const name = moved ? "Move the mouse" : `${which[0].toUpperCase()}${which.slice(1)} mouse button`;
  return gSvg(moved ? 40 : 22, name, `<rect class="g-b" x="${at + 3}" y="2.5" width="16" height="27" rx="8"/>`
    + `<path class="g-l" d="M${at + 11} 2.5 V13 M${at + 3} 13 H${at + 19}"/>${lit}`);
}

mkdirSync(OUT, { recursive: true });
const files = {};
for (const kit of ["xbox", "playstation"]) {
  for (const w of ["south", "east", "west", "north"]) files[`${kit}_${w}`] = gFace(kit, w);
  for (const w of ["l1", "r1", "l2", "r2"]) files[`${kit}_${w}`] = gShoulder(kit, w);
  for (const side of ["left", "right"]) {
    files[`${kit}_stick_${side}`] = gStick(kit, side, "");
    files[`${kit}_stick_${side}_x`] = gStick(kit, side, "x");
    files[`${kit}_stick_${side}_y`] = gStick(kit, side, "y");
  }
  files[`${kit}_l3`] = gStick(kit, "left", "press");
  files[`${kit}_r3`] = gStick(kit, "right", "press");
  files[`${kit}_select`] = gMiddle(kit, "select");
  files[`${kit}_start`] = gMiddle(kit, "start");
}
for (const ways of [["up"], ["down"], ["left"], ["right"], ["up", "down"], ["left", "right"], ["up", "down", "left", "right"]]) {
  files[`dpad_${ways.join("_")}`] = gDpad(ways);
}
for (const m of ["left", "right", "middle", "move"]) files[`mouse_${m}`] = gMouse(m);
for (const [name, svg] of Object.entries(files)) writeFileSync(join(OUT, `${name}.svg`), svg);
console.log(`wrote ${Object.keys(files).length} drawings to ${OUT}`);
