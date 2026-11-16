# Opening hands in Base

**Question.** How much does the opening hand (two Age 1 cards) matter, and
which cards are good to be dealt?

**Answer.** A hand's score is close to a sum over its two cards, minus 2.6
points if they share a color. Metalworking and Clothing are worth about +5
points each; Writing, Code of Laws and Tools −3 to −4.5. The best hand,
Clothing+Metalworking, scores 62% against an equal opponent and the worst,
Code of Laws+Writing, 42%.

## Method

`opening_hands` plays 200,000 Base games of the network against itself.
Each seat's starting meld is a coin flip, so a hand's score does not depend
on which card the network would choose. After the melds both seats play
the policy at 850 Elo, its strongest setting that still samples (greedy
play is deterministic and 6% of its games loop to the decision cap).

## Results

A hand scores 50.4% plus the values of its two cards, minus 2.6 (±0.2) if
they are the same color. Card values are in points, ±0.2:

| Card | Color | Value |
|---|---|---|
| Metalworking | red | +5.8 |
| Clothing | green | +4.6 |
| The Wheel | green | +2.0 |
| Mysticism | purple | +1.6 |
| Sailing | green | +1.4 |
| Pottery | blue | +1.4 |
| Agriculture | yellow | +1.0 |
| Archery | red | −0.6 |
| Domestication | yellow | −0.8 |
| Oars | red | −1.3 |
| City States | purple | −1.7 |
| Masonry | yellow | −2.1 |
| Tools | blue | −3.0 |
| Code of Laws | purple | −3.8 |
| Writing | blue | −4.4 |

Each of the 105 hands is measured to ±0.8 points, and the hands sit 1.2
times that far from this rule, so it is nearly the whole story.

Which card to meld matters less than which cards are dealt. Four cards do
better melded than kept in hand: Clothing (+2.9 points), Metalworking
(+2.2), Domestication (+1.5) and Sailing (+1.4). Three do better kept:
Writing (−2.6), Code of Laws (−2.1) and Archery (−1.4). The other eight
show no measurable preference (each is measured to ±0.4).

The seat that takes the first turn scores 50.6%.

## Reproduce

Weights `weights/inari_v1.0.bin` (fnv1a64 `1974b9846d5c2526`), revision
`96d3571`. 12 minutes with 22 workers
on a 24-thread machine.

```sh
pixi run lab opening_hands run > lab/runs/opening-hands.csv
pixi run lab opening_hands summary lab/runs/opening-hands.csv
```

## Caveat

Both seats play the policy without search. Between 1200-Elo search players
the best hand against the worst scores 72%
([hand vs. skill](2026-11-15-hand-vs-skill-example.md)).
