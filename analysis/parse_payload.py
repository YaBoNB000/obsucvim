"""Static binary-format reader. Does not dispatch or execute VM opcodes."""
from pathlib import Path
import json, re, csv, math
ROOT=Path(__file__).resolve().parents[1]; OUT=ROOT/'recovered'
meta=json.loads((OUT/'static_constants.json').read_text())
e=(OUT/'payload.encrypted.bin').read_bytes()
sp=meta['stream_params']; seed=sp['O']; mul=sp['S']; add=sp['F']
first=e[:64]; h=0
for b in first: h=(h*31+b)%2**28
plain=bytearray();g=seed
for i,b in enumerate(e):
    if i==64: g=(seed+h)%2**28
    assert mul*g+add<2**53
    g=(g*mul+add)%2**28
    plain.append((b-g%256)%256)
s=bytes(plain)
(OUT/'payload.decrypted.bin').write_bytes(s)
(OUT/'payload.decrypted.hex.txt').write_text('\n'.join(f'{i:04x}: '+s[i:i+16].hex(' ') for i in range(0,len(s),16))+'\n')

def vu(data,p):
    start=p;v=0
    for n in range(4):
        b=data[p];p+=1
        if b<128 or n==3:
            v+=b*128**n
            if v>=2**31:v-=2**32
            return v,p
        v+=(b-128)*128**n
    raise AssertionError(start)
p=0;got=0;obj={};sections=[]
while got<6:
    start=p;tag=s[p];p+=1
    if tag==118:
        for k in ('nregs','nparams','vararg'):obj[k],p=vu(s,p)
        label='header';got+=1
    elif tag==221:
        obj['constant_seed'],p=vu(s,p);label='constant_seed';got+=1
    elif tag==237:
        n,p=vu(s,p);obj['upsrc']=[]
        for _ in range(n):
            v,p=vu(s,p);obj['upsrc'].append(v)
        label='upvalues';got+=1
    elif tag==86:
        obj['nconst'],p=vu(s,p);obj['const_bytes'],p=vu(s,p)
        obj['const_offset']=p;p+=obj['const_bytes'];label='constants';got+=1
    elif tag==82:
        n,p=vu(s,p);obj['S']=[]
        for _ in range(n):v,p=vu(s,p);obj['S'].append(v)
        label='register_map';got+=1
    elif tag==230:
        n,p=vu(s,p);obj.setdefault('decoys',[]).append({'offset':p,'size':n,'hex':s[p:p+n].hex(' ')})
        p+=n;label='skipped_decoy'
    else:
        n,p=vu(s,p);obj['ncode']=n;obj['W_before_patch']=list(s[p:p+n]);p+=n
        for k in ('SA','SB','SC','SD'):
            obj[k]=[]
            for _ in range(n):v,p=vu(s,p);obj[k].append(v)
        obj['checksum']=sum(sum(obj[k]) for k in ('SA','SB','SC','SD'))%2**32
        label='instructions';got+=1
    sections.append({'tag':tag,'start':start,'end':p,'meaning':label})
obj['trailing_bytes']=s[p:].hex(' ')
obj['sections']=sections;obj['first64_ciphertext_hash']=h
# Constant pool byte decryption, followed by typed-format decoding.
g=obj['constant_seed'];cs=bytearray()
for b in s[obj['const_offset']:obj['const_offset']+obj['const_bytes']]:
    assert sp['t']*g+sp['dS']<2**53
    g=(sp['t']*g+sp['dS'])%2**28
    cs.append((b-g%256)%256)
obj['constants_plain_hex']=cs.hex(' ')
(OUT/'constants.decrypted.bin').write_bytes(cs)
constants=[];q=0

def uleb(data,q):
    v=0;n=0
    while True:
        b=data[q];q+=1;v+=(b&127)*128**n
        if b<128:return v,q
        n+=1
        assert n<16

for i in range(obj['nconst']):
    start=q;t=cs[q];q+=1
    if t==0:v=None
    elif t==1:v=cs[q]==1;q+=1
    elif t==3:
        n=int.from_bytes(cs[q:q+2],'little');q+=2
        bs=bytes(cs[q:q+n]);q+=n;v=bs.decode('utf-8')
    else:
        m,q=uleb(cs,q);kp,q=uleb(cs,q)
        exponent=kp//2-2048
        v=math.ldexp(m,exponent)*(-1 if kp%2 else 1)
        if v.is_integer():v=int(v)
    constants.append({'index':i+1,'tag':t,'value':v,'pool_range':[start,q]})
assert q==len(cs),(q,len(cs))
obj['C']=constants
obj['W']=obj['W_before_patch'][:];obj['W'][5]=meta['Y_patch']
parser=(OUT/'bytecode_parser.decoded.lua.txt').read_text()
# This is a fixed three-literal addition, not executing the parser.
a,b,c=map(int,re.search(r'local OS = \{\((\d+) \+ (\d+) - (\d+)\)\}',parser).groups())
obj['expected_checksum']=a+b-c
assert obj['checksum']==obj['expected_checksum']
assert len(obj['S'])==obj['nregs']
assert len(set(obj['S']))==len(obj['S'])  # Register locations may be sparse, not 1..nregs.
assert obj['trailing_bytes']==''
obj['instructions']=[{'pc':i+1,'opcode_before_patch':obj['W_before_patch'][i], 'opcode':obj['W'][i],
                     'a_SD':obj['SD'][i],'b_SC':obj['SC'][i],'c_SA':obj['SA'][i],'d_SB':obj['SB'][i]}
                    for i in range(obj['ncode'])]
(OUT/'prototype.json').write_text(json.dumps(obj,ensure_ascii=False,indent=2))
with (OUT/'bytecode.csv').open('w') as f:
    w=csv.DictWriter(f,fieldnames=list(obj['instructions'][0]));w.writeheader();w.writerows(obj['instructions'])
print(json.dumps(obj,ensure_ascii=False,indent=2))
