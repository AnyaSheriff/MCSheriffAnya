# Separate BlockPlace prototype: source review only

Base main8b9a8718. Initial canBuild=true is retained legacy SDK constructor semantics,
NOT a native permission input. Placed block is the existing pre-resolver preview.
No new hand/item/against/replaced getter, native permission or resolver change.

New namespace mcsa:place-priority-events/block-place@0.1.0 carries BOTH flags;
all capable -> global six phases; any legacy -> whole old dispatch. Old BlockBreak
world, frame implementation and 40..42 preserved byte-for-byte. Same cumulative
per-component fuel/guest/native budget helper and monotonic Host ID counter.
Trap retains BOTH last successful flags; healthy peers finish; failed store buried.

Build scripts compile NEW own Java fixtures with NEW compiler/SDK inputs.
Never run old priority_staged_ci.py reuse equality checks against new inputs.
Old seven+two malformed, old-five, old BlockBreak baseline and old Place baseline
WASMs must be loaded immutable with receipt SHA+OLD source Git proof. This patch
changes Entry/adapter/translator, so current-vs-old compiler input equality is FALSE.
No old fixture/criteria/raw/artifact rewritten or retranslated.

Expected new public12 (AB/BA×3 + solo6): zero mismatches/wrong verdict;
additional AB/BA buildfalse/restore/uncancelonly/buildonly: eight Host cases.
All traces include actual canBuild after each callback, including ignoreCancelled
handlers during uncancelled build=false. Mixed old A or B: original modes only,
whole legacy source prediction; callback order never interleaves old and new paths.

Native matrix in place-staged-cases.json: original10+four buildfalse/restore.
Each needs a fresh process, same proven stone geometry and actual plugin worker;
input hand, raw event/NBT, world, complete stacks, actual wire ACK/corrections and
phase boundaries retained. Uncancel changes from old Deny to Allow in both normal
orders and offhand; used stack2→1 in survival. No client visual rollback claim.

Safety/fault tests need actual new components. Six faults × four confirmed-state
combinations check independent flags; cumulative fuel/guest/native exhaustion must
happen BETWEEN phases, not first-call full timeout. Faults use valid actual preview
materials (stone/dirt/granite/andesite/diorite/calcite), never fabricated event fields.
Malformed components compiled from own A jar using alternate own WIT/adapter,
with namespace present but missing finish/wrong phase result. No main source WIT
mutation; no foreign code. Reload and Break→Place→Break exercise shared IDs/epochs.

FIRST CI gate is actual WIT parser+C signature check via check-place-priority-wit.py;
root source review before push/CI. End-to-end runner/immutable loader+independent
auditor need separate review before native CI. No runtime evidence claimed yet.
