#!/usr/bin/env python3
"""Evaluator for the order-vocabulary clause of docs/specs/sim.md (SIM-016)."""

import os
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import simlib as S

COMMAND_CAPS = {"RULEUCC_Attack": 191, "RULEUCC_CallTransport": 137, "RULEUCC_Capture": 27, "RULEUCC_Dive": 13, "RULEUCC_Dock": 42, "RULEUCC_Ferry": 10, "RULEUCC_Guard": 227, "RULEUCC_Move": 235, "RULEUCC_Nuke": 9, "RULEUCC_Overcharge": 4, "RULEUCC_Patrol": 228, "RULEUCC_Pause": 113, "RULEUCC_Reclaim": 29, "RULEUCC_Repair": 30, "RULEUCC_RetaliateToggle": 236, "RULEUCC_Sacrifice": 3, "RULEUCC_SiloBuildNuke": 9, "RULEUCC_SiloBuildTactical": 8, "RULEUCC_Stop": 294, "RULEUCC_Tactical": 4, "RULEUCC_Transport": 21}
TOGGLE_CAPS = {"RULEUTC_CloakToggle": 1, "RULEUTC_IntelToggle": 25, "RULEUTC_JammingToggle": 3, "RULEUTC_ProductionToggle": 23, "RULEUTC_ShieldToggle": 20, "RULEUTC_SpecialToggle": 2, "RULEUTC_StealthToggle": 10, "RULEUTC_WeaponToggle": 3}


def vocabularies(g: S.Game) -> None:
    caps, toggles = Counter(), Counter()
    for bp in g.units().values():
        caps.update(k for k, v in S.block(bp, "General", "CommandCaps").items() if v is True)
        toggles.update(k for k, v in S.block(bp, "General", "ToggleCaps").items() if v is True)
    S.expect("distinct_CommandCaps", len(caps), 21)
    S.expect("CommandCaps", dict(sorted(caps.items())), COMMAND_CAPS)
    S.expect("distinct_ToggleCaps", len(toggles), 8)
    S.expect("ToggleCaps", dict(sorted(toggles.items())), TOGGLE_CAPS)
    S.expect("blueprints_with_CommandCaps", sum(1 for bp in g.units().values() if S.block(bp, "General", "CommandCaps")), 568)


if __name__ == "__main__":
    sys.exit(S.run({"SIM-016": vocabularies}, sys.argv[1:]))
