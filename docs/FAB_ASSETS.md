# Fab shopping list: Path to School

Download these in **glTF** at the same quality tier as the existing library (`gltf/mid` or high). They should land in `D:/EpicVault/VaultCache/FabLibrary` like the others. Every item goes through the quality gate (`docs/ASSET_QUALITY.md`) before the level uses it.

**Priority:** ★ = needed for the first pass of the level, ○ = nice to have.

## Status (2026-11-21)

**Downloaded:** Claude downloaded these through the Epic launcher: free Quixel Megascans assets as glTF Medium, into `D:/EpicVault/VaultCache/FabLibrary`.

- Surfaces:
  - Asphalt Road
  - Precast Concrete Wall
  - Japanese Roof Tiles
  - Wood Planks
  - Wooden Planks (dark)
  - Mossy Stone Wall
  - Japanese Shrine Stone Floor
- Models:
  - Modular Shrine Roof Top
  - Japanese Shrine Stairs
  - Japanese Stone Lantern
  - Japanese Komainu Statue
  - Japanese Mailbox (red post box)
  - Japanese Wooden Mailbox
  - Urban Japanese Street Box Metal
  - Urban Japanese Street Curb Concrete
  - Urban Japanese Street Block Concrete (gutter cover)
  - Urban Japanese Street Bollard Metal

**Free, for later:** Quixel's "Urban Japanese" collection has much more for the School level:
- desks, chairs, blackboard, bookshelves, piano, stove, windows, signs
- railings, hydrants, barriers, benches

**Not free — you decide:** Megascans has no vending machines or utility poles. Those need one of the third-party packs below (the Japanese Street Props Pack and the Utility Poles Modular Pack). I didn't buy anything.

## Surfaces (Quixel Megascans, free)

These texture the pieces I build in Blender: road, walls, roofs, the shrine.

- ★ [Asphalt Road (2×2 m, cracked / faded)](https://www.fab.com/listings/73d9acb3-e8b3-420e-9ad4-912affac49af) — the road
- ○ [Asphalt Road (close-up, 0.42 m)](https://www.fab.com/listings/ab56bd64-6bd4-4882-8a02-86826355a8dd) — detail variant
- ★ [Precast Concrete Wall (tagged Japan, roadside)](https://www.fab.com/listings/7d5ed180-d5aa-4d73-bee9-20c6d2dc7d52) — block and retaining walls along the road
- ★ [Japanese Roof Tiles (clay, weathered)](https://www.fab.com/listings/43bc2fbe-ff7e-419f-813c-b7b7f84eb249) — house roofs
- ★ [Wood Planks](https://www.fab.com/listings/2f3201b4-93a0-4c86-a154-ee31e35b2781) — house siding, the shrine
- ★ [Mossy Stone Wall (Japan)](https://www.fab.com/listings/5a4993d7-573c-4ca6-9d9d-9c11383b14f6) — stone walls and the shrine base
- ○ [Japanese Shrine Stone Floor](https://www.fab.com/listings/a79ac890-8596-4a0d-aaf7-e3152b836b59) — the shrine approach

Already downloaded and usable: Wet Grass Ground, Soil Mud (paddy mud), Ploughed Soil, Pebbles Round, Mossy Riverbed, Sparse Grassy Soil.

## 3D models (Quixel Megascans, free)

- ★ [Japanese Shrine Stairs](https://www.fab.com/listings/d808f867-2560-4476-ad05-a1d999bd940b) — up to the Inari shrine
- ○ [Japanese Stone Wood Base](https://www.fab.com/listings/61d6a82a-f1a2-4a6f-b75b-fde404c261dd) — wall trim
- ○ [Japanese Wooden Mailbox](https://www.fab.com/listings/db69621d-02cd-4b02-ad5a-46acd8c583ca) — outside houses
- ○ [Japanese Arched Wooden Bridge](https://www.fab.com/listings/b52a182a-d18b-4287-858e-e7c140483ae9) — a footbridge by the shrine

## Street props (third-party: check the price and licence first)

These cover what Megascans doesn't: vending machines, utility poles, signs, torii. I picked realistic, real-scale packs. Each still has to pass the gate; anything stylised gets rejected.

- ★ [Japanese Street Props Pack (33 assets, real scale, PBR)](https://www.fab.com/listings/363d4607-94ad-4d01-9e83-9613896f581a) — vending machines and street clutter
- ★ [Japanese Utility Poles Modular Pack (6 poles + 30 parts, 4K)](https://www.fab.com/listings/3d121166-a8c6-4b48-8e42-132e92256d40) — the poles and wires down the road
- ○ [Japan Street Prop Collection (30+ props)](https://www.fab.com/listings/3321c16a-9efc-4c4d-b7ce-3db0daae8056) — an alternative or extra to the pack above
- ○ [Japanese Street Signs Pack (109 signs)](https://www.fab.com/listings/516eae83-9ecf-404f-9eaf-21a338185919) — road signs, the level-crossing sign
- ★ [Traditional Japanese folk house + Torii + Interior](https://www.fab.com/listings/7a35f330-da32-438d-8078-3c645d727a84) — torii and old-house parts for the shrine
  - or [Japanese Shrine Pack (modular)](https://www.fab.com/listings/f6a1185b-fe06-4cc3-bc60-84fdfdba0231)
- ○ [Kawara Roof Collection](https://www.fab.com/listings/17fe6fb4-3460-4cd8-8ea9-1d4328e86756) — proper ridge and edge tiles

## What I build in Blender

Built by scripts in `tools/blender/` and textured with the surfaces above. Each is exported as its own asset and goes through the gate.

- Rural two-storey houses: plaster and wood siding, kawara roofs, sliding windows, entrance canopies
- The road: lanes, white edge lines, concrete gutters with grates, kerbs
- Concrete block walls and hedges along the plots
- Rice paddies: water, mud, levees, rows of young rice for April
- The river bridge with railings, and the level crossing (rails, sleepers, barrier arms, crossing signal)
- The konbini building shell
- Torii and the shrine hall, if the third-party packs don't pass the gate

## Still needed

- **Japanese cedar (sugi) and bamboo with foliage.** The cedar trunk we have has no foliage. Bamboo is a Megaplant, which needs the Unreal export route SmartImposter used.
- **Rice plants.** There's no Megascans rice. I'll build young rice in Blender from grass-card textures.
