# Ordered multiversion plan
1. Keep canonical world IDs separate from client IDs and primary gameplay profiles; pin allowed JSON and retain raw evidence.
2. Build adjacent, directional adapters 774 ↔ 775 ↔ 776 ↔ 777; compose routes rather than implement every pair.
3. First validate clients 26.2 (776) and 1.21.11 (774) on both primaries 26.1.2 and 26.3.
4. Cover login/configuration, chunks/light, entities, inventory, interactions, combat/PvP, death/respawn and relogin for each route.
5. Give missing content explicit visual substitutes; preserve true server items/states and validate client predictions against authoritative inventory.
6. Keep admission closed until every required check passes independently for that client and primary; intermediate adapters do not grant admission.
7. Extend the chain to priority clients 1.21.1, 1.20.1, 1.17 (exactly), 1.16.5, then 1.12.2.
8. Add intervening release protocols as required: components/Configuration, signed chat, old slots, height/light changes, then numeric blocks/items.
9. Verify mixed clients, dimensions, recipes/containers, AuthMe/online-mode and persistence at each stage; retain existing combat regressions.
10. Complete remaining stable client releases in 1.12.2–26.3; snapshots and 1.8–1.11 remain outside scope.
11. Implement selectable primary profiles independently from client translation, including registries, generation, mechanics, recipes and commands.
12. A primary older than 26.1.2 exposes ONLY that version's content, rules and commands; a newer client does not unlock newer gameplay.
13. Validate profile-bound saves, explicit backed-up upgrades, and refusal of unsupported downgrade before listeners open.
14. Publish a client × primary readiness matrix with raw evidence; never infer full support from packet IDs or one successful login.

Step 1 readiness (client translation; gameplay still follows the selected primary):

| Client | Primary26.1.2 | Primary26.3 |
|---|---|---|
| 26.2 (776) | OPEN | OPEN |
| 1.21.11 (774) | OPEN | OPEN |

776 completes the existing checklist: prior recipe-book/crafting/Nether-return and password-dialog evidence, shared encrypted-login tests, fresh11/11 gameplay/restart per primary, and an actual26.2 client accepting196 translated payloads per primary. Live GUI actions prove cancel/drop/release/swap/jab and four statue poses. The typed path covers the core's component writers, metadata41/rich text, populated block entities and the remaining shared GUI/action envelopes. Primary profiles and canonical saved content are unchanged. Unimplemented arbitrary custom forms remain refused; End travel and Nether/End persistence remain unclaimed, without adding admission gates beyond the existing checklist. Per owner decisions, chat fallback and a live Mojang account trace are not required. 774 now completes the same checklist on both primaries:196 payloads,6 GUI actions,4 statue poses,11 gameplay/restart checks,4 recipe/Nether checks, password registration/relogin, and the shared encrypted-login test extended to774. Its adapter converts clocks, combined interactions/attacks, recipe displays, count-first nested slots, metadata indices/serializers, registries and the single-count chunk header. The existing Russian refusal is retained for unsupported routes. See [continuation and raw evidence](../tools/protocol-bridge/testdata/step1-774-776/CONTINUED.md#776-admission-and-evidence).
