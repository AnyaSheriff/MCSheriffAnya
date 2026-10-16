`fall-mace-motion-26.1.2.json` contains only outgoing movement coordinates,
ground flags, teleport markers, mode changes and successful smash markers
extracted in event order from the supplied official 26.1.2 recordings:

- `tmp/bb/vanilla-fall-mode-teleport-motion.json` (`a: sent ...`)
- `tmp/bb/vanilla-pvp-mace-landing-motion.json` (`b: sent ...`)

Ticks are relative to `window.first`; Y is the original absolute client Y
(the stone surface is -59). Coordinates retain their recorded precision.
No client library implementation was used. The Rust replay places the surface
at Y=64 and checks damage and particle counts against recorded server output.
The velocity test also advances entity ticks and checks encoded motion packets.

Incoming landing packets occur at relative ticks 18/74/134 for the fall probe;
outgoing grounded movement occurs at 18/73/133. The mace equivalents are
15/78/139/200/257/318 and 15/77/138/199/256/317. These differences describe
packet arrival relative to the recorder clock, not a fixed extra landing tick.

Expected mace wide particle counts are 105/160/200/200/134/167. The older
`pvp-mace-landing-count` has 167 instead of 160 in its second trial; it has no
movement trace. Do not normalize that difference by teleport height: the motion
recording shows a descending packet between attack and the velocity response.
Ordinary particles are 40 in each of the 6- and 7-block trials; the attacker's
fall damage is respectively 1 and 2. The other trials cause no fall damage.
