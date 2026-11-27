"""Independent mid-size checkdown enumeration; no solver evaluator or tree imported."""
import hashlib
import itertools
import json
from pathlib import Path
from verify_river import rank5, cards

root=Path('.artifacts/solver/m1-capacity')
definition=json.loads(Path('benchmarks/solver/m1-capacity-definition.json').read_text())
manifest=json.loads((root/'frozen-manifest.json').read_text())
results=[]
for c in manifest['cases']:
    p=root/(c['caseId']+'.json');report=json.loads(p.read_text());data=report['input']
    assert data=={'schemaVersion':4,**c,'limits':manifest['limits']}
    assert report['stopReason']=='quality-target'
    assert 2*report['exploitability']/data['pot']<=0.001
    assert [len(v)for v in report['privateCards']]==[len(v)for v in c['ranges']]
    assert report['memoryBytes']<=268435456 and report['elapsedMs']<=120000
    results.append({'caseId':c['caseId'],'iterations':report['iterations'],'nashConvPotFraction':2*report['exploitability']/data['pot'],
                    'computeMs':report['elapsedMs'],'estimatedBytes':report['memoryBytes'],'nativeBytes':p.with_suffix('.json.native.bin').stat().st_size,
                    'inputSha256':hashlib.sha256((root/(c['caseId']+'.input.json')).read_bytes()).hexdigest()})

r=json.loads((root/'m1-checkdown-64.json').read_text());data=r['input'];hands=r['privateCards'];board=data['board']
deck=[rank+suit for rank in '23456789TJQKA' for suit in 'cdhs' if rank+suit not in board]
weights=[{frozenset(cards(c)):w for c,w in ran.items()}for ran in data['ranges']]
# Each legal river/hand is evaluated once by independent stdlib five-card enumeration.
ranks={}
for p in (0,1):
    for h in hands[p]:
        for river in deck:
            if river not in cards(h):
                ranks[(h,river)]=max(rank5(five)for five in itertools.combinations([*board,river,*cards(h)],5))
err_eq=err_ev=0.0
for p in (0,1):
    for i,h in enumerate(hands[p]):
        total=win=0.0
        for other in hands[1-p]:
            used=cards(h)|cards(other)
            if len(used)!=4:continue
            w=weights[1-p][cards(other)]
            total+=w
            for river in deck:
                if river in used:continue
                a,b=ranks[(h,river)],ranks[(other,river)]
                win+=w*(1.0 if a>b else .5 if a==b else 0.0)/44
        eq=win/total
        err_eq=max(err_eq,abs(r['root']['equity'][p][i]-eq))
        err_ev=max(err_ev,abs(r['root']['ev'][p][i]/100-eq*data['pot']/100))
assert err_eq<=definition['oracleThresholds']['equity'],err_eq
assert err_ev<=definition['oracleThresholds']['evBb'],err_ev
out={'model':'m1-turn-capacity/1','runs':results,'independentCheckdown':{'combos':[64,64],'equityMaxError':err_eq,'evBbMaxError':err_ev,'evaluator':'stdlib rank5, exact compatible pair and 44-river enumeration'}}
(root/'verified.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps(out,indent=2))
