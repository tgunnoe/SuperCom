# HUD and input specification

This file specifies the basic HUD of the v0 skirmish: how it reads the sim and issues commands, selection, orders, the resource display, the build menu, strategic zoom and its icons, and the key actions, together with the facts about the installed game's HUD data that bear on them. It follows the clause grammar of docs/specs/README.md. A clause of class `design` is a decision of this project and states nothing about how the original engine or its HUD behaves; a clause of class `game-data` is a fact about the installed Forged Alliance data and is machine-verified by a script that reads only `SUPERCOM_FA_DIR`, reads archive members in memory, prints counts, constants, and identifiers, prints `SKIP:` and succeeds when the install is absent, fails under `SUPERCOM_REQUIRE_GAME=1`, and exits non-zero when its clause is false. A clause whose evaluator is a pinned source is cited, not run; it is agent-checked. Clauses cite the simulation specification (docs/specs/sim.md) by clause ID, and the AI specification (docs/specs/ai.md) where the two share a view of the sim.

Screen layout, widget geometry, the strategic zoom threshold, the camera's pan and zoom limits, and the minimap, chat, diplomacy, and score screens are frontiers of the implementation; nothing here invents them.

## The HUD's contract with the sim

**HUD-001** `design` `source:docs/specs/sim.md#SIM-018`
Every HUD action that changes the sim MUST resolve to a command of the shape SIM-018 fixes: the issuing army, the unit entity IDs addressed, one order of the SIM-016 vocabulary, and whether it replaces or appends. The HUD MUST NOT change the sim by any other path, so its commands and the built-in AI's (SKAI-001 in docs/specs/ai.md) are one command stream (R2). Construction is the one family not in SIM-016's list of command capabilities; its orders are the build orders of SIM-017 (IssueBuildMobile, IssueBuildFactory, IssueBuildAllMobile, and IssueFactoryAssist), and widening SIM-018's order field to them is a frontier of the sim specification that this clause assumes.

**HUD-002** `design` `source:AGENTS.md`
The HUD MUST read the sim only through the sim's published snapshot, the view SKAI-002 gives an AI; it MUST treat the snapshot as read-only and MUST show a unit of another army only to the extent that the intel of SIM-030 and SIM-032 reveals it to the HUD's focus army. HUD state (selection, camera, command mode, panel layout) lives outside the sim state and MUST reach the sim only as the commands of HUD-001 (R2, I2).

**HUD-003** `design` `source:docs/adr/0003-render-and-sim-stack.md`
The HUD MUST be built on winit for windowing and input and on egui for panels (ADR-0003), and its logic MUST be testable without a window or a GPU (I4): input handling is a pure function of the previous HUD state, the snapshot, and one input event, returning the next HUD state and the commands to issue.

**HUD-004** `design` `source:docs/specs/sim.md#SIM-018`
Selection MUST be HUD-local state and never sim state: a set of unit entity IDs changed only by HUD input. A command carries the IDs the selection holds when it is issued (SIM-018), a selection of another army's units MAY exist for inspection and MUST NOT be used to issue commands, and an entity that leaves the snapshot MUST drop out of the selection on the next snapshot.

## Orders

**HUD-005** `game-data` `script:tools/spec_eval/ui/panels.py HUD-005`
The installed order bar MUST be taken as keyed by the command capabilities of SIM-016: lua/ui/game/orders.lua of lua.scd names 22 distinct RULEUCC identifiers, which are 20 of the 21 capabilities that blueprints grant (all but RULEUCC_CallTransport) and 2 that no blueprint grants (RULEUCC_Script and RULEUCC_Teleport), and 9 RULEUTC identifiers, which are all 8 toggle capabilities of SIM-016 and RULEUTC_GenericToggle, which no blueprint grants.

**HUD-006** `design` `source:docs/specs/sim.md#SIM-016`
An order of SIM-016's vocabulary MUST be available in the HUD exactly when at least one selected unit's blueprint grants the matching capability in General.CommandCaps (or General.ToggleCaps for a toggle), and a command MUST address only the selected units whose blueprint grants it. An order issued plainly MUST replace the addressed units' queues and one issued with the Shift modifier MUST append to them, the replace-or-append field of SIM-018; HUD-018 shows that the installed bindings draw the same line.

## Resources

**HUD-007** `game-data` `script:tools/spec_eval/ui/panels.py HUD-007`
The installed economy bar MUST be taken as a view of per-army totals: lua/ui/game/economy.lua of lua.scd defines CreateEconomyBar, CreateResourceGroup, DisplayEconData, FormatRateString, and ToggleEconPanel, and reads the totals through GetEconomyTotals, with the fields income, stored, maxStorage, lastUseActual, and lastUseRequested.

**HUD-008** `design` `source:docs/specs/sim.md#SIM-007`
The resource display MUST show the focus army's two resources and no others (mass and energy, SIM-007), and for each the stored amount, the storage capacity, the income, and the usage, all taken from the snapshot's economy totals (SIM-009 names the income, requested, stored, and usage quantities; SIM-006 to SIM-008 define the blueprint constants behind them). The HUD MUST NOT do economy arithmetic of its own beyond formatting.

## Build menu

**HUD-009** `design` `source:docs/specs/sim.md#SIM-011`
The build menu of a selected builder MUST list exactly the blueprints that the builder's BuildableCategory selects under SIM-011's reading, grouped by the tech-level categories of SIM-013 to SIM-015 (TECH1, TECH2, TECH3, EXPERIMENTAL), and each entry MUST show its blueprint's strategic icon (HUD-013). With several builders selected the menu lists the union of their menus, and the build command of an entry addresses only the selected builders that can build it. Choosing an entry issues a build order (HUD-001); layout is implementation.

**HUD-010** `game-data` `script:tools/spec_eval/ui/build.py HUD-010`
Under the reading of SIM-011, grouping the build menu by tech-level category MUST leave no entry ungrouped: of the 370 blueprints that at least one builder selects, 105 carry TECH1, 134 TECH2, 116 TECH3, and 15 EXPERIMENTAL, each exactly one of the four (SIM-014), and no builder's menu holds a blueprint without a tier. The largest menu holds 45 entries; the commanders' menus hold 45 (UEL0001: TECH1 14, TECH2 15, TECH3 12, EXPERIMENTAL 4), 45 (UAL0001: 14, 14, 13, 4), 45 (URL0001: 14, 15, 12, 4), and 41 (XSL0001: 14, 14, 10, 3), and 1 of the 105 builders selects nothing. lua/ui/game/construction.lua of lua.scd names TECH1, TECH2, TECH3, EXPERIMENTAL, StrategicIconName, and IssueCommand.

## Strategic zoom and icons

**HUD-011** `design` `source:AGENTS.md`
The camera MUST zoom continuously between a tactical range and a strategic range. At and beyond a strategic zoom threshold the HUD MUST draw every unit that the snapshot reveals to the focus army as a strategic icon at the unit's position (HUD-015), and selection, orders, and the build menu MUST work on icons exactly as on meshes. The threshold, the transition between the two renderings, and the pan and zoom limits are frontiers of the render and camera implementation.

**HUD-012** `game-data` `script:tools/spec_eval/ui/icons.py HUD-012`
The strategic icon art MUST be taken from textures.scd, the only archive that holds textures/ui/common/game/strategicicons/: the folder holds 1102 .dds files, of which 1080 are 270 icons named icon_KIND_ROLE (40 kinds, 24 roles), each present in the four states over, rest, selected, and selectedover, so all 270 are complete in the over, rest, and selected states. The other 22 files fit no such stem: 8 icon_ files without a role or a state (icon_objective in the four states, and icon_strategic_antinuke, icon_strategic_artillery, icon_strategic_nuclear, and icon_strategic_nuke), 12 files of the three ping icons strat_alert_ping, strat_attack_ping, and strat_move_ping in four states, and pause_rest and stunned_rest.

**HUD-013** `game-data` `script:tools/spec_eval/ui/icons.py HUD-013`
A unit's strategic icon MUST be named by its blueprint's StrategicIconName: 550 of the 568 unit blueprints carry one, with 100 distinct values, each of the form icon_KIND_ROLE and each naming one of the 270 icons of HUD-012; the other 18 blueprints carry none, and 170 of the 270 icons are named by no unit blueprint.

**HUD-014** `derived` `script:tools/spec_eval/ui/icons.py HUD-014`
Following from HUD-012 and HUD-013, an icon that a unit blueprint names MUST have all four state files: all 550 named icons resolve to a stem present in the over, rest, selected, and selectedover states, so the HUD treats a missing state file of a named icon as a data error and needs no per-state fallback.

**HUD-015** `design` `source:AGENTS.md`
The HUD MUST draw a unit's icon in the rest state by default, in the over state when the pointer is over it, in the selected state when it is in the selection (HUD-004), and in the selectedover state when both hold; this mapping of the four states of HUD-012 is the project's own. A blueprint with no StrategicIconName MUST NOT be given an invented name, and what the HUD draws for it is implementation-defined.

## Key actions

**HUD-016** `game-data` `script:tools/spec_eval/ui/keys.py HUD-016`
The installed key actions MUST be taken as two tables of named actions in lua/keymap/keyactions.lua of lua.scd: keyActions holds 135 actions (selection 59, orders 32, ui 25, camera 12, chat 4, game 3) and debugKeyActions holds 44 (debug 43, game 1), 179 in all. lua/keymap/keycategories.lua of lua.scd defines the 9 categories ui, selection, camera, orders, game, chat, debug, user, and none, and every action's category is one of them.

**HUD-017** `game-data` `script:tools/spec_eval/ui/keys.py HUD-017`
The installed default bindings MUST be taken as two tables in lua/keymap/defaultKeyMap.lua of lua.scd that bind key strings to action names: defaultKeyMap holds 135 bindings and debugKeyMap 44, 179 in all. Every binding names an action its table defines, 0 of the 135 actions and 0 of the 44 debug actions lack a default binding, no action is bound twice, and the two tables share no key string.

**HUD-018** `game-data` `script:tools/spec_eval/ui/keys.py HUD-018`
The installed key actions MUST be taken as covering the HUD families of this file: the selection category holds 40 control-group actions (group, set_group, append_group, and fac_group for each digit 0 to 9), the camera category holds the four zoom actions zoom_in, zoom_in_fast, zoom_out, and zoom_out_fast, and the orders category holds 32 actions, 18 base orders and 14 shift_ variants, each variant mirroring a base action and bound to a key string that starts with Shift-.

**HUD-019** `design` `source:AGENTS.md`
The v0 HUD MUST provide key actions, as a table of named actions bound by a table of key strings so that bindings are data, for every order the HUD offers (HUD-006) with its shift-appending variant, for selection (select all, select by type, and the ten control groups with set, recall, append, and factory forms), and for camera zoom (in, out, and their fast forms). It MAY omit chat, the debug actions, layout and skin switching, and diagnostic actions. A key action that changes the sim resolves to a command (HUD-001); every other is HUD-local. The default key strings MAY be read at run time from the installed game's key map (R1) and the repository MUST NOT carry a copy of it (I1).

## Installed panels

**HUD-020** `game-data` `script:tools/spec_eval/ui/panels.py HUD-020`
The installed HUD MUST be taken as organised by panel in lua/ui/game/ of lua.scd, which no other archive holds: 115 members, of which 2 are directory entries, 68 are files in the folder, and 45 are layout files under layouts/. The panels the v0 HUD needs are members won by lua.scd: borders, buildmode, commandmode, construction, controlgroups, economy, minimap, orders, rallypoint, selection, strategicIcons, unitview, worldview, and zoomslider (14), and their layouts exist as borders_left, borders_mini, construction_left, construction_mini, construction_right, economy_mini, orders_left, orders_mini, and orders_right (9).
