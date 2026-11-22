"""Static round-trip checks. No Lua runtime or VM execution."""
import runpy,json,re,struct,hashlib,contextlib,io
from pathlib import Path
with contextlib.redirect_stdout(io.StringIO()): cx=runpy.run_path(str(Path(__file__).resolve().parent/'decode_segments.py'))
OUT=cx['OUT'];ROOT=cx['ROOT'];m=json.loads((OUT/'static_constants.json').read_text());p=json.loads((OUT/'prototype.json').read_text())
A=cx['A'];AL=bytes(sorted(A,key=A.get)).decode();arith=cx['arith'];st=cx['cx']
checks={}

def enc94(data):
    padded=data+bytes((-len(data))%4);res=[]
    for off in range(0,len(padded),4):
        word=int.from_bytes(padded[off:off+4],'little');ds=[]
        for _ in range(5):ds.append(AL[word%94]);word//=94
        assert word==0
        res.extend(reversed(ds))
    return ''.join(res)
segments=cx['segments'];cl=st['arr']('Cl')[1:]
for item in json.loads((OUT/'segments_manifest.json').read_text()):
    op=item['opcode'];bs=(OUT/item['file']).read_bytes();g=arith(cx['seed_expr'],{**cx['env'],'w':op});enc=bytearray()
    for b in bs:
        g=arith(cx['rec_expr'],{**cx['env'],'hs':g});enc.append((b+g%256)%256)
    # Segment block padding may be arbitrary encrypted source bytes. Check exactly flen bytes.
    original=cx['decode94'](segments[item['slot']])
    assert bytes(enc)==original[:len(enc)]
checks['handler_parser_ciphertext_prefix_roundtrip_44_of_44']=True
# Reverse body encryption; exact original 113 bytes.
bs=(OUT/'payload.decrypted.bin').read_bytes();src=(OUT/'payload.encrypted.bin').read_bytes();sp=m['stream_params'];g=sp['O'];enc=bytearray()
for i,b in enumerate(bs):
    if i==64:g=(sp['O']+p['first64_ciphertext_hash'])%2**28
    g=(g*sp['S']+sp['F'])%2**28;enc.append((b+g%256)%256)
assert bytes(enc)==src
checks['body_ciphertext_roundtrip']=True
encoded=enc94(struct.pack('<I',len(src))+src)
escape={v:k for k,v in m['escape_map'].items()}
escaped=''.join(escape.get(x,x) for x in encoded)
assert escaped==''.join(m['outer']['parts'])
checks['outer_base94_escape_roundtrip']=True
# Reverse constant encryption: exact 43 bytes.
g=p['constant_seed'];enc=bytearray()
for b in (OUT/'constants.decrypted.bin').read_bytes():
    g=(sp['t']*g+sp['dS'])%2**28;enc.append((b+g%256)%256)
assert bytes(enc)==bs[p['const_offset']:p['const_offset']+p['const_bytes']]
checks['constant_pool_roundtrip']=True
checks['outer_byte_sum']=sum(escaped.encode())
checks['operand_sum']=p['checksum'];checks['expected_operand_sum']=p['expected_checksum']
assert checks['operand_sum']==checks['expected_operand_sum']
checks.update(outer_escaped_length=len(escaped),outer_unescaped_length=len(encoded),outer_base94_binary_length=len(cx['decode94'](encoded)),encrypted_body_length=len(src),constant_pool_length=p['const_bytes'])
checks['body_sections_consume_all_bytes']=p['trailing_bytes']==''
checks['opcode_ids_are_distinct']=len(set(m['j'].values()))==42
folded=st['inner_folded']
params={}
for name in ('u','M'):
    start=st['initial'](name)
    expr=re.search((r'for WM=1,94 do u=([^;]+);' if name=='u' else r'for T=1,60 do M=([^;]+);'),folded)[1].replace('#ma','94')
    c0=arith(expr,{name:0});c1=arith(expr,{name:1})
    params['alphabet' if name=='u' else 'escape_map']={'seed':start,'multiplier':(c1-c0)%2**28,'increment':c0}
expr=m['loader_prng_recurrence'];c0=arith(expr,{'_t':0});c1=arith(expr,{'_t':1})
params['stage1_loader']={'seed':m['loader_prng_init'],'multiplier':(c1-c0)%2**28,'increment':c0}
params['handler_parser']={'base_seed':cx['seed0'],'seed_step':(cx['seed1']-cx['seed0'])%2**28,'multiplier':(cx['a1']-cx['a0'])%2**28,'increment':cx['a0']}
checks['prng_parameters']=params
checks['empty_decoy_loop_JT']=(172065282+113321553+132649196)%7
checks['sha256']={name:hashlib.sha256((OUT/name).read_bytes()).hexdigest() for name in ('stage1_loader.decoded.lua.txt','bytecode_parser.decoded.lua.txt','payload.encrypted.bin','payload.decrypted.bin','constants.decrypted.bin')}
(OUT/'verification.json').write_text(json.dumps(checks,ensure_ascii=False,indent=2))
print(json.dumps(checks,ensure_ascii=False,indent=2))
