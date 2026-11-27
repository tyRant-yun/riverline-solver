"""Independent explicit-river oracle; native actions/payoffs/BR are never reused.

Tiny normal-form games only. Locked strategies impose linear constraints on own
realization, including interior information sets. SciPy HiGHS solves the game and
both constrained BRs. This is verification code, not a product solver.
"""
import argparse
import itertools
import json
from pathlib import Path

import numpy as np
from scipy.optimize import linprog
from verify_river import cards, key, showdown


def label(action):
    return {"check": "Check", "fold": "Fold", "call": "Call"}.get(action["kind"]) or (
        {"bet": "Bet", "raise": "Raise", "all-in": "AllIn"}[action["kind"]]
        + f"({action['amount']})")


def order(action):
    name = action.split("(")[0]
    return (["Fold", "Check", "Call", "Bet", "Raise", "AllIn"].index(name),
            int(action.split("(")[1][:-1]) if "(" in action else 0)


def independent_tree(data):
    """Build only the declared additions plus the mandatory passive skeleton."""
    additions = {}
    for line in data["treeLines"]:
        path = tuple(map(label, line))
        assert path and path[-1].split("(")[0] in ("Bet", "Raise", "AllIn")
        assert path[-1] not in additions.get(path[:-1], [])
        additions.setdefault(path[:-1], []).append(path[-1])
    nodes = {}

    def visit(path, history, invested, actor, last_raise, terminal=False, folded=None):
        assert len(path) <= 16
        facing = invested[1 - actor] > invested[actor]
        actions = [] if terminal else (["Fold", "Call"] if facing else ["Check"])
        actions = sorted(actions + additions.get(history, []), key=order)
        node = {"path": path, "history": history, "invested": invested, "player": None if terminal else actor,
                "actions": actions, "terminal": terminal, "folded": folded}
        nodes[path] = node
        for i, action in enumerate(actions):
            next_invested = invested.copy()
            done, folder = False, None
            increment = last_raise
            if action == "Fold":
                done, folder = True, actor
            elif action == "Call":
                next_invested[actor] = invested[1 - actor]
                done = True
            elif action == "Check":
                done = actor == 1
            else:
                name, amount = action[:-1].split("(")
                amount = int(amount)
                assert 0 < amount <= data["stack"]
                assert (name == "AllIn") == (amount == data["stack"])
                if facing:
                    assert name in ("Raise", "AllIn") and invested[1 - actor] < data["stack"]
                    assert amount > invested[1 - actor]
                    assert amount - invested[1 - actor] >= last_raise or name == "AllIn"
                else:
                    assert name in ("Bet", "AllIn")
                increment = amount - invested[1 - actor]
                next_invested[actor] = amount
            visit((*path, i), (*history, action), next_invested, 1 - actor, increment, done, folder)
    visit((), (), [0, 0], 0, 1)
    assert set(additions) <= {n["history"] for n in nodes.values()}, "orphan additions"
    return nodes


def pure_plans(nodes, player, hand_count):
    infos = [p for p, n in nodes.items() if n["player"] == player and len(n["actions"]) > 1]
    slots = [(p, h) for h in range(hand_count) for p in infos]
    count = int(np.prod([len(nodes[p]["actions"]) for p, _ in slots], dtype=object))
    assert count <= 4096, "oracle pure-plan budget (not an engine limit)"
    return slots, list(itertools.product(*(range(len(nodes[p]["actions"])) for p, _ in slots)))


def equalities(data, nodes, hands, player, slots, plans):
    rows, rhs = [np.ones(len(plans))], [1.0]
    index = {slot: i for i, slot in enumerate(slots)}
    for lock in data["locks"]:
        path = tuple(lock["path"])
        assert path in nodes and not nodes[path]["terminal"]
        if nodes[path]["player"] != player:
            continue
        for combo, probs in lock["allocations"].items():
            h = next(i for i, c in enumerate(hands[player]) if key(c) == key(combo))
            assert len(probs) == len(nodes[path]["actions"]) and abs(sum(probs) - 1) < 1e-6
            assert all(0 <= p <= 1 for p in probs)
            if (path, h) not in index:  # single forced action
                continue
            for action, probability in enumerate(probs):
                row = []
                for plan in plans:
                    own_reach = all(plan[index[(path[:k], h)]] == path[k]
                                    for k in range(len(path)) if (path[:k], h) in index)
                    row.append(float(own_reach) * ((plan[index[(path, h)]] == action) - probability))
                rows.append(row)
                rhs.append(0.0)
    return np.array(rows), np.array(rhs)


def lp_game(matrix, eq):
    # max f'q : A'x - F'q >= 0, Ex=e, x>=0; q is unrestricted.
    (e_mat, e), (f_mat, f) = eq
    size, dual = matrix.shape[0], len(f)
    result = linprog(np.r_[np.zeros(size), -f],
                     A_ub=np.c_[-matrix.T, f_mat.T], b_ub=np.zeros(matrix.shape[1]),
                     A_eq=np.c_[e_mat, np.zeros((len(e), dual))], b_eq=e,
                     bounds=[(0, None)] * size + [(None, None)] * dual, method="highs")
    assert result.success, result.message
    return -result.fun


def best_value(vector, equality, maximize):
    e_mat, e = equality
    result = linprog(-vector if maximize else vector, A_eq=e_mat, b_eq=e, bounds=(0, None), method="highs")
    assert result.success, result.message
    return -result.fun if maximize else result.fun


def verify(report, manifest):
    data = report["input"]
    case = next(c for c in manifest["cases"] if c["caseId"] == data["caseId"])
    assert data == {"schemaVersion": 2, **case, "limits": manifest["limits"]}, "frozen input changed"
    assert report["engineCommit"] == "9d1509fe5077d019825f833eed04b16d342dfda1"
    if data["locks"]:
        assert report["qualityScope"] == "fixed-constraints-half-constrained-nashconv"
    tree = independent_tree(data)
    actual = {tuple(n["path"]): n for n in report["nodes"]}
    assert len(actual) == len(report["nodes"]) and set(actual) == set(tree), "topology"
    hands = report["privateCards"]
    expected = [{key(c): w for c, w in r.items() if w > 0} for r in data["ranges"]]
    assert all({key(c) for c in hands[p]} == set(expected[p]) and len(hands[p]) == len(expected[p]) for p in range(2)), "combo mapping"
    n, m = map(len, hands)
    pot = data["pot"] / 100
    weights = np.array([[expected[0][key(a)] * expected[1][key(b)] if cards(a).isdisjoint(cards(b)) else 0
                         for b in hands[1]] for a in hands[0]])
    weights /= weights.sum()
    wins = np.array([[showdown(data["board"], a, b) if weights[i, j] else 0
                      for j, b in enumerate(hands[1])] for i, a in enumerate(hands[0])])
    strategies, reaches, payoffs = {}, {(): [np.ones(n), np.ones(m)]}, {}
    tolerance = manifest["thresholds"]
    for path, node in tree.items():
        raw = actual[path]
        assert all(raw[k] == node[k] for k in ("actions", "terminal", "player", "invested")), ("ledger/tree", path)
        if node["terminal"]:
            a, b = np.array(node["invested"]) / 100
            fraction = np.where(wins > 0, 1.0, np.where(wins < 0, 0.0, 0.5))
            if node["folded"] is not None:
                fraction = np.full((n, m), float(node["folded"] == 1))
            payoffs[path] = fraction * (pot + a + b) - a
        else:
            p = node["player"]
            strategy = np.array(raw["strategy"]).reshape(len(node["actions"]), len(hands[p]))
            assert np.isfinite(strategy).all() and (strategy >= 0).all()
            assert np.max(abs(strategy.sum(axis=0) - 1)) <= tolerance["frequencySum"]
            strategies[path] = strategy
            for action in range(len(node["actions"])):
                child = [r.copy() for r in reaches[path]]
                child[p] *= strategy[action]
                reaches[(*path, action)] = child
    for lock in data["locks"]:
        path = tuple(lock["path"])
        p = tree[path]["player"]
        for combo, probs in lock["allocations"].items():
            h = next(i for i, c in enumerate(hands[p]) if key(c) == key(combo))
            assert np.max(abs(strategies[path][:, h] - probs)) <= tolerance["frequencySum"], "locked probability column"
    for path in reversed(list(tree)):
        if path in payoffs:
            continue
        p = tree[path]["player"]
        payoffs[path] = sum(payoffs[(*path, a)] * (s[:, None] if p == 0 else s[None, :])
                            for a, s in enumerate(strategies[path]))

    slots_plans = [pure_plans(tree, p, len(hands[p])) for p in range(2)]
    eq = [equalities(data, tree, hands, p, *slots_plans[p]) for p in range(2)]
    # Pure outcomes follow independently constructed nodes and independently evaluated terminals.
    matrix = np.zeros((len(slots_plans[0][1]), len(slots_plans[1][1])))
    assert matrix.size <= 2_000_000, "oracle matrix budget"
    for i, x in enumerate(slots_plans[0][1]):
        x_lookup = dict(zip(slots_plans[0][0], x))
        for j, y in enumerate(slots_plans[1][1]):
            lookup = [x_lookup, dict(zip(slots_plans[1][0], y))]
            for h in range(n):
                for k in range(m):
                    if not weights[h, k]:
                        continue
                    path = ()
                    while not tree[path]["terminal"]:
                        p = tree[path]["player"]
                        action = lookup[p].get((path, h if p == 0 else k), 0)
                        path = (*path, action)
                    matrix[i, j] += weights[h, k] * payoffs[path][h, k]
    mixed = []
    for p, (slots, plans) in enumerate(slots_plans):
        values = np.array([np.prod([strategies[path][a, h] for (path, h), a in zip(slots, plan)]) for plan in plans])
        assert np.max(abs(eq[p][0] @ values - eq[p][1])) <= tolerance["frequencySum"], "lock realization"
        mixed.append(values)
    x, y = mixed
    value = lp_game(matrix, eq)
    current = float(x @ matrix @ y)
    constrained = best_value(matrix @ y, eq[0], True) - best_value(x @ matrix, eq[1], False)
    unconstrained = float(max(matrix @ y) - min(x @ matrix))
    assert constrained <= tolerance["nashConvPotFraction"] * pot, ("constrained NashConv", constrained)
    assert abs(current - value) <= tolerance["valuePotFraction"] * pot, ("LP value", current, value)
    assert abs(report["exploitability"] / 50 - constrained) <= tolerance["metricBb"], "constrained metric conversion"
    assert abs(float((payoffs[()] * weights).sum()) - current) <= tolerance["evBb"]
    assert max(abs(report["centeredRootEv"][p] / 100 - ([current, pot - current][p] - pot / 2)) for p in range(2)) <= tolerance["evBb"], "root origin"

    errors = {"ev": 0.0, "actionEv": 0.0, "equity": 0.0, "conditionalWeights": 0.0}
    checked = 0
    for path, node in tree.items():
        raw = actual[path]
        chance = weights * reaches[path][0][:, None] * reaches[path][1][None, :]
        for p in range(2):
            marginal = chance.sum(axis=1 if p == 0 else 0)
            native_weights = np.array(raw["compatibleWeights"][p])
            if marginal.sum() > 1e-12:
                assert native_weights.sum() > 0
                errors["conditionalWeights"] = max(errors["conditionalWeights"], float(max(abs(marginal / marginal.sum() - native_weights / native_weights.sum()))))
            for h in range(len(hands[p])):
                distribution = chance[h, :] if p == 0 else chance[:, h]
                if distribution.sum() <= 1e-12:
                    continue
                distribution = distribution / distribution.sum()
                origin = node["invested"][p] / 100
                payoff = payoffs[path][h, :] if p == 0 else pot - payoffs[path][:, h]
                ev = float(distribution @ payoff) + origin
                errors["ev"] = max(errors["ev"], abs(ev - raw["ev"][p][h] / 100))
                equity = np.where(wins[h, :] > 0, 1, np.where(wins[h, :] < 0, 0, 0.5)) if p == 0 else np.where(wins[:, h] < 0, 1, np.where(wins[:, h] > 0, 0, 0.5))
                errors["equity"] = max(errors["equity"], abs(float(distribution @ equity) - raw["equity"][p][h]))
                if node["player"] == p:
                    for a in range(len(node["actions"])):
                        child = payoffs[(*path, a)]
                        payoff = child[h, :] if p == 0 else pot - child[:, h]
                        ev = float(distribution @ payoff) + origin
                        errors["actionEv"] = max(errors["actionEv"], abs(ev - raw["actionEv"][a * len(hands[p]) + h] / 100))
                checked += 1
    assert max(errors["ev"], errors["actionEv"]) <= tolerance["evBb"], errors
    assert max(errors["equity"], errors["conditionalWeights"]) <= 1e-6, errors
    return {"caseId": data["caseId"], "pass": True, "nodes": len(tree), "purePlanCounts": list(matrix.shape),
            "lpValueOopBb": value, "actualValueOopBb": current, "constrainedNashConvBb": constrained,
            "unconstrainedNashConvBb": unconstrained, "locks": len(data["locks"]), "errors": errors,
            "checkedComboNodes": checked, "iterations": report["iterations"], "stopReason": report["stopReason"],
            "elapsedMs": report["elapsedMs"], "estimatedEngineBytes": report["memoryBytes"]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    manifest = json.loads((Path(__file__).parent / "river-tree-cases.json").read_text())
    results = [verify(json.loads((args.directory / f"{c['caseId']}.output.json").read_text()), manifest) for c in manifest["cases"]]
    summary = {"oracle": "independent-tree+stdlib-hand-enumeration+constrained-normal-form-HiGHS/v1", "thresholds": manifest["thresholds"], "results": results}
    (args.directory / "independent-summary.json").write_text(json.dumps(summary, indent=2), encoding="utf8")
    print(json.dumps(summary, indent=2))
