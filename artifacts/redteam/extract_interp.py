#!/usr/bin/env python3
"""按括号配平切字段源码(候选=逗号后跟字段名=)。"""
import re

src = open("sample_luraph15.lua").read()

def extract_field(field):
    i = src.index(field)
    j = src.index("=", i) + 1
    for m in re.finditer(r"end,", src[j:]):
        k = j + m.start() + 3  # 含 end
        seg = src[j:k]
        # 配平检查(忽略字符串内括号)
        pd = bd = 0
        instr = None
        p = 0
        ok = True
        while p < len(seg):
            c = seg[p]
            if instr:
                if c == "\\":
                    p += 2
                    continue
                if c == instr:
                    instr = None
            elif c in "\"'":
                instr = c
            elif c == "(":
                pd += 1
            elif c == ")":
                pd -= 1
            elif c == "{":
                bd += 1
            elif c == "}":
                bd -= 1
            if pd < 0 or bd < 0:
                ok = False
                break
            p += 1
        if ok and pd == 0 and bd == 0 and instr is None:
            return seg
    return None

for field, out in [("[18]=function", "interp18.lua"), ("[73]=function", "factory73.lua")]:
    s = extract_field(field)
    print(field, "->", len(s), "chars")
    open(out, "w").write(s + "\n")

i18 = open("interp18.lua").read()
ops = sorted(set(int(n) for _, n in re.findall(r"O([<>]=?|~=)(\d+)", i18)))
print("opcode constants in [18]:", ops)
f73 = open("factory73.lua").read()
print("f-constants in [73]:", sorted(set(int(n) for _, n in re.findall(r"f([<>]=?|~=)(\d+)", f73))))
