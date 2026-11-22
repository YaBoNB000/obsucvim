"""Recover CFG edges by lexical tree parsing + integer-domain predicate proofs.
No source execution, no calling any handler, and no VM interpretation.
"""
import re,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'recovered'
s=(ROOT/'analysis/masked.lua.txt').read_text().replace(';\n',';')
my=s[s.index('My=function(b,C,ya,'):s.index(',[10]=buffer.readi16')]
call=r'ya,_Z,Zc,PZ,yM,qu=(?:b:\w+|b\[92\])\([^;]+;continue'
conds=[r'\(ya\*ya\+ya\)%2==0',r'\(ya\+1\)\*\(ya\+1\)>=ya\*ya',r'ya%3\*\(ya%3\)>=0',r'ya%5\*\(ya%5\)\*\(ya%5\*\(ya%5\)\)>=0']
removed=0
for cond in conds:
    my,n=re.subn(r'if\s*'+cond+r'\s*then ('+call+r') else local _=ya end',r'\1',my);removed+=n
    my,n=re.subn(r'if not\('+cond+r'\)then local _=ya else ('+call+r') end',r'\1',my);removed+=n
my=re.sub(call,lambda m:' LEAF_'+re.search(r'=(?:b:([\w]+)|b\[(92)\])',m[0])[0].replace('=b:','').replace('=b[','slot').replace(']','')+' ',my)
start='My=function(b,C,ya,_Z,Zc,PZ,yM,qu)while true do '
assert my.startswith(start)
my=my[len(start):]
my=my.replace('local h,E=b:dl(C,ya);if h==2 then return 2,E end;ya=h',' LEAF_dl ')
# One function-end and one loop-end after the root if.
assert my.endswith('end end end')
my=my[:-len(' end end')]
tokens=re.findall(r'if ya<=\d+ then|LEAF_\w+|else|end|\S+',my)
pos=0;routes=[]
def tree(lo,hi):
    global pos
    tok=tokens[pos];pos+=1
    if tok.startswith('LEAF_'):
        routes.append({'min':lo,'max':hi,'handler':tok[5:]});return
    assert tok.startswith('if ya<='),(pos,tok)
    bound=int(re.search(r'\d+',tok)[0])
    tree(lo,min(hi,bound) if hi is not None else bound)
    assert tokens[pos]=='else';pos+=1
    tree(max(lo,bound+1),hi)
    assert tokens[pos]=='end';pos+=1
# Valid execution states are nonnegative integers, a statically proven invariant.
tree(0,None)
assert pos==len(tokens),(pos,tokens[pos:])
# Constant return slots of syntactically identical no-op state handlers.
rtn={}
dummy={}
for m in re.finditer(r'(\w+)=function\(b,C,([^)]*)\)local (\w+)=C\[8\]~=nil and (\d+) or (\d+);return (\d+),C,([^;]*?) end',s):
    name=m[1];rtn[name]=int(m[6]);dummy[name]={'dead_local':m[3],'dead_condition_values':[int(m[4]),int(m[5])],'return_state':int(m[6])}
rtn.update(cB=4,KM=7,Ra=8,Mu=10,uc=12,YO=19,slot92=218,iH=231,Rx=233,jN=236)
for r in routes:r['return_state']=rtn.get(r['handler']);r['dummy']=r['handler'] in dummy
# Traverse the statically recovered directed edges, not the functions themselves.
edges=[];state=3;seen=set()
while state not in seen:
    seen.add(state)
    r=next(r for r in routes if state>=r['min'] and (r['max'] is None or state<=r['max']))
    edges.append({'state':state,**r})
    if r['handler']=='dl':break
    assert r['return_state'] is not None,r
    state=r['return_state']
assert edges[-1]['handler']=='dl'
obj={'removed_opaque_wrappers':removed,'dummy_handler_count':len(dummy),'route_leaves':len(routes),
     'reachable_route_leaves':len(edges),'routes':routes,'reachable_path':edges,'dummy_handlers':dummy}
(OUT/'outer_cfg.json').write_text(json.dumps(obj,indent=2))
(ROOT/'analysis/outer_cfg.normalized.txt').write_text(my)
print('opaque wrappers',removed,'dummy handlers',len(dummy),'route leaves',len(routes),'path',len(edges))
print(' -> '.join(f"{r['state']}:{r['handler']}" for r in edges))
