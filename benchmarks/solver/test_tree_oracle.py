"""Self-tests and mutations for the independent river-tree oracle."""
import copy
import json
import unittest
from pathlib import Path
import numpy as np
from verify_river_tree import independent_tree, equalities, lp_game, best_value, verify

HERE = Path(__file__).parent
ROOT = HERE.resolve().parents[1]


class TreeOracleTests(unittest.TestCase):
    def test_lp_known_matching_pennies_and_forced_mix(self):
        matrix = np.array([[1., -1.], [-1., 1.]])
        ordinary = (np.ones((1, 2)), np.array([1.]))
        locked = (np.array([[1., 1.], [1., 0.]]), np.array([1., .75]))
        self.assertAlmostEqual(lp_game(matrix, [ordinary, ordinary]), 0.)
        self.assertAlmostEqual(lp_game(matrix, [locked, ordinary]), -.5)
        self.assertAlmostEqual(best_value(np.array([1., -1.]), locked, True), .5)

    def test_interior_constraint_conditions_on_own_sequence(self):
        # A player's earlier choice determines whether its later lock is binding.
        nodes = {(): {"player": 0, "actions": ["Check", "Bet(500)"], "terminal": False},
                 (1,): {"player": 0, "actions": ["Fold", "Call"], "terminal": False}}
        data = {"locks": [{"path": [1], "allocations": {"AcAd": [.25, .75]}}]}
        plans = [(0, 0), (0, 1), (1, 0), (1, 1)]
        e_mat, rhs = equalities(data, nodes, [["AcAd"], []], 0, [((), 0), ((1,), 0)], plans)
        valid = np.array([.5, 0, .125, .375])
        np.testing.assert_allclose(e_mat @ valid, rhs)
        # A marginal .25 fold at the later node would be the wrong rule.
        self.assertGreater(float(max(abs(e_mat @ np.array([.5, 0, .25, .25]) - rhs))), .1)

    def test_tree_and_short_allin_payments(self):
        manifest = json.loads((HERE / "river-tree-cases.json").read_text())
        case = next(c for c in manifest["cases"] if c["caseId"] == "river-short-allin-tie")
        tree = independent_tree(case)
        self.assertEqual(tree[(1, 2)]["actions"], ["Fold", "Call"])
        self.assertEqual(tree[(1, 2, 1)]["invested"], [800, 800])
        self.assertEqual(tree[(1, 2, 0)]["invested"], [500, 800])

    def test_mutated_evidence_is_rejected(self):
        manifest = json.loads((HERE / "river-tree-cases.json").read_text())
        report = json.loads((ROOT / ".artifacts/solver/river-tree/river-root-mixed-lock.output.json").read_text())
        mutations = [
            lambda r: r["nodes"][0]["strategy"].__setitem__(0, -.1),
            lambda r: r["nodes"][0]["actionEv"].__setitem__(0, 9999),
            lambda r: r["nodes"][1]["invested"].__setitem__(0, 1),
            lambda r: r["nodes"].pop(),
            lambda r: r.__setitem__("exploitability", 999),
            lambda r: r["input"].__setitem__("stack", 999),
            lambda r: r["privateCards"][0].append("QhQd"),
            lambda r: r["nodes"][0]["compatibleWeights"][0].__setitem__(0, 999),
        ]
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                changed = copy.deepcopy(report)
                mutate(changed)
                with self.assertRaises(AssertionError):
                    verify(changed, manifest)

    def test_locked_quality_is_distinct_from_original_game(self):
        manifest = json.loads((HERE / "river-tree-cases.json").read_text())
        report = json.loads((ROOT / ".artifacts/solver/river-tree/river-root-mixed-lock.output.json").read_text())
        result = verify(report, manifest)
        self.assertLess(result["constrainedNashConvBb"], .001)
        self.assertGreater(result["unconstrainedNashConvBb"], 1.)


if __name__ == "__main__":
    unittest.main()
