"""Independent two-street oracle: public tree, exact chance, sequence-flow LP/BR.

No upstream evaluator, tree, chance, payoff or best-response routine is imported.
Turn information sets never contain the future river. This is bounded validation
code, not an application solver. Existing independent seven-card evaluator reused.
"""
import functools
import hashlib
import json
import sys
from pathlib import Path
import numpy as np
from scipy.optimize import linprog
from scipy import sparse
from verify_river import cards, key, showdown

DECK = [r+s for r in "23456789TJQKA" for s in "cdhs"]

def path_key(path):
    return tuple((s["kind"], s["index"] if s["kind"] == "action" else s["card"]) for s in path)

def independent_tree(data, hands):
    possible = [r for r in DECK if r not in data["board"] and any(
        cards(a).isdisjoint(cards(b)) and r not in cards(a) | cards(b) for a in hands[0] for b in hands[1])]
    nodes = {}
    def visit(path, board, invested, street_bets, actor, done=False, folder=None):
        facing = street_bets[1-actor] > street_bets[actor]
        chance = done and folder is None and len(board) == 4 and max(invested) < data["stack"]
        terminal = done and not chance
        options = data["turnBets"] if len(board) == 4 else data["riverBets"]
        actions = [] if done else (["Fold", "Call"] if facing else ["Check"])
        if not done and not facing and options[actor]:
            size = options[actor]
            assert size <= data["stack"] - invested[actor], "no hidden size clamping"
            actions.append(f"{'AllIn' if size == data['stack']-invested[actor] else 'Bet'}({size})")
        nodes[path] = {"kind":"chance" if chance else "terminal" if terminal else "decision", "terminal":terminal,
                       "player":None if done else actor,"actions":actions,"invested":invested,"board":board,"folder":folder}
        if chance:
            for r in possible:
                visit((*path,("deal",r)), [*board,r], invested.copy(),[0,0],0,
                      done=max(invested)==data["stack"])
        for i,action in enumerate(actions):
            total, street = invested.copy(), street_bets.copy()
            next_done, folded = False, None
            if action == "Fold": next_done,folded=True,actor
            elif action == "Call":
                total[actor] += street[1-actor]-street[actor]
                street[actor]=street[1-actor]
                next_done=True
            elif action == "Check": next_done=actor==1
            else:
                amount=int(action.split("(")[1][:-1]);total[actor]+=amount;street[actor]=amount
            visit((*path,("action",i)),board,total,street,1-actor,next_done,folded)
    visit((),data["board"],[0,0],[0,0],0)
    return nodes,possible

@functools.lru_cache(maxsize=50000)
def fraction(board,a,b):
    if not cards(a).isdisjoint(cards(b)) or (cards(a)|cards(b)) & set(board): return 0.0
    if len(board)==5: return (showdown(board,a,b)+1)/2
    return sum(fraction((*board,r),a,b) for r in DECK if r not in set(board)|cards(a)|cards(b))/44

def sequence_system(nodes, hands, player):
    sequences = {}; infos = []; terminal_ids = {}; parents={}
    def walk(path, own):
        node=nodes[path]
        if node["terminal"]: terminal_ids[path]=own.copy();return
        if node["kind"]=="chance":
            for child in nodes:
                if len(child)==len(path)+1 and child[:-1]==path: walk(child,own)
            return
        for action in range(len(node["actions"])):
            nxt=own.copy()
            if node["player"]==player:
                for h in range(len(hands[player])):
                    seq=(path,h,action)
                    if seq not in sequences: sequences[seq]=len(sequences)+1
                    nxt[h]=sequences[seq]
                    if action==0: infos.append((path,h,own[h]))
                    parents[(path,h)]=own[h]
            walk((*path,("action",action)),nxt)
    walk((),[0]*len(hands[player]))
    size=len(sequences)+1; e=sparse.lil_matrix((len(infos)+1,size));rhs=np.zeros(len(infos)+1)
    e[0,0]=1;rhs[0]=1
    for row,(path,h,parent) in enumerate(infos,1):
        e[row,parent]=-1
        for a in range(len(nodes[path]["actions"])): e[row,sequences[(path,h,a)]]=1
    return sequences,e.tocsr(),rhs,terminal_ids,parents

def lp_value(matrix, eq):
    (e,rhs),(f,target)=eq;nx=matrix.shape[0]
    result=linprog(np.r_[np.zeros(nx),-target],A_ub=sparse.hstack([-matrix.T,f.T]),b_ub=np.zeros(matrix.shape[1]),
                   A_eq=sparse.hstack([e,sparse.csr_matrix((len(rhs),len(target)))]),b_eq=rhs,
                   bounds=[(0,None)]*nx+[(None,None)]*len(target),method="highs")
    assert result.success,result.message
    return -result.fun

def br(vector,eq,maximize):
    result=linprog(-vector if maximize else vector,A_eq=eq[0],b_eq=eq[1],bounds=(0,None),method="highs")
    assert result.success,result.message
    return -result.fun if maximize else result.fun

def verify(report,manifest):
    data=report["input"];case=next(c for c in manifest["cases"] if c["caseId"]==data["caseId"])
    assert data=={"schemaVersion":3,**case,"limits":manifest["limits"]},"frozen input changed"
    assert report["engineCommit"]=="9d1509fe5077d019825f833eed04b16d342dfda1"
    hands=report["privateCards"];n,m=map(len,hands);pot=data["pot"]/100
    expected=[{key(c):w for c,w in r.items() if w>0} for r in data["ranges"]]
    assert all({key(c) for c in hands[p]}==set(expected[p]) for p in range(2))
    nodes,possible=independent_tree(data,hands);actual={path_key(v["path"]):v for v in report["turnNodes"]}
    assert set(actual)==set(nodes),"independent topology differs"
    base=np.array([[expected[0][key(a)]*expected[1][key(b)] if cards(a).isdisjoint(cards(b)) else 0 for b in hands[1]] for a in hands[0]],dtype=float)
    base/=base.sum();strategies={};reaches={():[np.array([expected[p][key(c)] for c in hands[p]],dtype=float) for p in range(2)]};values={};chance_error=0.0;metric_error=0.0
    for path,node in nodes.items():
        raw=actual[path]
        for field in ["kind","terminal","player","actions","invested","board"]: assert raw[field]==node[field],(field,path,raw[field],node[field])
        reach=reaches[path];valid=np.array([[cards(a).isdisjoint(cards(b)) and not ((cards(a)|cards(b)) & set(node["board"])) for b in hands[1]] for a in hands[0]],dtype=float)
        joint=np.outer(*reach)*valid;mass=joint.sum()
        for p in range(2):
            compatible=joint.sum(axis=1-p)
            assert np.max(abs(compatible-np.array(raw["compatibleWeights"][p])))<1e-5,"compatible range"
        if node["kind"]=="chance":
            assert [v["card"] for v in raw["chanceCards"]]==possible
            for item in raw["chanceCards"]:
                r=item["card"];mask=np.array([[r not in cards(a)|cards(b) for b in hands[1]] for a in hands[0]],dtype=float)
                probability=float((joint*mask).sum()/(44*mass)) if mass>0 else None
                if probability is None: assert item["probability"] is None
                else: chance_error=max(chance_error,abs(probability-item["probability"]))
                reaches[(*path,("deal",r))]=[w*np.array([r not in cards(c) for c in hands[p]]) for p,w in enumerate(reach)]
        elif not node["terminal"]:
            p=node["player"];strategy=np.array(raw["strategy"]).reshape(len(node["actions"]),len(hands[p]))
            assert np.isfinite(strategy).all() and (strategy>=0).all()
            live=joint.sum(axis=1-p)>0
            if live.any(): assert np.max(abs(strategy[:,live].sum(axis=0)-1))<=manifest["thresholds"]["frequencySum"]
            # Complete undefined off-path columns arbitrarily; BR still evaluates the resulting full policy.
            zero=strategy.sum(axis=0)==0;strategy[:,zero]=1/len(node["actions"])
            strategies[path]=strategy
            for a in range(len(node["actions"])):
                child=[w.copy() for w in reach];child[p]*=strategy[a];reaches[(*path,("action",a))]=child
        if node["terminal"]:
            win=np.array([[fraction(tuple(node["board"]),a,b) for b in hands[1]] for a in hands[0]],dtype=float)
            if node["folder"] is not None: win=np.full((n,m),float(node["folder"]==1))
            a,b=np.array(node["invested"])/100;values[path]=win*(pot+a+b)-a
            values[path]*=valid
    for path,node in reversed(list(nodes.items())):
        if node["terminal"]: continue
        if node["kind"]=="chance": values[path]=sum(values[(*path,("deal",r))] for r in possible)/44
        else:
            p=node["player"];values[path]=sum(values[(*path,("action",a))]*(s[:,None] if p==0 else s[None,:]) for a,s in enumerate(strategies[path]))
    # Independent sequence form: chance is in A, each player's own realization in x/y.
    systems=[sequence_system(nodes,hands,p) for p in range(2)];a_mat=sparse.lil_matrix(tuple(len(v[0])+1 for v in systems))
    for path,node in nodes.items():
        if not node["terminal"]:continue
        chance=1/44 if len(node["board"])==5 else 1
        for i in range(n):
            for j in range(m):
                if not base[i,j] or (cards(hands[0][i])|cards(hands[1][j])) & set(node["board"]):continue
                a_mat[systems[0][3][path][i],systems[1][3][path][j]]+=base[i,j]*chance*values[path][i,j]
    matrix=a_mat.tocsr();eq=[(v[1],v[2]) for v in systems];realizations=[]
    for p,system in enumerate(systems):
        x=np.zeros(len(system[0])+1);x[0]=1
        for (path,h,a),idx in system[0].items():x[idx]=x[system[4][(path,h)]]*strategies[path][a,h]
        assert np.max(abs(system[1]@x-system[2]))<1e-5
        realizations.append(x)
    current=float(realizations[0]@matrix@realizations[1]);assert abs(current-float((values[()]*base).sum()))<1e-7
    lp=lp_value(matrix,eq);nash=br(matrix@realizations[1],eq[0],True)-br(matrix.T@realizations[0],eq[1],False)
    assert nash<=manifest["thresholds"]["nashConvPotFraction"]*pot,("independent NashConv",nash)
    assert abs(lp-current)<=manifest["thresholds"]["valuePotFraction"]*pot
    assert abs(2*report["exploitability"]/100-nash)<1e-5,("quality metric",report["exploitability"],nash)
    assert max(abs(report["centeredRootEv"][p]/100-([current,pot-current][p]-pot/2)) for p in range(2))<1e-5
    for path,node in nodes.items():
        raw=actual[path];reach=reaches[path]
        valid=np.array([[cards(a).isdisjoint(cards(b)) and not ((cards(a)|cards(b))&set(node["board"])) for b in hands[1]] for a in hands[0]],dtype=float)
        joint=np.outer(*reach)*valid
        for p in range(2):
            for h,c in enumerate(hands[p]):
                distribution=joint[h,:] if p==0 else joint[:,h]
                if distribution.sum()<=0:continue
                distribution=distribution/distribution.sum()
                payoff=values[path][h,:] if p==0 else pot-values[path][:,h]
                ev=float(distribution@payoff)+node["invested"][p]/100
                metric_error=max(metric_error,abs(ev-raw["ev"][p][h]/100))
                equities=np.array([fraction(tuple(node["board"]),c,o) for o in hands[1-p]])
                assert abs(float(distribution@equities)-raw["equity"][p][h])<1e-5,("equity",path,p,h)
                if node["player"]==p:
                    for a in range(len(node["actions"])):
                        child=values[(*path,("action",a))];payoff=child[h,:] if p==0 else pot-child[:,h]
                        metric_error=max(metric_error,abs(float(distribution@payoff)+node["invested"][p]/100-raw["actionEv"][a*len(hands[p])+h]/100))
    assert metric_error<=manifest["thresholds"]["evBb"],("node/action EV",metric_error)
    assert chance_error<1e-7
    return {"caseId":data["caseId"],"pass":True,"nodes":len(nodes),"lpValueOopBb":lp,"actualValueOopBb":current,"independentNashConvBb":nash,"maxNodeActionErrorBb":metric_error,"maxChanceError":chance_error,"iterations":report["iterations"],"elapsedMs":report["elapsedMs"],"memoryBytes":report["memoryBytes"],"stopReason":report["stopReason"],"inputSha256":hashlib.sha256(json.dumps(data,sort_keys=True,separators=(",",":")).encode()).hexdigest()}

if __name__=="__main__":
    root=Path(__file__).parent;manifest=json.loads((root/"turn-cases.json").read_text());directory=Path(sys.argv[1]);results=[]
    for case in manifest["cases"]:
        result=verify(json.loads((directory/(case["caseId"]+".json")).read_text()),manifest);results.append(result);print(json.dumps(result),flush=True)
    (directory/"independent-results.json").write_text(json.dumps(results,indent=2))
