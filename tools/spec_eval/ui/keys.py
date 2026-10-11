#!/usr/bin/env python3
"""Evaluator for the key-action clauses of docs/specs/ui.md (HUD-016 to HUD-018)."""

import os
import re
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import uilib as U

CATEGORIES = ["ui", "selection", "camera", "orders", "game", "chat", "debug", "user", "none"]
ZOOM = ["zoom_in", "zoom_in_fast", "zoom_out", "zoom_out_fast"]


def tables(g: U.Game) -> tuple[dict, dict, dict, dict]:
    actions = U.key_actions(g, "keyActions", "debugKeyActions")
    debug_actions = U.key_actions(g, "debugKeyActions", None)
    bindings = U.key_bindings(g, "defaultKeyMap", "debugKeyMap")
    debug_bindings = U.key_bindings(g, "debugKeyMap", None)
    return actions, debug_actions, bindings, debug_bindings


def actions(g: U.Game) -> None:
    acts, debug, _, _ = tables(g)
    U.expect("keyActions", len(acts), 135)
    U.expect("debugKeyActions", len(debug), 44)
    U.expect("top_level_actions", len(acts) + len(debug), 179)
    U.expect("keyActions_by_category", dict(sorted(Counter(acts.values()).items())), {"camera": 12, "chat": 4, "game": 3, "orders": 32, "selection": 59, "ui": 25})
    U.expect("debugKeyActions_by_category", dict(sorted(Counter(debug.values()).items())), {"debug": 43, "game": 1})
    text = U.strip_comments(g.text("lua", "lua/keymap/keycategories.lua"))
    named = re.findall(r"^\s*\['([^']+)'\]", U.table_section(text, "keyCategories", "keyCategoryOrder"), re.M)
    U.expect("keyCategories", named, CATEGORIES)
    U.expect("actions_outside_the_categories", sorted(a for a, c in {**acts, **debug}.items() if c not in CATEGORIES), [])


def bindings(g: U.Game) -> None:
    acts, debug, keys, debug_keys = tables(g)
    U.expect("defaultKeyMap", len(keys), 135)
    U.expect("debugKeyMap", len(debug_keys), 44)
    U.expect("default_bindings", len(keys) + len(debug_keys), 179)
    U.expect("default_bindings_naming_an_undefined_action", sorted(a for a in keys.values() if a not in acts), [])
    U.expect("debug_bindings_naming_an_undefined_action", sorted(a for a in debug_keys.values() if a not in debug), [])
    U.expect("actions_without_a_default_binding", sorted(set(acts) - set(keys.values())), [])
    U.expect("debug_actions_without_a_default_binding", sorted(set(debug) - set(debug_keys.values())), [])
    U.expect("actions_bound_more_than_once", sorted(a for a, n in Counter(keys.values()).items() if n > 1), [])
    U.expect("key_strings_in_both_maps", sorted(set(keys) & set(debug_keys)), [])


def families(g: U.Game) -> None:
    acts, _, keys, _ = tables(g)
    groups = [f"{stem}{digit}" for stem in ("group", "set_group", "append_group", "fac_group") for digit in range(10)]
    U.expect("control_group_actions", sum(acts.get(a) == "selection" for a in groups), 40)
    U.expect("zoom_actions_in_camera", sorted(a for a in ZOOM if acts.get(a) == "camera"), ZOOM)
    orders = sorted(a for a, c in acts.items() if c == "orders")
    shifted = [a for a in orders if a.startswith("shift_")]
    U.expect("order_actions", len(orders), 32)
    U.expect("order_actions_base", len(orders) - len(shifted), 18)
    U.expect("order_actions_shift", len(shifted), 14)
    U.expect("shift_actions_without_a_base_action", [a for a in shifted if acts.get(a[len("shift_") :]) != "orders"], [])
    by_action = {a: k for k, a in keys.items()}
    U.expect("shift_actions_bound_to_a_Shift_key", sum(by_action[a].startswith("Shift-") for a in shifted), 14)


if __name__ == "__main__":
    sys.exit(U.run({"HUD-016": actions, "HUD-017": bindings, "HUD-018": families}, sys.argv[1:]))
