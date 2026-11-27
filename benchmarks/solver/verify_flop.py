"""AF04 independent oracle. Exact unseen two-card enumeration; normal-form LP/BR
for the flop-only wager and perfect-information backward induction for a fixed
private-pair three-street game. This is validation code, not a product solver.
"""
import functools
import itertools
import json
import sys
from pathlib import Path
import numpy as np
from scipy.optimize import linprog
from verify_river import cards, key, showdown

DECK = [r+s for r in '23456789TJQKA' for s in 'cdhs']

@functools.lru_cache(None)
def equity(board, a, b):
    remaining = [c for c in DECK if c not in board and c not in cards(a) | cards(b)]
    results = [showdown([*board, *runout], a, b) for runout in itertools.combinations(remaining, 5-len(board))]
    return (sum(results)/len(results)+1)/2

def verify_small(report, thresholds):
    d = report['input']; hands = report['privateCards']; n,m=map(len,hands); pot=d['pot']/100; bet=d['flopBets'][0]/100
    weights=np.array([[d['ranges'][0][next(c for c in d['ranges'][0] if key(c)==key(a))]*d['ranges'][1][next(c for c in d['ranges'][1] if key(c)==key(b))] if cards(a).isdisjoint(cards(b)) else 0 for b in hands[1]] for a in hands[0]])
    weights/=weights.sum()
    eq=np.array([[equity(tuple(d['board']),a,b) if weights[i,j] else 0 for j,b in enumerate(hands[1])] for i,a in enumerate(hands[0])])
    root=report['sampleNodes'][0]
    eqerr=0
    for p in range(2):
        for i in range(len(hands[p])):
            w=weights[i] if p==0 else weights[:,i]
            v=eq[i] if p==0 else 1-eq[:,i]
            if w.sum(): eqerr=max(eqerr,abs(root['equity'][p][i]-w@v/w.sum()))
    assert eqerr<thresholds['equity'], ('equity',eqerr)
    show=pot*eq
    if not bet:
        err=max(abs(root['ev'][0][i]/100-weights[i]@show[i]/weights[i].sum()) for i in range(n) if weights[i].sum())
        assert err<thresholds['rootValueBb'],err
        return {'caseId':d['caseId'],'equityMaxError':eqerr,'valueMaxErrorBb':err}
    called=(pot+2*bet)*eq-bet
    x_pure=list(itertools.product((0,1),repeat=n));y_pure=list(itertools.product((0,1),repeat=m))
    matrix=np.array([[sum(weights[i,j]*(show[i,j] if not x[i] else called[i,j] if y[j] else pot) for i in range(n) for j in range(m)) for y in y_pure] for x in x_pure])
    lp=linprog([*([0]*len(x_pure)),-1],A_ub=np.column_stack((-matrix.T,np.ones(len(y_pure)))),b_ub=np.zeros(len(y_pure)),A_eq=[[*([1]*len(x_pure)),0]],b_eq=[1],bounds=[*([(0,None)]*len(x_pure)),(None,None)],method='highs')
    assert lp.success,lp.message
    xs=np.array(root['strategy']).reshape(2,n);ys=np.array(report['sampleNodes'][1]['strategy']).reshape(2,m)
    x=np.array([np.prod([xs[a,i] for i,a in enumerate(p)]) for p in x_pure]);y=np.array([np.prod([ys[a,i] for i,a in enumerate(p)]) for p in y_pure])
    conv=float(max(matrix@y)-min(x@matrix));v=float(x@matrix@y)
    assert conv<thresholds['nashConvBb'],conv
    assert abs(v-lp.x[-1])<thresholds['rootValueBb'],(v,lp.x[-1])
    err=0
    for i in range(n):
        if weights[i].sum():
            for action,value in enumerate([weights[i]@show[i]/weights[i].sum(),weights[i]@((1-ys[1])*pot+ys[1]*called[i])/weights[i].sum()]):
                err=max(err,abs(root['actionEv'][action*n+i]/100-value))
    assert err<thresholds['actionEvBb'],err
    return {'caseId':d['caseId'],'lpValueBb':float(lp.x[-1]),'actualValueBb':v,'nashConvBb':conv,'actionEvMaxErrorBb':float(err),'equityMaxError':eqerr}

def verify_three_street(report, thresholds):
    d=report['input']; a,b=[h[0] for h in report['privateCards']];pot=d['pot']/100
    @functools.lru_cache(None)
    def win(board): return showdown(board,a,b)
    @functools.lru_cache(None)
    def visit(board,inv,street,actor,passed):
        facing=street[1-actor]>street[actor]
        def finish(total,folder=None):
            if folder is not None:return 0-total[0] if folder==0 else pot+total[1]
            if len(board)==5 or max(total)==d['stack']/100:
                e=equity(board,a,b) if len(board)<5 else (win(board)+1)/2
                return e*(pot+sum(total))-total[0]
            remaining=[c for c in DECK if c not in board and c not in cards(a)|cards(b)]
            return sum(visit((*board,c),total,(0,0),0,False) for c in remaining)/len(remaining)
        values=[]
        if facing:
            values.append(finish(inv,actor))
            total=list(inv);total[actor]+=street[1-actor]-street[actor];values.append(finish(tuple(total)))
        else:
            values.append(finish(inv) if passed else visit(board,inv,street,1-actor,True))
            bet=d[['flopBets','turnBets','riverBets'][len(board)-3]][actor]/100
            if bet:
                total=list(inv);total[actor]+=bet;nextstreet=list(street);nextstreet[actor]+=bet
                values.append(visit(board,tuple(total),tuple(nextstreet),1-actor,False))
        return (max if actor==0 else min)(values)
    expected=visit(tuple(d['board']),(0,0),(0,0),0,False)
    actual=report['sampleNodes'][0]['ev'][0][0]/100
    error=abs(expected-actual)
    assert error<thresholds['rootValueBb'],(expected,actual,error)
    return {'caseId':d['caseId'],'independentBackwardInductionBb':expected,'actualValueBb':actual,'valueErrorBb':error,'oracleStates':visit.cache_info().currsize}

if __name__=='__main__':
    root=Path(sys.argv[1]);manifest=json.loads(Path('benchmarks/solver/flop-cases.json').read_text())
    output=[]
    for case in manifest['cases']:
        report=json.loads((root/(case['caseId']+'.output.json')).read_text())
        assert report['input']=={**case,'limits':manifest['limits']}
        assert report['engineCommit']=='9d1509fe5077d019825f833eed04b16d342dfda1'
        result=(verify_three_street if case['caseId'].startswith('F04') else verify_small)(report,manifest['thresholds'])
        result.update({'iterations':report['iterations'],'elapsedMs':report['elapsedMs'],'memoryBytes':report['memoryBytes'],'stopReason':report['stopReason']})
        output.append(result);print(json.dumps(result),flush=True)
    (root/'independent-summary.json').write_text(json.dumps({'oracle':'independent-7card-enumeration+LP+BR+fixed-pair-backward-induction/1','thresholds':manifest['thresholds'],'results':output},indent=2))
