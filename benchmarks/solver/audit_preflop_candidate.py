"""Audit downloaded CC-BY push/fold data. Reconstruct compatibility by enumerating
physical hole cards; recompute action EV and two-player BR gap from the separate
equity file. This validates internal math, not the exactness of the equity source.
"""
import itertools
import json
import hashlib
import random
import sys
from pathlib import Path
import numpy as np
from verify_river import showdown

def combos(label):
    ranks=label[:2].upper();suits='cdhs'
    return [frozenset([ranks[0]+a,ranks[1]+b]) for a in suits for b in suits
            if (ranks[0]!=ranks[1] or a<b) and (len(label)==2 or (a==b if label[2]=='s' else a!=b))]

def audit(root):
    push_doc=json.loads((root/'push-fold-heads-up.json').read_text())
    eq_doc=json.loads((root/'preflop-equity.json').read_text())
    assert push_doc['license']==eq_doc['license']=='CC BY 4.0'
    p=push_doc['data'];q=eq_doc['data'];assert p['classes']==q['classes'] and len(p['classes'])==169
    hands=[combos(c) for c in p['classes']]
    assert sum(map(len,hands))==1326
    compat=np.array([[sum(a.isdisjoint(b) for a in x for b in y) for y in hands] for x in hands],dtype=float)
    assert compat.sum()==1326*1225 and np.all(compat==compat.T)
    matrix=np.array(q['matrix']);assert matrix.shape==(169,169,3)
    assert np.max(abs(matrix.sum(axis=2)-1))<0.00002
    equity=matrix[:,:,0]+matrix[:,:,1]/2
    assert np.max(abs(equity+equity.T-1))<0.00002
    probabilities=np.array([len(h)/1326 for h in hands]);denom=compat.sum(axis=1)
    rows=[];max_ev_error=0
    for spot in p['spots']:
        S=spot['stack'];push=np.array(spot['push']);call=np.array(spot['call'])
        assert push.shape==call.shape==(169,) and np.all((push>=0)&(push<=1)) and np.all((call>=0)&(call<=1))
        shove=(compat*((1-call)[None,:]+call[None,:]*(S*(2*equity-1)))).sum(axis=1)/denom
        reach=(compat*push[:,None]).sum(axis=0)
        callable=reach>0
        call_value=np.divide((compat*push[:,None]*S*(1-2*equity)).sum(axis=0),reach,out=np.zeros(169),where=callable)
        ev_error=max(float(np.max(abs(shove-np.array(spot['sbPushEv'])))),float(np.max(abs(call_value[callable]-np.array(spot['bbCallEv'],dtype=float)[callable]))))
        max_ev_error=max(max_ev_error,ev_error)
        value=float(probabilities@((1-push)*(-0.5)+push*shove))
        sb_gain=float(probabilities@(np.maximum(-0.5,shove)-((1-push)*(-0.5)+push*shove)))
        bb_gain=float(np.sum(probabilities*(reach/denom)*(np.maximum(-1,call_value)-((1-call)*(-1)+call*call_value))))
        gap=sb_gain+bb_gain
        # Source arrays rounded to 5 decimals: 2*S*5e-6 plus final EV rounding.
        assert ev_error <= 2*S*0.000005+0.000006,(S,ev_error)
        assert abs(value-spot['sbValue'])<2*S*0.000005+0.000006
        assert gap<=0.001+4*S*0.000005,(S,gap)
        rows.append({'stackBb':S,'recomputedNashConvBb':gap,'actionEvMaxDifferenceBb':ev_error,'sbValueBb':value})
    assert len(rows)==39
    rng=random.Random(20261127)
    samples=[]
    deck=[r+s for r in '23456789TJQKA' for s in 'cdhs']
    for left,right in [('aa','kk'),('aks','qq'),('76s','ako')]:
        i,j=p['classes'].index(left),p['classes'].index(right)
        pairs=[(a,b) for a in hands[i] for b in hands[j] if a.isdisjoint(b)]
        total=0;n=10000
        for _ in range(n):
            a,b=rng.choice(pairs)
            board=rng.sample([c for c in deck if c not in a|b],5)
            ca,cb=''.join(sorted(a)),''.join(sorted(b))
            total+=(showdown(board,ca,cb)+1)/2
        observed=total/n;delta=abs(observed-equity[i,j])
        assert delta<0.025,(left,right,observed,equity[i,j])
        samples.append({'hero':left,'villain':right,'samples':n,'sourceEquity':float(equity[i,j]),'independentSampleEquity':observed,'absoluteDifference':delta,'threshold':0.025})
    output={'source':'https://georgelipoker.com/data','credit':push_doc['credit'],'license':push_doc['license'],
      'rawFiles':[{'name':f.name,'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'bytes':f.stat().st_size} for f in [root/'push-fold-heads-up.json',root/'preflop-equity.json']],
      'checks':'independent physical-card compatibility + rounded matrix action EV + BR gap',
      'maxActionEvDifferenceBb':max_ev_error,'maxRecomputedNashConvBb':max(r['recomputedNashConvBb'] for r in rows),
      'status':'limited-model-consistency-passed','eligibleForCurrent100BbPractice':False,
      'limits':['HU shove/fold only, 1-20BB, no ante','Equity source exactness not independently certified','Not a 100BB raise/limp/call strategy, not installed as practice scoring reference'],
      'independentEquitySpotChecks':samples,
      'results':rows}
    (root/'candidate-audit.json').write_text(json.dumps(output,indent=2))
    print(json.dumps({k:v for k,v in output.items() if k!='results'},indent=2))

if __name__=='__main__':audit(Path(sys.argv[1]))
