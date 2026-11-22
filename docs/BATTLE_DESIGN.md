# Mythbeasts Duel: battle rules (v1)

A 1v1 monster card battle for players around 12. It takes the parts of Yu-Gi-Oh!, Pokémon and
Hearthstone that kids love and learn fastest, and drops the bookkeeping. A game lasts about 10-15
minutes.

The rules engine is `crates/tcg/src/battle/`. The 3D test (two players at the shop's play table) is
`tools/duel`.

## What we took, and why

| From | Idea | Why it fits a 12-year-old |
|---|---|---|
| **Yu-Gi-Oh! Speed Duel** | A small field (3 monster zones and 2 trap zones each), a 20-card deck, a 4-card opening hand | Short games, quick deck building, a table that is easy to read |
| **Yu-Gi-Oh!** | **Traps**: set face down, they spring when the opponent attacks | Bluffing and "gotcha!" moments |
| **Yu-Gi-Oh!** | Monsters **protect your Leader**: it can only be attacked directly when you have no monsters, or by Flying monsters | One clear rule that makes the board matter |
| **Pokémon** | **Types** in one simple cycle, shown as icons; a hit on a type you beat does **+2** ("SUPER!") | Pokémon's most loved learnable depth; one chart to remember |
| **Pokémon** | **Evolution** becomes **Ascend**: a Rare or Mythic is played on top of one of your monsters | The big moment kids wait for; it also stands in for tribute summoning |
| **Hearthstone** | **Spark**, a resource that refills and grows by 1 a turn (to 6) | No energy attaching, no mana screw |
| **Hearthstone** | Combat where **both monsters deal damage**, damage that stays, **Guard** (taunt) | Trading monsters is the core decision; the maths is a single subtraction |
| **Your Leader** | 20 HP; bring the enemy Leader to 0 to win | One goal, shown big on the HUD |

**Left out on purpose:**
- life points in the thousands;
- chains and priority;
- multiple battle phases;
- ATK/DEF positions;
- energy attachment and retreat costs;
- the graveyard as a resource;
- 60-card decks.

## The field (per player)

```
   [ Monster 1 ] [ Monster 2 ] [ Monster 3 ]        <- facing the opponent
 [Discard]  [ Trap ]  [Leader]  [ Trap ]  [Deck]
                (your hand along your table edge)
```

## Cards

- **Monster**: cost (Spark), ATK, HP, a type, and at most one keyword. Rares can Ascend, and Mythics
  *must* Ascend.
- **Spell**: cast from the hand, takes effect immediately. A few spells are **Traps**: you set them
  face down in a trap/relic zone, and they spring on the opponent's next attack.
- **Relic**: equipped to one of your monsters. It gives +ATK/+HP and sometimes a keyword, and is
  discarded when that monster leaves.

### Keywords (one per card, each with an icon)

| Keyword | Rule |
|---|---|
| **Swift** | Can attack on the turn it is summoned |
| **Guard** | While you have a Guard monster, enemies must attack a Guard first |
| **Flying** | Can attack the enemy Leader even when the enemy has monsters (unless a Guard is there) |
| **Pierce** | When it knocks out a monster, the leftover damage hits that monster's Leader |

## Types

Water douses fire, fire burns roots, stone stops wind, lightning shocks water:

```
Tide ──▶ Flame ──▶ Terra ──▶ Gale ──▶ Tide        Radiant ◀──▶ Umbral
```

A hit on a type yours beats does **+2 damage**. This counts in both directions of a fight: a Tide
monster attacking a Flame monster deals +2, and so would a Tide monster striking back. Nothing ever
does less, so there is no resistance to remember.

## A turn

1. **Start.**
   - Your maximum Spark goes up by 1 (to at most 6), and your Spark refills.
   - Your monsters wake up (they can attack again).
   - Draw a card. The first player skips this draw on turn 1.
   - You may hold at most 7 cards. Running out of deck is harmless: you just stop drawing.
2. **Main**, in any order, as long as you have Spark:
   - **Summon** a monster into an empty monster zone. It can't attack this turn (unless Swift).
   - **Ascend**: play a Rare or Mythic on top of one of your monsters. It takes that zone, keeps
     the monster's "already attacked" state, and the old monster is discarded.
   - **Cast** a spell, **set** a trap, or **equip** a relic.
   - **Attack** with each awake monster, once each:
     - Pick an enemy monster. It must be a Guard if they have one.
     - Or pick the enemy Leader, if they have no monsters (or your monster is Flying and there is no
       Guard).
     - Both monsters deal their ATK to each other, +2 for SUPER. Monsters at 0 HP are knocked out
       and go to the discard pile.
     - A Leader doesn't strike back.
     - If a face-down trap of the defender's can respond, it springs first.
3. **End turn.**

## Card stats

Cards in `crates/tcg` have no battle numbers in their data. `tcg::battle::stats()` derives them
deterministically from rarity, kind, attribute and card number, so every existing and future card is
playable:

| Rarity | Monster cost | ATK + HP | Keyword |
|---|---|---|---|
| Common | 1-2 | 3-5 | sometimes |
| Uncommon | 2-3 | 6-7 | often |
| Rare | 4 (or Ascend for 3) | 9 | always |
| Mythic | Ascend for 5 | 13 | always |

Each type has a stat lean (Flame hits harder, Terra is tougher, and so on) and a favourite keyword.

### Spells and traps by type

| Type | Spell | Trap (springs on an enemy attack) |
|---|---|---|
| Flame | Deal damage to a monster or Leader | Burn the attacker |
| Tide | Heal a monster or your Leader | Negate the attack |
| Gale | A monster gets +ATK this turn and can attack now | Send the attacker back to its owner's hand |
| Terra | A monster gets +HP for good | Your defender gets +HP before the hit |
| Radiant | Draw cards | Negate the attack and heal your Leader |
| Umbral | Send an enemy monster back to its owner's hand | Weaken the attacker for good |

Relics give +ATK/+HP by rarity. Some types also grant a keyword: Terra gives Guard, Gale gives
Flying and Flame gives Pierce.

## AI

`tcg::battle::ai` plays a whole turn one action at a time. It tries every legal action on a copy of
the game, scores the result, and keeps the best:
- It never sees your hand or your face-down traps.
- **Easy** adds noise to its scores.
- **Normal** scores the board as it stands.
- **Hard** also looks ahead at your best reply attacks.

`cargo test -p tcg battle` runs AI-vs-AI games and checks the balance: the first-player win rate,
the average game length, and the type win rates.

## Balance (AI vs AI, `balance_report`)

Results from 900 Normal-vs-Normal games with the set-1 starters:

| Measure | Result |
|---|---|
| First player wins | 54% |
| Average game length | about 20 turns (10 each) |
| Draws | about 1% (turn limit) |

Win rates when the row deck plays first (the rest are draws):

| Row deck vs | Blaze | Torrent | Quake |
|---|---|---|---|
| **Blaze** | 46% | 28% | 63% |
| **Torrent** | 64% | 49% | 46% |
| **Quake** | 40% | 52% | 49% |

The types make the starters counter one another, as in Pokémon: Torrent's Tide beats Blaze's Flame,
and Blaze beats Quake. Hard beats Easy in about two games out of three.

To re-run the report:
`cargo test -p tcg --release balance_report -- --ignored --nocapture`.

## The 1v1 test (`tools/duel`)

`cargo run --release` in `tools/duel`. Options:
- `--deck Blaze|Torrent|Quake`
- `--ai-deck …`
- `--level easy|normal|hard`
- `--seed N`
- `--autoplay`: AI vs AI in a hidden window; stills go to `assets/cards/duel_test/`.

**The scene is all physical.** You sit at the west play table inside Tsukumodō: the shop glTF, plus
its props placed from `layout.json` (the oak `shop_battle_table`, the pipe chairs and so on).
- A 640 x 500 mm neoprene playmat lies on the table (`tools/cardgen --playmat`, in
  `assets/cards/playmat/`). It shows the zones and a type wheel.
- Every card is a real 63 x 88 mm card slab using its cardgen face (`tools/cardgen --set 1`
  renders the whole season).
- Your hand lies face up on the table in front of the mat. The card you point at lifts and tips
  towards you.
- The opponent's hand lies face down on their side.
- Decks are real stacks, discards real piles, and each Leader zone holds a card-back emblem.
- Cards glide between places in an arc: from the deck to the hand, from the hand to a zone, and
  from a zone to the discard pile.
- Attacks lunge at the target. Traps flip face up before they spring. Spells hold up over the mat,
  then go to the discard pile.
- Object motion vectors keep all of this sharp under DLSS.

**How to play.** It plays with a mouse or with a controller (crates/play, as in the main game).

- **Mouse.**
  - Drag a hand card onto a lit spot. Over a spot it can go, it settles just above it; drop it anywhere else and it goes back.
  - Drag one of your monsters onto a red target to attack (a dotted arrow follows the cursor).
  - Clicking a card, then a target, works too.
  - Right click cancels, or looks around while held. The wheel leans in.
- **Controller** (Xbox or PlayStation):
  - The d-pad or a flick of the left stick moves the focus: left and right choose, up and down switch between your hand and your monsters.
  - A / Cross picks up and plays, B / Circle puts it back, Y / Triangle ends the turn.
  - The right stick looks around. Start begins a new duel after a game; Select hides the hints.
  - The hints switch to the controller's own buttons as soon as it's used, and back when the mouse or keyboard is used.
- **Seeing what you can do:**
  - On your turn, cards you can't play are greyed out. Playable ones have a gold glow on the table.
  - Picking something up lights where it can go on the mat: green for your spots, red for enemies, white-gold for the one you're pointing at.
  - Monsters ready to attack glow soft gold.
  - Hover a card to read its rules in the panel on the right.
- **Attacks:** the monster rears up, dashes in, strikes with a burst of sparks in its type's colour (more, plus white, on a SUPER hit), and settles back. The target shakes and the camera jolts on big hits. Summons and Ascends burst too.

**HUD.**
- ATK/HP chips at each monster, with their keyword and ZZZ while summoning-sick.
- Leader HP plates.
- Spark gems.
- A log, banners (YOUR TURN, TRAP!, ASCEND!, YOU WIN) and floating damage (SUPER! on type hits).
