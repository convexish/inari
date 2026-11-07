# Decree effect source review

Primary sources reviewed for the replacement kernel:

- Ultimate rulebook v4.1,
  pp29–30: Decree action admission/payment, ownership cases, the Homer example,
  and Expansion's effect.
- The special-card sheet v0.9,
  rendered card fronts on PDF pages 31, 33, 35, 37 and 39.
  This is the v0.9 sheet, not an independently verified final-retail
  transcription. The rulebook independently confirms Expansion and ownership.

The printed Decree cards (checked 2026-10-25) are
the authority where they differ from the v0.9 sheet: Advancement draws **two**
higher than the highest top card, not three as the sheet had it.

Effect summaries (two-player scope):

| Decree | Color | Effect |
|---|---|---|
| Expansion | Yellow | Splay one own color up. |
| Trade | Green | Draw three cards, requested age highest own top + 1. |
| War | Red | Choose a value; return one matching top per opposing color. |
| Rivalry | Purple | Return three opposing score cards, then one top Figure per opposing color. |
| Advancement | Blue | Draw one card, requested age highest own top + 2. |

Issue an opponent-owned token: return it to supply without its effect. Issue a
supply token: finish its effect, then claim it. Issue one's own token: repeat
its effect without another claim. A terminal effect prevents the later claim.
Decrees count as achievements, but are not Special Achievements or standard
card achievements. Their claims do not grant an Achieve-action Figure draw.

The kernel's `IssueDecree` operation starts **after authorization/payment**.
`Action::Decree` performs admission and whole-hand payment before selecting a
Decree and issuing it. Ordinary admission requires three distinct Figure ages;
Nightingale grants Expansion and Archimedes grants Advancement with any two
Figures. Other roster permissions remain unimplemented.

**The subsequent project ruling:** use the initial hand's Figure colors,
even when Homer tucks every Figure instead of junking it. The p29 example says
the action continues, while p30 describes a color of a Figure "you junked";
we confirmed initial-hand colors so Homer cannot erase all choices.
The permission list is retained through payment, not chosen until afterward.
Non-Figures are still junked, and payment replacements do not count as successful
junking for any other conditional. Meiji's initial hand is evaluated at the new
action's timing, so the second regular action cannot borrow the opponent's hand.
