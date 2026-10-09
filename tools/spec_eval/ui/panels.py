#!/usr/bin/env python3
"""Evaluator for the installed HUD panel clauses of docs/specs/ui.md (HUD-005, HUD-007, HUD-020)."""

import os
import re
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import uilib as U

GAME = "lua/ui/game/"
CAPS = ["RULEUCC_Attack", "RULEUCC_CallTransport", "RULEUCC_Capture", "RULEUCC_Dive", "RULEUCC_Dock", "RULEUCC_Ferry", "RULEUCC_Guard", "RULEUCC_Move", "RULEUCC_Nuke", "RULEUCC_Overcharge", "RULEUCC_Patrol", "RULEUCC_Pause", "RULEUCC_Reclaim", "RULEUCC_Repair", "RULEUCC_RetaliateToggle", "RULEUCC_Sacrifice", "RULEUCC_SiloBuildNuke", "RULEUCC_SiloBuildTactical", "RULEUCC_Stop", "RULEUCC_Tactical", "RULEUCC_Transport"]
TOGGLES = ["RULEUTC_CloakToggle", "RULEUTC_IntelToggle", "RULEUTC_JammingToggle", "RULEUTC_ProductionToggle", "RULEUTC_ShieldToggle", "RULEUTC_SpecialToggle", "RULEUTC_StealthToggle", "RULEUTC_WeaponToggle"]
PANELS = ["borders", "buildmode", "commandmode", "construction", "controlgroups", "economy", "minimap", "orders", "rallypoint", "selection", "strategicIcons", "unitview", "worldview", "zoomslider"]
LAYOUTS = ["borders_left", "borders_mini", "construction_left", "construction_mini", "construction_right", "economy_mini", "orders_left", "orders_mini", "orders_right"]


def won_by_lua(g: U.Game, member: str) -> str:
    winner = g.index().get(member.lower())
    if winner != "lua":
        raise U.Fail(f"{member} is won by {winner}, not lua")
    return g.text("lua", member)


def orders(g: U.Game) -> None:
    text = U.strip_comments(won_by_lua(g, f"{GAME}orders.lua"))
    caps = set(re.findall(r"\bRULEUCC_\w+", text))
    toggles = set(re.findall(r"\bRULEUTC_\w+", text))
    granted_caps, granted_toggles = set(), set()
    for bp in g.members("units", "units/"):
        if bp.lower().endswith("_unit.bp"):
            body = U.strip_comments(g.text("units", bp))
            granted_caps.update(re.findall(r"\b(RULEUCC_\w+)\s*=\s*true", body))
            granted_toggles.update(re.findall(r"\b(RULEUTC_\w+)\s*=\s*true", body))
    U.expect("blueprints_grant_RULEUCC", sorted(granted_caps), CAPS)
    U.expect("blueprints_grant_RULEUTC", sorted(granted_toggles), TOGGLES)
    U.expect("orders_lua_RULEUCC_names", len(caps), 22)
    U.expect("SIM_016_capabilities_orders_lua_omits", sorted(set(CAPS) - caps), ["RULEUCC_CallTransport"])
    U.expect("orders_lua_RULEUCC_not_granted_by_a_blueprint", sorted(caps - granted_caps), ["RULEUCC_Script", "RULEUCC_Teleport"])
    U.expect("orders_lua_RULEUTC_names", len(toggles), 9)
    U.expect("SIM_016_toggles_orders_lua_omits", sorted(set(TOGGLES) - toggles), [])
    U.expect("orders_lua_RULEUTC_not_granted_by_a_blueprint", sorted(toggles - granted_toggles), ["RULEUTC_GenericToggle"])


def economy(g: U.Game) -> None:
    member = f"{GAME}economy.lua"
    text = won_by_lua(g, member)
    defined = ["CreateEconomyBar", "CreateResourceGroup", "DisplayEconData", "FormatRateString", "ToggleEconPanel"]
    missing = sorted(set(defined) - U.lua_defs(text))
    U.expect("defined_names", len(defined), 5)
    if missing:
        raise U.Fail(f"gamedata/lua.scd!{member} does not define {' '.join(missing)}")
    fields = ["GetEconomyTotals", "income", "stored", "maxStorage", "lastUseActual", "lastUseRequested"]
    absent = sorted(w for w in fields if U.word_count(text, w) == 0)
    U.expect("read_names", len(fields), 6)
    if absent:
        raise U.Fail(f"gamedata/lua.scd!{member} never mentions {' '.join(absent)}")
    U.expect("GetEconomyTotals_mentions", U.word_count(text, "GetEconomyTotals"), 1)


def panels(g: U.Game) -> None:
    members = g.members("lua", GAME)
    files = [n for n in members if not n.endswith("/")]
    layouts = [n for n in files if n.startswith(f"{GAME}layouts/")]
    U.expect("holders", dict(g.holders(GAME)), {"lua": 115})
    U.expect("members", len(members), 115)
    U.expect("directory_entries", len(members) - len(files), 2)
    U.expect("files_in_the_folder", len(files) - len(layouts), 68)
    U.expect("layout_files", len(layouts), 45)
    for name in PANELS:
        won_by_lua(g, f"{GAME}{name}.lua")
    U.expect("panels_present", len(PANELS), 14)
    for name in LAYOUTS:
        won_by_lua(g, f"{GAME}layouts/{name}.lua")
    U.expect("layouts_present", len(LAYOUTS), 9)


if __name__ == "__main__":
    sys.exit(U.run({"HUD-005": orders, "HUD-007": economy, "HUD-020": panels}, sys.argv[1:]))
