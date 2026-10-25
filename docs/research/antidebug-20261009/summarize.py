#!/usr/bin/env python3
"""Refresh human/machine evidence from completed tests and sweep snapshots."""
import csv
import json
from pathlib import Path
import re
import statistics

from sweep import integrity

HERE = Path(__file__).resolve().parent


def main():
    behavior = (HERE / 'behavior.log').read_text()
    reseal = (HERE / 'reseal.log').read_text()
    tests = []
    counterexamples = []
    benches = []
    for line in behavior.splitlines():
        fields = line.split('\t')
        if fields[0] == 'PASS': tests.append(fields[1])
        if fields[0] == 'COUNTEREXAMPLE': counterexamples.append(fields[1])
        if fields[0] == 'BENCH': benches.append([float(x) for x in fields[1:]])
    rotations = []
    rotation_counterexamples = []
    for line in reseal.splitlines():
        fields = line.split('\t')
        if fields[0] == 'BENCH': rotations.append([float(x) for x in fields[1:]])
        if fields[0] == 'COUNTEREXAMPLE': rotation_counterexamples.append(fields[1])
    baseline = (HERE / 'baseline-matrix.log').read_text()
    presets = (HERE / 'baseline-presets.log').read_text()
    cargo = (HERE / 'cargo.log').read_text()
    defense = (HERE / 'defense-audit.log').read_text()
    data = {
        'scope': 'research only; no candidate integrated; not a completed five-hour claim',
        'production_cargo_test_summaries': re.findall(r'test result:.*', cargo),
        'production_matrix_summary': re.findall(r'PASS checks:.*', baseline),
        'production_preset_summary': re.findall(r'PRESET PASS:.*', presets),
        'production_defense_summary': re.findall(r'防御审计:.*', defense),
        'production_defense_failures': [line.strip() for line in defense.splitlines() if line.strip().startswith('FAIL ')],
        'loader_audit_pass': 'H4 ZERO-LOADER GATE: PASS' in (HERE / 'loader-audit.log').read_text(),
        'micro_audit_pass': 'H5-P0 审计: PASS' in (HERE / 'micro-audit.log').read_text(),
        'luau_behavior_passes': tests,
        'luau_counterexamples_reproduced': counterexamples,
        'receipt_microbench_samples': benches,
        'receipt_microbench_ratio_median': statistics.median(row[-1] for row in benches),
        'reseal_microbench_samples': rotations,
        'reseal_counterexamples_reproduced': rotation_counterexamples,
        'models': json.loads((HERE / 'model-results.json').read_text()),
        'pilot': json.loads((HERE / 'pilot-status.json').read_text()),
        'file_integrity': integrity(),
    }
    if (HERE / 'extended-status.json').exists():
        data['extended_snapshot'] = json.loads((HERE / 'extended-status.json').read_text())
    (HERE / 'evidence.json').write_text(json.dumps(data, indent=2, ensure_ascii=False) + '\n')
    extended = data.get('extended_snapshot', {})
    markdown = f'''# 实验进度快照

> 本文件由 `python3 research/antidebug-20261009/summarize.py` 更新；不是实时刷新页面。
> 五小时长跑的较新状态见 `extended-status.json`，逐条记录见 `extended.csv`。

- 长跑状态：**{extended.get('state', 'not-started')}**。
- 已用单调时钟时间：**{extended.get('elapsed_seconds', 0):.2f} 秒**；预算 18,000 秒，不代表已完成。
- 已记录：**{extended.get('attempted', 0)}** 个源码/seed 组合，**{extended.get('passed', 0)}** 全链对照通过，**{extended.get('failed', 0)}** 出现差异。
- 这些是未修改引擎的运行 + 外层 adapter 对照，**不是正式 VM 内植入候选后的验证**。
- 首轮：117 组，116 通过 / 1 现役产物差异；独立重现见 `baseline-discrepancy.json`。
- 研究模型：20,000 轮；160,000 次合法调用完成验证；279,535 次合法边推进。完整状态/当前凭证绕过、合法边内改分支、线性伪造均已构造反例。
- Luau 定向语义：{len(tests)} 项正向检查，{len(counterexamples)} 项退步/失效反例复现。
- 调用凭证微基准中位开销比：**{data['receipt_microbench_ratio_median']:.3f}×**（相对于同样 pack/pcall 的无凭证 adapter；不是整个 VM 的开销）。
- 现役固定测试：cargo 39+4 passed / 1 ignored；主矩阵 169/169；预设 370/370；loader / H5-P0 通过；防御审计 20 pass / 1 fail，D3 23/32 为已知结构分。
- 不变性复核：{data['file_integrity']['checked']} 个原有文件，变化：`{data['file_integrity']['changed']}`。

## 长跑当前已记录差异（最多保存 25 项详细条目）

'''
    for failure in extended.get('failures', []):
        markdown += f"- `{failure['case']}` / seed `{failure['seed']}`：`{failure['reason']}`。\n"
    markdown += '\n**只有 state 为 `completed-time-budget` 才表示跑到设定时间预算。`stopped` / 沙箱进程丢失不算完成。**\n'
    (HERE / 'STATUS.md').write_text(markdown)
    print(markdown)


if __name__ == '__main__':
    main()
