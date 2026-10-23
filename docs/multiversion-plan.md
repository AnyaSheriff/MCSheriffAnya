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

Step 1 status: public774/776 admission remains closed on both primaries. Private776 gameplay/restart has11/11 per primary; recipe-book/crafting/Nether-return traces have4/4 per primary, and password registration/relogin succeeds before player creation. The latest schema-based extension adds158 entity layouts,30 typed component kinds, Creative Slot lengths and nonempty particle metadata; it preserves sulfur cubes and substitutes cushions on776. These are codec regressions, not fresh official776 observations. The shared encrypted-login test now covers775/776/777 through the same RSA/session-service implementation; per owner decision, a live Mojang trace and chat fallback are not admission requirements for776. Remaining776 action enums, block-entity types/data beyond empty chest, unsupported component/metadata forms and final live payload coverage prevent admission. Full774 Play/gameplay remains absent. See [continuation and trace matrix](../tools/protocol-bridge/testdata/step1-774-776/CONTINUED.md).
