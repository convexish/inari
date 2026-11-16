# How Artifacts games end at 800 Elo

**Question.** With Artifacts as the only expansion, how are games between
800-Elo players decided?

**Answer.** 42% by achievements, 33% on score when the decks run out, and
25% by a card: 13% by a Base card and 12% by an Artifact. Holy Lance decides
the most games of any card, 5%; Dancing Girl decides 2%.

## Method

`endings` plays 50,000 Base + Artifacts games of the network against
itself, both seats the policy at 800 Elo (temperature 0.425), and records
the engine's reason for each result and the card whose effect caused it.

## Results

| Ending | Games |
|---|---|
| Achievements | 42.0% ±0.2 |
| Score, when a draw past the last age ends the game | 33.1% ±0.2 |
| A card's effect | 24.7% ±0.2 |
| Draw | 0.2% |

Cards that decided at least 0.1% of games (50 games):

| Card | Set | Games |
|---|---|---|
| Holy Lance | Artifacts | 5.03% |
| Bioengineering | Base | 4.08% |
| Self Service | Base | 3.49% |
| Globalization | Base | 2.66% |
| Dancing Girl | Artifacts | 2.21% |
| Astrogeology | Base | 1.42% |
| Earhart's Lockheed Electra 10E | Artifacts | 1.32% |
| Space Traffic | Base | 0.66% |
| Velcro Shoes | Artifacts | 0.49% |
| Where's Waldo | Artifacts | 0.40% |
| Seikan Tunnel | Artifacts | 0.38% |
| Magnavox Odyssey | Artifacts | 0.37% |
| Empiricism | Base | 0.33% |
| Syncom 3 | Artifacts | 0.33% |
| Parnell Pitch Drop | Artifacts | 0.29% |
| Musk's Tesla Roadster | Artifacts | 0.28% |
| Collaboration | Base | 0.24% |
| A.I. | Base | 0.19% |
| Tanker Exxon Valdez | Artifacts | 0.15% |
| International Prototype Metre Bar | Artifacts | 0.14% |
| Dragon's Lair | Artifacts | 0.10% |

Space Traffic, Tanker Exxon Valdez and Dragon's Lair end the game by making
a player lose; the rest by making one win. Five more cards (Dolly the
Sheep, Marilyn Diptych, Cross of Coronado, Solar Sailing and Garland's Ruby
Slippers) decided 99 games between them.

## Reproduce

Weights `weights/inari_v1.0.bin` (fnv1a64 `1974b9846d5c2526`), revision
`cf5f5f1`. 3 minutes with 22 workers on a 24-thread machine.

```sh
pixi run lab endings run expansions=artifacts > lab/runs/endings-artifacts-800.csv
pixi run lab endings summary lab/runs/endings-artifacts-800.csv
```
