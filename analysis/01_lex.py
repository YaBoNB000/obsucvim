"""Lexical string masking only; no Lua evaluation."""
from pathlib import Path
import re,json,hashlib
ROOT=Path(__file__).resolve().parents[1]
s=(ROOT/'uploads/test.txt').read_text()
(ROOT/'analysis').mkdir(exist_ok=True)
pat=re.compile(r'\[(=*)\[|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'')
pos=0;chunks=[];strings=[]
while (m:=pat.search(s,pos)):
    a=m.start()
    if m.group(1) is not None:
        term=']'+m.group(1)+']';b=s.find(term,m.end());assert b>=0
        end=b+len(term)
    else:end=m.end()
    literal=s[a:end];idx=len(strings);strings.append((a,end,literal))
    chunks.extend([s[pos:a],f'__STR_{idx:03d}__']);pos=end
chunks.append(s[pos:])
(ROOT/'analysis/masked.lua.txt').write_text(''.join(chunks).replace(';',';\n'))
(ROOT/'analysis/string_literals.json').write_text(json.dumps(strings,ensure_ascii=False,indent=2))
print('bytes',len(s.encode()),'literal_count',len(strings),'sha256',hashlib.sha256(s.encode()).hexdigest())
