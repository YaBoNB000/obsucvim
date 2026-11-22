"""Decode inert strings/byte arrays using literal arithmetic, not their Lua code."""
import runpy,re,json,struct
from pathlib import Path
# This imports our trusted literal extractor, NOT the uploaded Lua or decoded text.
cx=runpy.run_path(str(Path(__file__).resolve().parent/'static_extract.py'))
OUT=cx['OUT']; ROOT=cx['ROOT']; arith=cx['arith']; A=cx['A']; z=cx['z']
loader=(OUT/'stage1_loader.decoded.lua.txt').read_text()
env=dict(zip(('KA','KB','KC','KM'),[cx['env'][n] for n in ('y','Y','N','I')]))
seed_expr=re.search(r'local hs = (.*?) local t =',loader)[1]
rec_expr=re.search(r'hs = (.*?); t\[ti\]',loader[loader.index('local b4 ='):])[1]
a0=arith(rec_expr,{**env,'hs':0}); a1=arith(rec_expr,{**env,'hs':1})
seed0=arith(seed_expr,{**env,'w':0}); seed1=arith(seed_expr,{**env,'w':1})
print('Handler PRNG:',seed0,(seed1-seed0)%2**28,(a1-a0)%2**28,a0)

def decode94(v, escapes=False, header=False):
    if escapes:
        s=[]; pos=0
        while pos<len(v):
            if v[pos]=='Y':
                assert v[pos:pos+5] in z,v[pos:pos+5]
                s.append(z[v[pos:pos+5]]);pos+=5
            else:
                s.append(v[pos]);pos+=1
        v=''.join(s)
    assert len(v)%5==0,len(v)
    decoded=bytearray()
    for p in range(0,len(v),5):
        x=0
        for q in v[p:p+5]: x=x*94+A[ord(q)]
        assert x<2**32
        decoded.extend(struct.pack('<I',x))
    if header:
        n=int.from_bytes(decoded[:4],'little')
        assert n<=len(decoded)-4,(n,len(decoded))
        return bytes(decoded[4:4+n])
    return bytes(decoded)

cl=cx['arr']('Cl')[1:]
segments={int(k):cx['content'](int(v)) for k,v in re.findall(r'_\[(\d+)\]=__STR_(\d+)__',cx['mask'])}
(OUT/'handlers').mkdir(exist_ok=True)
manifest=[]
for i in range(0,len(cl),3):
    op,slot,size=cl[i:i+3]
    ct=decode94(segments[slot])
    hs=arith(seed_expr,{**env,'w':op})
    pt=[]
    for b in ct:
        hs=arith(rec_expr,{**env,'hs':hs})
        pt.append((b-hs%256)%256)
    pt=bytes(pt[:size])
    pt.decode('ascii')
    dest=OUT/'bytecode_parser.decoded.lua.txt' if op==200 else OUT/'handlers'/f'op_{op:02d}.decoded.lua.txt'
    dest.write_bytes(pt)
    manifest.append({'opcode':op,'slot':slot,'length':size,'file':str(dest.relative_to(OUT))})
(OUT/'segments_manifest.json').write_text(json.dumps(manifest,indent=2))
# Binary is inert data, not native Lua bytecode. No instruction interpretation here.
payload=decode94(cx['H'],escapes=True,header=True)
(OUT/'payload.encrypted.bin').write_bytes(payload)
(OUT/'payload.encrypted.hex.txt').write_text(payload.hex(' '))
print('DECODED SEGMENTS',len(manifest),'payload length',len(payload),'hex',payload.hex(' '))
print('PARSER\n'+(OUT/'bytecode_parser.decoded.lua.txt').read_text())
