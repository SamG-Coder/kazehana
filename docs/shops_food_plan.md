# Shops and food: buying and eating in the world

How you get food and things in town without menus (`docs/life_plan.md`). The rules (prices, seasons, what each place sells, buying and eating) are in `sim::items`, `sim::life` and `assets/data/items.json`. This plan covers the places and the props.

## Buying without a menu

- **Shelves and fridges**: things sit on shelves (the konbini's onigiri cooler, the drinks fridge). Look at one and pick it up: it's in your hand. Take it to the till; whoever is on shift there (the census's schedule) rings it up, and your wallet comes out with the money you have. Walk out with something unpaid and the shopkeeper calls after you.
- **Counters and cases**: at the bakery, the butcher's, the wagashi shop, you look at the tray or case and say which and how many. They wrap it, you pay.
- **Vending machines**: look at a button (cold blue rows, hot red rows in winter), coins go in, the can drops into the tray, and you pick it up.
- **Eating in**: the ramen and soba counters, the coffee shop's tables. Sit down, order, and you're eating (the Eat activity).
- **Stock runs out**: shelves hold a day's stock and are filled in the morning. The bakery's curry bread is gone by noon; the konbini's bento thin out by evening and get marked down (the half-price sticker).
- **Opening hours** come from the census: shops with keepers on shift are open; closed shops have their shutters down.

## Where

| Place | Open | Sells | Inside |
|---|---|---|---|
| **The konbini** | 6:00–23:00 | onigiri, bento, bread, sandwiches, hot snacks, oden (winter), cup noodles, sweets, ice cream, drinks, magazines, umbrellas, bug nets (summer), sparklers (summer) | the onigiri and bento cooler, the drinks fridge, the hot case and oden pot by the till, the magazine rack, the ice cream freezer, the till, the eat-in counter |
| **Vending machines** | always | drinks (hot in winter) | by the station, the petrol station, the temple's car park, outside Tsukumodō, the bus stop |
| **The open shophouses** (about 15) | their keepers' hours | what their trade sells | a counter and a display for the trade: the bakery's trays, the butcher's hot case, the tofu shop's water tanks, the wagashi glass case, the greengrocer's crates out front, the fishmonger's ice, the stationery shop's dagashi corner |
| **Ramen and soba shops** | lunch and evening | bowls, eaten at the counter | counter stools, the noren at the door |
| **The coffee shop** | 9:00–17:00 | toast sets, soft-serve | tables |
| **The JA co-op shop** | 8:30–17:00 | rice, eggs, local fruit and vegetables | crates and shelves |
| **The drugstore** | 9:00–20:00 | snacks, drinks, cup noodles, plasters, bath salts | shelves |
| **The shrine office** | 9:00–16:00 | charms, fortunes, ema | the window |
| **Festival stalls** | the summer festival | takoyaki, yakisoba, candy apples, shaved ice, goldfish, masks | stalls with lanterns |
| **Tsukumodō** | Kenji's hours | packs, sleeves, deck boxes, the card magazine | (the shop) |
| **School** | | school lunch (served in the classroom), library books | |
| **Home** | | Kenji's meals, the fridge, barley tea in summer | the kitchen (`docs/home_plan.md`) |

## Props for the food and things

98 items in the catalog, but they come in a few shapes. Each **family** is one builder with variants (colour, label, filling):

| Family | Items |
|---|---|
| **Onigiri** (wrapped triangle, nori, label) | 4 onigiri |
| **Bento** (tray, clear lid, compartments) | 2 bento, school lunch tray |
| **Sandwiches and bread** (wedge pack; buns with their tops) | egg sandwich, melon pan, anpan, curry bread, yakisoba bread, the loaf |
| **Hot snacks** (paper sleeve, a skewer, a bun) | nikuman, fried chicken, croquette, menchi katsu, oden cup |
| **Cups and bowls** (cup with lid; a bowl with noodles) | cup ramen, cup yakisoba, ramen, soba, kakigori |
| **Bottles and cans** (PET bottle with label; can; glass ramune bottle; carton) | tea, cola, sports drink, Calpis, corn potage, ramune, milk, Yakult, soy milk |
| **Sweets on sticks and in packs** | dango, yakitori, candy apple, ice pop, chocolate sticks, chips, gummies, umaibō, kinako stick, ramune candy |
| **Wagashi** (mochi and cakes on a paper) | sakura mochi, kashiwa mochi, daifuku, taiyaki |
| **Fruit** | mikan, apple, banana, watermelon slice, strawberries, persimmon, corn |
| **Ingredients** (packs and blocks) | tofu, aburaage, natto, eggs, rice bag, miso, mackerel, sashimi |
| **Paper things** | magazines, manga, notebooks, textbooks, library book, card magazine, the journal (done) |
| **Small goods** | sleeves, deck box, umbrella, towel, plasters, bath salts, charms, fortunes, ema, fishing rod, bug net and cage, sparklers, goldfish bag, mask |

Each family is built, checked by the gate, rendered as a look-dev sheet of all its variants, and approved before any of it goes on a shelf. **One approval per family** keeps it moving (about a dozen approvals rather than 98); if you'd rather see every item on its own, say so.

## Building it

1. **Food families**, starting with the konbini's (onigiri, bento, bottles and cans, hot snacks, cups), each for approval.
2. **The konbini**: its interior and fittings (cooler, fridge, hot case, shelves, till), stocked from the catalog. Buying at the shelf and the till, and the staff on shift.
3. **Vending machines**: one model with hot and cold rows, placed round town.
4. **The open shophouses**: a counter and display per trade, and the noren (`docs/cloth_plan.md`).
5. Eating in: the ramen and soba counters, the coffee shop.
6. The festival stalls (for August).
