import unittest
import numpy as np
from scipy import sparse
from verify_turn import fraction, independent_tree, sequence_system, lp_value, br

class TurnOracleTests(unittest.TestCase):
    def data(self):
        return {"board":["2c","3d","7h","9s"],"pot":1000,"stack":2000,"turnBets":[500,0],"riverBets":[0,700]}
    def test_known_checkdown_outs(self):
        self.assertAlmostEqual(fraction(tuple(self.data()["board"]),"AcAd","KhKd"),42/44)
    def test_sequence_lp_and_br_known_game(self):
        e=sparse.csr_matrix([[1,0,0],[-1,1,1]])
        matrix=sparse.csr_matrix([[0,0,0],[0,1,-1],[0,-1,1]])
        eq=(e,np.array([1,0]))
        self.assertAlmostEqual(lp_value(matrix,[eq,eq]),0)
        self.assertAlmostEqual(br(matrix@np.array([1,.5,.5]),eq,True),0)
        self.assertAlmostEqual(br(matrix@np.array([1,1,0]),eq,True),1)
    def test_no_future_river_at_turn_information_set(self):
        hands=[["AcAd"],["KhKd"]];nodes,possible=independent_tree(self.data(),hands)
        self.assertEqual(len(possible),44)
        seq=sequence_system(nodes,hands,0)[0]
        self.assertEqual(sum(path==() for path,h,a in seq),2)
        self.assertTrue(any(any(step[0]=="deal" for step in path) for path,h,a in seq))
    def test_allin_terminal_integrates_river_instead_of_future_decisions(self):
        data=self.data();data.update(stack=500,turnBets=[500,0],riverBets=[0,0])
        nodes,_=independent_tree(data,[["AcAd"],["KhKd"]])
        node=nodes[(("action",1),("action",1))]
        self.assertTrue(node["terminal"]);self.assertEqual(len(node["board"]),4)
        self.assertAlmostEqual(fraction(tuple(node["board"]),"AcAd","KhKd"),42/44)

if __name__=="__main__": unittest.main()
