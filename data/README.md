# Card data

Card texts are © Asmadi Games. They are not covered by this repository's MIT
license.

One JSON file per set, each an array of cards sorted by publisher code. Every
set shares one schema and leaves out the fields it never prints. Age 0 cards
(`age_zero.json`) are Base cards that ship with Figures.

| Field | Meaning |
|---|---|
| `code`, `title`, `color`, `age` | Identity; `age` is the printed age the back shows. |
| `face_value` | The value on the face where it differs from `age` (Battleship Yamato). |
| `icons` | The standard icon in each slot, or `null`: `top_left`, `bottom_left`, `bottom_center`, `bottom_right`, then a City's `top_center` and `top_right`. |
| `bonuses` | Per slot, a bonus value or `null` (Echoes, Cities, Figures). |
| `city_icons` | Per slot, a City icon or `null`; `search` marks that slot's standard icon (Cities). |
| `echoes` | Echo effects: `slot` and `text` (Echoes). |
| `effects` | Dogma effects in printed order: `featured_icon`, `kind` (`non-demand`, `demand` or `compel`) and `text`. |
| `karmas` | A Figure's karmas in printed order: `kind` (`would`, `when`, `each` or `decree`), `reaction` (`first` or `instead` for `would`) and `text`. |
| `appendix` | A clarifying note on the card, where the source data has one (Figures). |
| `image_slot` | The slot holding the art, where more than one slot prints nothing (Battleship Yamato). |
| `variant` | Marks text that follows a project ruling instead of the printed card. |

Each slot prints one thing; the single slot printing nothing is the image.
In text, `[3]` is an age, `{[HEALTH]}` an icon and `[IMAGE]` the hex.

`pixi run metadata` regenerates the engine's `engine/src/catalog/*_metadata.rs`
tables from these files, and `pixi run check` fails when they are out of date.
The app reads the files directly for card faces.
