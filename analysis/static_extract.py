"""Static data extraction only. Never executes/loads Lua or VM instructions.
Only parses literal arrays and folds an allowlisted arithmetic AST.
"""
from pathlib import Path
import re, json, ast, struct, hashlib
ROOT=Path(__file__).resolve().parents[1]
SRC=ROOT/'uploads/test.txt'
OUT=ROOT/'recovered'
OUT.mkdir(exist_ok=True)
s=SRC.read_text()
strings=json.loads((ROOT/'analysis/string_literals.json').read_text())
mask=(ROOT/'analysis/masked.lua.txt').read_text().replace(';\n',';')

def arith(expr,env=None):
    env=env or {}
    def f(n):
        if isinstance(n,ast.Expression): return f(n.body)
        if isinstance(n,ast.Constant) and isinstance(n.value,(int,float)): return n.value
        if isinstance(n,ast.Name): return env[n.id]
        if isinstance(n,ast.Subscript): return f(n.value)[f(n.slice)]
        if isinstance(n,ast.UnaryOp) and isinstance(n.op,ast.USub): return -f(n.operand)
        if isinstance(n,ast.BinOp):
            a,b=f(n.left),f(n.right)
            if isinstance(n.op,ast.Add): return a+b
            if isinstance(n.op,ast.Sub): return a-b
            if isinstance(n.op,ast.Mult): return a*b
            if isinstance(n.op,ast.Mod): return a%b
            if isinstance(n.op,ast.FloorDiv): return a//b
            if isinstance(n.op,ast.Div): return a/b
        raise ValueError(ast.dump(n))
    return f(ast.parse(expr,mode='eval'))

def arr(name):
    p=rf'local {re.escape(name)}=\{{([\d,]+)\}}'
    m=re.search(p,mask)
    if not m: raise KeyError(name)
    return [None]+list(map(int,m[1].split(',')))

def barray(n):
    m=re.search(rf'\[{n}\]=\{{([\d,]+)\}}',mask)
    return [None]+list(map(int,m[1].split(',')))

def content(i):
    v=strings[i][2]
    m=re.match(r'\[(=*)\[',v)
    if m:
        k=len(m[0]); return v[k:-k]
    # Only short literals needed here are empty; no eval is used.
    if v in ('""',"''"): return ''
    raise ValueError('Short literal not needed for data extraction')

b={n:arith(re.search(rf'\[{n}\]=([\d+\-]+)(?=,)',mask)[1]) for n in (43,98,30,25,63,23)}
for n in (48,70,67): b[n]=barray(n)
base=(b[43]+b[98])%2**32
step=(b[30]-b[25])%2**32
x=(b[63]+b[23])%2**28
outer=[]; keys=[]
params=[(b[70][6]+b[48][2],b[70][18]+b[48][18]),
        (b[70][21]+b[48][36],b[70][10]+b[48][33]),
        (b[70][12]+b[48][20],b[70][5]+b[48][28])]
for seg in range(5):
    for a,c in params:
        assert a*x+c < 2**53
        x=(a*x+c)%2**28
    keys.append(x)
    raw=b''.join(struct.pack('<I',(b[67][k]-(base+k*step))%2**32) for k in range(seg*10+1,seg*10+11))
    n=39 if seg<4 else 38
    decoded=bytes(v^((x+j)%256) for j,v in enumerate(raw[:n],1))
    outer.append(decoded.decode('ascii'))
H=''.join(outer)
assert sum(H.encode())==b[70][7]+b[48][40]
(OUT/'payload.base94.txt').write_text(H)
print('Outer',json.dumps({'base':base,'step':step,'seed':(b[63]+b[23])%2**28,'prng':params,'keys':keys,'parts':outer,'checksum':sum(H.encode())},ensure_ascii=False,indent=2))

env={name:arr(name) for name in ('y','Y','N','I')}
# Replace obfuscated literal-index lookups with their literal values.
folded=re.sub(r'\b([yYNI])\[([\d+\-]+)\]',lambda m:str(env[m[1]][arith(m[2])]),mask)
(ROOT/'analysis/constant_folded.lua.txt').write_text(folded.replace(';',';\n'))

inner_folded=folded[folded.index('local UB=function(H)'):]

def initial(name):
    # Decode constants only within UB, to avoid shadowed locals in outer functions.
    m=re.search(r'local '+re.escape(name)+r'=([^;]+);',inner_folded)
    return arith(m[1])

ad=arr('Ad')[1:]
j={}
expr=re.search(r'j\[v\]=([^;]+);',folded)[1].replace('#Ad',str(len(ad)))
for k in range(0,len(ad),2):
    j[ad[k]]=arith(expr,{'Ad':[None]+ad,'m':k+1,'v':ad[k]})
Y_=initial('Y_')

ma=arr('ma')[1:]; u=initial('u')
expr=re.search(r'for WM=1,94 do u=([^;]+);',folded)[1].replace('#ma',str(len(ma)))
A={}
for i,v in enumerate(ma):
    u=arith(expr,{'u':u})
    A[(v-u%256)%256]=i
assert len(A)==94
alphabet=bytes(sorted(A,key=A.get)).decode('ascii')

kz=arr('kz')[1:]; M=initial('M'); U=[]
expr=re.search(r'for T=1,60 do M=([^;]+);',folded)[1]
for v in kz:
    M=arith(expr,{'M':M})
    U.append((v-M%256)%256)
z={bytes(U[i*5:i*5+5]).decode():chr(U[50+i]) for i in range(10)}

params2={name:initial(name) for name in ('t','dS','S','F','O','lg')}

# Decode static source bytes: host guard requires debug.info(loadstring,'s') == '[C]'.
# This proves the hash in the non-trapping branch; we do NOT query any host functions.
hash_C=0
for v in b'[C]': hash_C=(hash_C*31+v)%2**28
assert hash_C==89621
init_expr=re.search(r'local _t=([^;]+);',inner_folded)[1]
_t=arith(init_expr,{'mD':hash_C})
stream_expr=re.search(r'for jT=1,#kO do _t=([^;]+);',folded)[1]
order=re.findall(r'for \w+=1,#(\w+) do kO\[#kO\+1\]',mask)
ko=sum((arr(n)[1:] for n in order),[])
loader=[]
for v in ko:
    _t=arith(stream_expr,{'_t':_t})
    loader.append((v-_t%256)%256)
loader=bytes(loader)
(OUT/'stage1_loader.decoded.lua.txt').write_bytes(loader)

state_info={'source_sha256':hashlib.sha256(s.encode()).hexdigest(),
            'outer':{'base':base,'step':step,'seed':(b[63]+b[23])%2**28,'prng':params,'keys':keys,'parts':outer,'checksum':sum(H.encode())},
            'j':j,'Y_patch':Y_,'alphabet':alphabet,'escape_map':z,'stream_params':params2,
            'loader_array_order':order,'loader_length':len(loader),
            'loader_prng_init':arith(init_expr,{'mD':hash_C}), 'loader_prng_recurrence':stream_expr}
(OUT/'static_constants.json').write_text(json.dumps(state_info,ensure_ascii=False,indent=2))
print('J',j,'patch',Y_)
print('alphabet',repr(alphabet),'z',z,'params',params2)
print('loader length',len(loader))
print(loader.decode('ascii'))
