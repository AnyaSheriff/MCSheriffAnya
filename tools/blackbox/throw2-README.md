# Throw trajectory matrix (Java 26.1.2)

`throw2_scenarios.py` generates 720 independent JSON scenarios, without starting
anything: 10 items × 3 pitches × 3 yaws × 8 movement cases. Each records 180 ticks,
projectile spawn velocity, tracking packets, impacts, and the player's received
packets. No blanket randomness masks hide trajectory or packet errors.

Items: snowball, egg, ender pearl, splash/lingering potion, experience bottle,
wind charge, trident, bow arrow, crossbow arrow. Cases: standing, walking,
sprinting, sprint-jump near the apex, sprint-jump descending, falling from an
8-block drop, moving minecart, moving boat. Each case has up45/flat/down30 and
Minecraft yaw 0/90/-135.

The `!look` and `look` values are **mineflayer radians**, converted as
`yaw = pi - radians(Minecraft yaw)` and `pitch = -radians(Minecraft pitch)`.
Thus up45 is +0.785398163397448 and down30 is -0.523598775598299.

The pad is a stone catch enclosure from -40 to +40 horizontally and up to 64
blocks high. It catches wind charges and long arrow flights before they leave
the tracking range. `pad_wide=64` and `pad_up=80` prevent the recorder's default
8-block window from hiding later updates. In the supplied recordings, a relative
move or full sync outside that window updates the recorder's position and is
then discarded; subsequent velocities/removal can also be discarded. Therefore,
absence of those packets in the old JSON does not prove velocity-only tracking. Every fill is below 32768 blocks.
Cleanup kills projectiles, chickens, endermites, XP, clouds, items, and vehicles,
clears the enclosure to air and restores its stone floor. Kill commands are
anchored to the original pad, including after a pearl teleport. Scenarios use
the recorder's default origin so lane isolation still works.

Bows/tridents charge before movement; crossbows load and release before firing.
Minecart and boat cases mount by interaction. Minecarts use powered rails;
boats receive successive `vehicle_move` packets, following `boat-drive.json`.
These are verification inputs, not recorded passes. Confirm successful mounting,
actual velocity, charge behavior, and the intended apex/descending phase from
new recordings; charging can affect sprinting, and client physics phase can
shift a release by one tick. Stone walls intentionally end some long flights
before they would hit an open ground plane.

Suggested first comparisons: egg sprintjump-apex/falling, snowball flat-standing,
all up45-standing variants, then walking/sprinting and vehicles. Preserve raw
spawn velocities to distinguish spread from inherited movement. In the supplied
recordings, egg/pearl velocity at +10 follows 0.99 drag with vertical update
`(vy - 0.03) * 0.99`; an initial zero move is distinct from in-flight corrections.
Bow/trident gravity and their existing 20-tick tracker are separate from thrown
items' 10-tick tracker. New recordings must establish cadence for each family.
