from pathlib import Path
import json,re,csv
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'recovered'
mask=(ROOT/'analysis/masked.lua.txt').read_text()
ki={int(i):''.join(chr(int(v)) for v in seq.split(',')) for i,seq in re.findall(r'kI\[(\d+)\]=J\(([\d,]+)\)',mask)}
gs={int(i):''.join(chr(int(v)) for v in seq.split(',')) for i,seq in re.findall(r'GS\[(\d+)\]=schar\(([\d,]+)\)',mask)}
msnames=['EQ','APPEND_ONE','NEWTABLE','LT','FLOORDIV','JMP_IF_FALSE','VARARG_COUNT','NOT','MUL','GE','MOVE','NE','SETUPVAL','POW','NOP','CALL_APPEND','RETURN','GETGLOBAL','MOD','CALL_WITH_TAIL','GETUPVAL','APPEND_VARARGS','LOADNIL_RANGE','CALL','LOAD_SHORT_STRING','SETTABLE','JMP_IF_TRUE','GT','CLOSURE','JMP','UNM','CALL_COUNTED','GETTABLE','PACK_VARARGS','DIV','LEN','LE','LOADK','SUB','CONCAT','ADD','SETGLOBAL','NOP_PATCH']
c=json.loads((OUT/'static_constants.json').read_text());jinv={v:int(k) for k,v in c['j'].items()}
entries=[];lines=['# 已解码指令处理器（静态文本）','', 'E 的 22 个槽见主报告。以下只去掉重复的槽别名声明，并将 MS[n] 替换为已恢复的字符串；未执行这些处理器。','']
for n in range(43):
    text=(OUT/'handlers'/f'op_{n:02d}.decoded.lua.txt').read_text()
    body=text.split('E[22]',1)[1].strip()
    assert body.endswith(' end');body=body[:-4]
    body=re.sub(r'MS\[(\d+)\]',lambda m:json.dumps(ki[int(m[1])],ensure_ascii=False),body)
    entries.append({'opcode':n,'j_index':jinv.get(n),'name':msnames[n],'body':body})
    lines.extend([f'## {n:02d} — {msnames[n]}'+(f'（j[{jinv[n]}]）' if n in jinv else '（补丁专用）'),'','```lua',body.replace('; ',';\n'),'```',''])
(OUT/'opcode_semantics.json').write_text(json.dumps(entries,ensure_ascii=False,indent=2))
(OUT/'all_handlers.annotated.md').write_text('\n'.join(lines))
(OUT/'vm_strings.json').write_text(json.dumps({'VM_MS':ki,'Preflight_GS':gs},ensure_ascii=False,indent=2))
proto=json.loads((OUT/'prototype.json').read_text())
comments=[
 'V[9] = ENV[C[3]] = ENV["print"]；c、d 未使用',
 'V[4] = C[2] = 12；c、d 未使用',
 'base=2；f=V[S[2]]=V[9]；arg=V[S[3]]=V[4]；调用 f(12)，请求至少一个返回槽',
 'V[8] = V[4]；复制返回值的第一个槽，之后不再使用',
 'return；返回值数量 b=0，终止可达控制流',
 '原 opcode=14，在执行 VM 前静态补为42；两者均 NOP；此处在 RETURN 之后不可达'
]
lines=['# 自定义 VM 反汇编','', '此文件由指令字段解码与处理器语义匹配得到，没有运行 VM。','',f"S = {proto['S']}；逻辑寄存器位置映射到稀疏物理槽。",'', '|PC|opcode|语义|a=SD|b=SC|c=SA|d=SB|解释|','|---:|---:|---|---:|---:|---:|---:|---|']
for i,x in enumerate(proto['instructions']):
    lines.append(f"|{x['pc']}|{x['opcode']}|{msnames[x['opcode']]}|{x['a_SD']}|{x['b_SC']}|{x['c_SA']}|{x['d_SB']}|{comments[i]}|")
lines.extend(['','## 静态控制流','', '```text','PC1 → PC2 → PC3 → PC4 → PC5 → EXIT','PC6: unreachable (NOP_PATCH)','```'])
(OUT/'disassembly.md').write_text('\n'.join(lines)+'\n')

# Source byte offsets with string spans masked, preventing false matches in random strings.
s=(ROOT/'uploads/test.txt').read_text();ss=s
for a,b,_ in reversed(json.loads((ROOT/'analysis/string_literals.json').read_text())): ss=ss[:a]+(' '*(b-a))+ss[b:]
anchors={}
for name in ('vH','CY','tB','My','cB','KM','Ra','Mu','uc','YO','iH','Rx','jN','dl','DX','ny','mp','h','gU','QL'):
    m=re.search(r'(?:[,{])('+re.escape(name)+r')=',ss)
    if m:anchors[name]=m.start(1)
for key in ('[92]=function','local ma={','local kz={','local Cl={','local kM={','local _g={','local sV=s(0)[od]','re,Bd=sV','W=sV(Bd)','local NQ=W[1].W','local function Wy(','local JT='):
    m=re.search(re.escape(key),ss)
    if m:anchors[key]=m.start()
(OUT/'source_anchors.json').write_text(json.dumps(anchors,ensure_ascii=False,indent=2))
print('anchors',anchors)
print('strings',json.dumps(ki,ensure_ascii=False))
