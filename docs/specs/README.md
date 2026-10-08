# SuperCom specification index

The reverse-engineered specification is the authority for the engine: code
satisfies spec clauses, never the other way round. Every clause carries an
identifier and names its evaluator (test, property, contract, or proof), per
predicate's `spec` workflow. The key words MUST, SHOULD, and MAY are to be
read as in BCP 14.

Clean-room rule: the spec records observed behaviour of the installed game and
published, licensed references. It never reproduces game or FAF Lua source.

| Domain      | Scope                                                                                                                                                                           | Status |
| :---------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | :----- |
| formats     | `.scd` archives and mount order; the data subset of the GPG Lua dialect in `.bp`; `.scm` and `.sca`; `.scmap` with `_scenario.lua` and `_save.lua`; DDS; XACT `.xwb` and `.xsb` | WIP    |
| data-model  | Nickel contracts for the blueprint families; mod overlay semantics; the import cache                                                                                            | WIP    |
| sim         | tick model, economy, units and categories, orders and build queues, weapons and projectiles, damage and armour, pathfinding, veterancy, intel                                   | WIP    |
| determinism | lockstep command stream, replay format, float strategy, desync detection                                                                                                        | WIP    |
| render      | terrain, decals, props, shadows, water, units, fog of war, strategic icons, post-processing                                                                                     | WIP    |
| plugin-api  | WIT worlds, host imports, versioning, sandbox policy                                                                                                                            | WIP    |
| ai          | the skirmish AI interface and the built-in AI's contract                                                                                                                        | WIP    |

Each domain gets its own file in this directory, authored by the spec campaign.
