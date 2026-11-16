# Hand vs. skill: Clothing+Metalworking against Tools+Writing

**Question.** When one player is dealt the best Base opening hand
(Clothing+Metalworking) and the other the worst (Tools+Writing), how much
does it matter, and can a stronger player overcome it?

**Answer.** Between equal players the good hand wins about 70% of games at
every strength from 600 to 1200 Elo: it is worth about 150 Elo. A player
200 Elo stronger still wins with the bad hand, but only 53–64% of the time
instead of 75–80%. A player 400 Elo stronger wins 77–85% with it, and a
player 600 stronger 94%.

## Method

`hand_matchup` finds the 734 Base deals among seeds 0–2,999,999 where one
seat is dealt Clothing+Metalworking and the other Tools+Writing, and plays
each twice, once with each player holding the good hand. When the players
differ, another 734 random deals, played once from each seat, give the
baseline. Every pairing of 600, 800, 1000 and 1200 Elo is played, each
strength against itself included.

Players are the app's strength settings: 1200 is search with 128
simulations per move, 1000 is 13 simulations, 800 and 600 are the policy
sampled at temperatures 0.425 and 1.04.

## Results

The score of the player holding **Tools+Writing** (rows) against each
opponent holding Clothing+Metalworking (columns). Each cell is 734 games
(1,468 on the diagonal), ±2 points or better:

| Bad hand ↓ / good hand → | 600 | 800 | 1000 | 1200 |
|---|---|---|---|---|
| **600** | 28.6% | 9.5% | 0.7% | 0.0% |
| **800** | 57.4% | 31.1% | 9.5% | 2.2% |
| **1000** | 84.5% | 63.6% | 30.1% | 15.3% |
| **1200** | 94.1% | 77.4% | 53.2% | 28.3% |

For comparison, the stronger player's score on random deals:

| Stronger ↓ / weaker → | 600 | 800 | 1000 |
|---|---|---|---|
| **800** | 79.7% | | |
| **1000** | 95.1% | 79.5% | |
| **1200** | 98.9% | 92.7% | 75.2% |

On the diagonal, the bad hand's 28–31% puts the hand's worth at 159, 138,
146 and 161 Elo (each ±10) for 600, 800, 1000 and 1200: no trend with
strength.

## Reproduce

Weights `weights/inari_v1.0.bin` (fnv1a64 `1974b9846d5c2526`), revision
`96d3571`. Roughly an hour in total with 22 workers on a 24-thread machine;
the four pairings with a 1200-Elo search player take most of it.

```sh
for pair in "1200 1200" "1000 1000" "800 800" "600 600" \
            "1200 1000" "1200 800" "1200 600" "1000 800" "1000 600" "800 600"; do
  set -- $pair
  pixi run lab hand_matchup run a=elo:$1 b=elo:$2 > lab/runs/hand-matchup-$1-$2.csv
  pixi run lab hand_matchup summary lab/runs/hand-matchup-$1-$2.csv
done
```

## Caveats

- This is the most lopsided pairing of hands; a typical deal's hands differ
  far less (see [opening hands](2026-11-15-base-opening-hands.md)).
- All players are the same network, so the result is about how that
  network plays these hands.
