#!/usr/bin/env python3
"""从 run3.log 提取 BUFDUMP 十六进制 → bin 文件, 并做初步结构分析。"""
import re, sys, collections, math

log = open(sys.argv[1] if len(sys.argv) > 1 else "run3.log").read()
dumps = re.findall(r"BUFDUMP (\w+) (B\d+) len=(\d+)\n([0-9a-f]+)", log)
for tag, bid, ln, hexd in dumps:
    data = bytes.fromhex(hexd)
    assert len(data) == int(ln), (tag, bid, len(data), ln)
    fn = f"{bid}_{tag}.bin"
    open(fn, "wb").write(data)
    # 熵
    cnt = collections.Counter(data)
    ent = -sum(c / len(data) * math.log2(c / len(data)) for c in cnt.values())
    # 可读 ASCII 串
    runs = re.findall(rb"[\x20-\x7e]{6,}", data)
    print(f"{fn}: len={len(data)} entropy={ent:.3f} distinct={len(cnt)} ascii_runs={len(runs)}")
    for r in runs[:40]:
        print("   ", r.decode())
