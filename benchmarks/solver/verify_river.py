"""Independent river oracle: stdlib hand enumeration + SciPy/HiGHS matrix LP.

No upstream cards, evaluator, payoffs, CFR, or BR implementation is imported.
The declared benchmark model has OOP check/bet and IP fold/call, no raises.
"""
import argparse
import hashlib
import itertools
import json
from pathlib import Path

import numpy as np
from scipy.optimize import linprog


def cards(combo):
    return frozenset((combo[:2], combo[2:]))


def key(combo):
    return tuple(sorted(cards(combo)))


def rank5(hand):
    ranks = sorted(("23456789TJQKA".index(c[0]) + 2 for c in hand), reverse=True)
    counts = sorted(((ranks.count(r), r) for r in set(ranks)), reverse=True)
    flush = len({c[1] for c in hand}) == 1
    distinct = sorted(set(ranks), reverse=True)
    straight = distinct[0] if len(distinct) == 5 and distinct[0] - distinct[4] == 4 else 0
    if distinct == [14, 5, 4, 3, 2]:
        straight = 5
    if flush and straight:
        return (8, straight)
    if counts[0][0] == 4:
        return (7, counts[0][1], counts[1][1])
    if [n for n, _ in counts] == [3, 2]:
        return (6, counts[0][1], counts[1][1])
    if flush:
        return (5, *ranks)
    if straight:
        return (4, straight)
    if counts[0][0] == 3:
        return (3, counts[0][1], *(r for _, r in counts[1:]))
    if [n for n, _ in counts[:2]] == [2, 2]:
        return (2, counts[0][1], counts[1][1], counts[2][1])
    if counts[0][0] == 2:
        return (1, counts[0][1], *(r for _, r in counts[1:]))
    return (0, *ranks)


def showdown(board, a, b):
    va = max(rank5(h) for h in itertools.combinations([*board, *cards(a)], 5))
    vb = max(rank5(h) for h in itertools.combinations([*board, *cards(b)], 5))
    return (va > vb) - (va < vb)


def verify(report, manifest):
    data = report["input"]
    case = next(c for c in manifest["cases"] if c["caseId"] == data["caseId"])
    assert all(data[field] == value for field, value in case.items()), "engine did not preserve the preregistered problem"
    assert data["schemaVersion"] == 1 and data["limits"] == manifest["limits"] and data["locks"] == [], "benchmark execution configuration changed"
    assert report["engineCommit"] == "9d1509fe5077d019825f833eed04b16d342dfda1", "wrong engine version"
    hands = report["privateCards"]
    expected = [{key(c): w for c, w in r.items() if w > 0} for r in data["ranges"]]
    assert all({key(c) for c in hands[p]} == set(expected[p]) for p in range(2)), "private-card mapping"
    n, m = map(len, hands)
    pot = data["pot"] / 100
    bet = data["pot"] * float(data["sizes"][0][0].rstrip("%")) / 10000
    assert bet * 100 == int(bet * 100) and bet <= data["stack"] / 100
    weights = np.array([[expected[0][key(a)] * expected[1][key(b)] if cards(a).isdisjoint(cards(b)) else 0 for b in hands[1]] for a in hands[0]])
    weights /= weights.sum()
    wins = np.array([[showdown(data["board"], a, b) if weights[i, j] else 0 for j, b in enumerate(hands[1])] for i, a in enumerate(hands[0])])
    show = np.where(wins > 0, pot, np.where(wins < 0, 0, pot / 2))
    called = np.where(wins > 0, pot + bet, np.where(wins < 0, -bet, pot / 2))
    nodes = {tuple(node["path"]): node for node in report["nodes"]}
    assert set(nodes) == {(), (0,), (0, 0), (1,), (1, 0), (1, 1)}, "unexpected actual tree"
    assert nodes[()]["actions"] == ["Check", f"{'AllIn' if bet * 100 == data['stack'] else 'Bet'}({int(bet * 100)})"]
    assert nodes[(0,)]["actions"] == ["Check"] and nodes[(1,)]["actions"] == ["Fold", "Call"]
    for path, node in nodes.items():
        expected_invested = [bet * 100, bet * 100 if path == (1, 1) else 0] if path and path[0] == 1 else [0, 0]
        assert node["invested"] == expected_invested, "ledger mapping"
    root_strategy = np.array(nodes[()]["strategy"]).reshape(2, n)
    response = np.array(nodes[(1,)]["strategy"]).reshape(2, m)
    threshold = manifest["thresholds"]
    equity = np.where(wins > 0, 1.0, np.where(wins < 0, 0.0, 0.5))
    equity_error = 0.0
    for p in range(2):
        for i in range(len(hands[p])):
            conditional = weights[i, :] if p == 0 else weights[:, i]
            if conditional.sum() == 0:
                continue
            values = equity[i, :] if p == 0 else 1 - equity[:, i]
            expected_equity = float(conditional @ values / conditional.sum())
            equity_error = max(equity_error, abs(nodes[()]["equity"][p][i] - expected_equity))
    assert equity_error <= 1e-6, ("equity", equity_error)
    for node in report["nodes"]:
        if node["terminal"]:
            continue
        p = node["player"]
        strategy = np.array(node["strategy"]).reshape(len(node["actions"]), len(hands[p]))
        assert np.isfinite(strategy).all() and (strategy >= 0).all()
        assert np.max(abs(strategy.sum(axis=0) - 1)) <= threshold["frequencySum"]
    oop_pure = list(itertools.product((0, 1), repeat=n))
    ip_pure = list(itertools.product((0, 1), repeat=m))
    matrix = np.array([[sum(weights[i, j] * (show[i, j] if not x[i] else called[i, j] if y[j] else pot) for i in range(n) for j in range(m)) for y in ip_pure] for x in oop_pure])
    # Maximize the guaranteed OOP value against every pure IP strategy.
    result = linprog([*([0] * len(oop_pure)), -1], A_ub=np.column_stack((-matrix.T, np.ones(len(ip_pure)))), b_ub=np.zeros(len(ip_pure)), A_eq=[[ *([1] * len(oop_pure)), 0]], b_eq=[1], bounds=[*([(0, None)] * len(oop_pure)), (None, None)], method="highs")
    assert result.success, result.message
    value = result.x[-1]
    x = np.array([np.prod([root_strategy[action, i] for i, action in enumerate(pure)]) for pure in oop_pure])
    y = np.array([np.prod([response[action, j] for j, action in enumerate(pure)]) for pure in ip_pure])
    actual = float(x @ matrix @ y)
    br_oop = float(max(matrix @ y))
    br_ip_oop_value = float(min(x @ matrix))
    nash_conv = br_oop - br_ip_oop_value
    assert nash_conv <= threshold["nashConvPotFraction"] * pot, ("NashConv", nash_conv)
    assert abs(actual - value) <= threshold["valuePotFraction"] * pot, ("LP value", actual, value)
    # Upstream compute_exploitability is half NashConv for this unraked game.
    assert abs(report["exploitability"] / 100 * 2 - nash_conv) < 1e-5, "quality metric conversion"
    assert max(abs(report["centeredRootEv"][0] / 100 - (actual - pot / 2)), abs(report["centeredRootEv"][1] / 100 - (pot / 2 - actual))) < 1e-5, "centered root origin"
    terminal_error = 0.0
    action_error = 0.0
    # Independent counterfactual action values at both nontrivial decisions.
    for i in range(n):
        if weights[i].sum() == 0:
            continue
        expected_check = float(weights[i] @ show[i] / weights[i].sum())
        expected_bet = float(weights[i] @ ((1 - response[1]) * pot + response[1] * called[i]) / weights[i].sum())
        action_error = max(action_error, abs(nodes[()]["actionEv"][i] / 100 - expected_check), abs(nodes[()]["actionEv"][n + i] / 100 - expected_bet))
    for j in range(m):
        reach = weights[:, j] * root_strategy[1]
        if reach.sum() == 0:
            continue
        expected_call = float(reach @ (pot - called[:, j]) / reach.sum())
        action_error = max(action_error, abs(nodes[(1,)]["actionEv"][j] / 100), abs(nodes[(1,)]["actionEv"][m + j] / 100 - expected_call))
    assert action_error <= threshold["terminalEvBb"], ("action EV", action_error)
    for path, payoff, chance in [((0, 0), show, weights * root_strategy[0, :, None]), ((1, 0), np.full((n, m), pot), weights * root_strategy[1, :, None] * response[0, None, :]), ((1, 1), called, weights * root_strategy[1, :, None] * response[1, None, :])]:
        node = nodes[path]
        for p in range(2):
            for i in range(len(hands[p])):
                # EV is undefined at zero own/opponent reach. Do not compare it to zero.
                conditional = chance[i, :] if p == 0 else chance[:, i]
                if conditional.sum() <= 1e-12:
                    continue
                payoff_row = payoff[i, :] if p == 0 else pot - payoff[:, i]
                expected_ev = float(conditional @ payoff_row / conditional.sum()) + node["invested"][p] / 100
                terminal_error = max(terminal_error, abs(expected_ev - node["ev"][p][i] / 100))
    assert terminal_error <= threshold["terminalEvBb"], ("terminal EV", terminal_error)
    for p in range(2):
        native = float(np.array(nodes[()]["ev"][p]) @ np.array(nodes[()]["compatibleWeights"][p]) / sum(nodes[()]["compatibleWeights"][p]) / 100)
        assert abs(native - (actual if p == 0 else pot - actual)) < 1e-5, "root per-combo EV aggregation"
    return {"caseId": data["caseId"], "pass": True, "lpValueOopBb": value, "actualValueOopBb": actual, "nashConvBb": nash_conv, "terminalMaxErrorBb": terminal_error, "actionMaxErrorBb": action_error, "equityMaxError": equity_error, "iterations": report["iterations"], "elapsedMs": report["elapsedMs"], "stopReason": report["stopReason"], "engineTargetMet": report["stopReason"] == "quality-target", "estimatedEngineBytes": report["memoryBytes"], "inputSha256": hashlib.sha256(json.dumps(data, sort_keys=True, separators=(",", ":")).encode()).hexdigest()}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    manifest = json.loads((Path(__file__).parent / "river-cases.json").read_text())
    summaries = []
    for case in manifest["cases"]:
        report = json.loads((args.directory / f"{case['caseId']}.output.json").read_text())
        summaries.append(verify(report, manifest))
    output = {"oracle": "stdlib-7card-enumeration+scipy-highs-normal-form-lp+pure-br/v1", "thresholds": manifest["thresholds"], "results": summaries}
    (args.directory / "independent-summary.json").write_text(json.dumps(output, indent=2), encoding="utf8")
    print(json.dumps(output, indent=2))
