# ORIGINAL777 tracker offsets: bounded NONCOMBAT calibration

NEW source-only preregistration. No push/CI/capture is authorized yet. Old Wind
head66a/criteria87, both failures and raw1058 remain immutable. No Wind, Attack,
use-item, candidate/core, Cargo, tag policy, main merge or deployment is included.

Question: what can ordinary authoritative ground Pos readbacks and ordered raw
observer packets establish about stepped tickOffset? Arrival time is not server
simulation time. A nonzero offset must never be accepted as current pose merely
because it was observed, and Explosion C is not a pose oracle.

A fresh opaque original26.3/protocol777 server and two own offline clients use a
constant stone floor blockY63/top64, X/Z92..124, clear AIR Y64..84. Actor starts
(100.5,64,100.5), observer stays (100.5,64,106.5), six blocks away. All source
positions are real ground-supported feetY64, multiples of1/4096, no body overlap,
flight, gravity edits, jump, teleport during the schedule, collision sweep or
physics model. Initial ordinary TP and matching raw ACK are mandatory.

The frozen412-frame/20Hz grid alternates dense20Hz movement, sparse10Hz movement,
known irregular gaps and stationary holds. No frame or phase is chosen after
seeing tracker packets. Ground X changes by0.125 per planned movement, max2blocks
from start. Repeated stationary source reports occur every20frames. TickEnd stays
20Hz. The observer reports the same ground position every20frames.

Eleven authority samples have predetermined frames. A single RCON worker submits
before-token /say, `data get entity blackbox Pos`, `data get entity blackbox
OnGround`, after-token /say, in that order. It runs separately from the movement
grid. No command retries or adaptive holds. Every raw RCON request/reply has
start/end timestamps. BOTH clients must receive exact unique typed profileless
phase tokens from Rcon/chatType4/targetNone after issuance. RCON Pos is parsed as a
three-double list with the exact named-player response prefix, and OnGround as a
byte boolean. Pos and OnGround are separate authority reads, not one atomic pose.

For held samples, Pos must equal the predetermined held point. For moving samples,
actual Pos must be one of the already transmitted finite ground positions; its
relation to observer endpoint delivery is reported, not assigned a server tick.
Any correction, unplanned teleport, missing ACK, bad cadence, lost sample/token,
non-ground state, disconnected client, foreign movement or unexpected Wind/damage
is technical INCONCLUSIVE. No replay after failure. A complete capture can be
technically valid while offset semantics remains UNIDENTIFIABLE.

Observer coordinates are independently reconstructed from actual absolute bases
and quantized delta steps. Every tickOffset is preserved, including nonzero ones,
as an observable with NO implicit eligibility for future Wind prehit proof.
Separate raw UpdateTime ages bracket coordinate packet EMISSION ORDER on that
single observer stream; these are not endpoint sample times or interpolation
execution times. Closest before/after ages may have20tick uncertainty. Never
replace age brackets with (arrival delta)*20.

One narrowly testable hypothesis is `sum(step.tickOffset) equals world ticks since
previous emitted coordinate packet`. If current-before age minus previous-after
age is strictly greater than that sum, ordered raw age brackets exclude THIS
emission-gap hypothesis. Otherwise the test remains inconclusive; no expected
numeric offset or favourable count is required. This does not decide elapsed time
between underlying samples, interpolation duration, or current-authority binding.

Held endpoint agreement with independent Pos and phase echoes is reported as a
bounded ground observation. Interpolation duration versus batched sample timing
may remain observationally indistinguishable. The helper never unlocks nonzero
Wind pose eligibility. Ground evidence is not falling/combat proof. Any future
Wind series needs NEW root-reviewed environment/criteria and independent pose
justification. The old partial C=feet in AIR is not reused as that justification.

Save exact JAR/protocol/source/criteria/config hashes, all raw streams, all query
attempts/partial schedules and cleanup. Explicit whitelist=false. Original Java
only on isolated26180/26181; bounded300s watchdog; stop only own Popen/group and
verify actual exit0/group gone/ports free. Retain timeout/TERM as failures. Pipeline
has one exact branch and attempt1 only; source/root review precedes a separate CI
gate. No capture permission is implied by this document.

Pre-capture revision v2: path phases have fixed stationary tails; every moving
authority sample has at least1.2seconds until fixed phase end (worker-start<=.1s +
sample-duration<=1s). No adaptive waits. Original336 draft and SHA are retained.
