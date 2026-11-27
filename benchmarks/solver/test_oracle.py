"""Checks oracle rules and proves that corrupted engine evidence is rejected."""
import copy
import json
import unittest
from pathlib import Path

from verify_river import rank5, showdown, verify


class OracleTests(unittest.TestCase):
    def test_categories_and_wheel(self):
        examples = [
            ["Ac", "Jd", "9h", "6s", "3c"],
            ["Ac", "Ad", "9h", "6s", "3c"],
            ["Ac", "Ad", "9h", "9s", "3c"],
            ["Ac", "Ad", "Ah", "6s", "3c"],
            ["Ac", "2d", "3h", "4s", "5c"],
            ["Ac", "Jc", "9c", "6c", "3c"],
            ["Ac", "Ad", "Ah", "6s", "6c"],
            ["Ac", "Ad", "Ah", "As", "3c"],
            ["Tc", "Jc", "Qc", "Kc", "Ac"],
        ]
        self.assertEqual([rank5(h)[0] for h in examples], list(range(9)))
        self.assertEqual(rank5(examples[4]), (4, 5))
        self.assertEqual(showdown(["Tc", "Jc", "Qc", "Kc", "Ac"], "2d2h", "3d3h"), 0)

    def test_rejects_ev_quality_strategy_and_tree_corruption(self):
        root = Path(__file__).resolve().parents[2]
        manifest = json.loads((Path(__file__).parent / "river-cases.json").read_text())
        report = json.loads((root / ".artifacts/solver/r1/river-polar-pot.output.json").read_text())
        corruptions = [
            lambda r: r["nodes"][0]["strategy"].__setitem__(0, -0.1),
            lambda r: r["nodes"][0]["actionEv"].__setitem__(0, 9999),
            lambda r: r.__setitem__("exploitability", 9999),
            lambda r: r["nodes"].pop(),
            lambda r: r["privateCards"][0].__setitem__(0, "2cAc"),
            lambda r: r["input"].__setitem__("pot", 999),
        ]
        for mutate in corruptions:
            with self.subTest(mutation=mutate):
                changed = copy.deepcopy(report)
                mutate(changed)
                with self.assertRaises(AssertionError):
                    verify(changed, manifest)


if __name__ == "__main__":
    unittest.main()
