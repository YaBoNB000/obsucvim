#!/usr/bin/env node
/**
 * Which obfuscator build is actually deployed?
 *
 *   node web/tools/verify-engine-version.mjs [path/to/obsucvim.wasm]
 *
 * The v15 output for a fixed input + fixed seed is deterministic, so the engine
 * build can be identified from its bytes. Known anchors:
 *
 * Probe: print("suda") with --preset v15 --env-check --bind-executor --seed 67067
 *
 *   R067.3  (2026-10-23, tier-2 no longer vetoes on implementation-specific values:
 *            Random exact floats and exact Luau parser wording became structural)
 *           230,177 B  sha256 70fbc8bb3f5303f4c1d4e279aaecea74bc649fb4068a8026d4aa9bf68c0e4c14
 *
 *   R067.2  (2026-10-23, precondition now resolves the executor env: getgenv()
 *            first, then _G — the R067.1 probe only read _G, so on Delta it
 *            reported every API as missing, including typeof)
 *           231,271 B  sha256 8f1b5e21a71e8886704ad79068d8114d386dddd4a9a4145af969b47a984575f5
 *
 *   R067.1  (2026-10-23, first precondition: reports missing APIs, but probes _G only)
 *           230,368 B  sha256 8b43132eb0297a9017689acfb3a1f80950b2fc269a62630dc4cb4483c014c8fb
 *
 *   R067    (2026-10-11, full noise density; junk_n=2, /12 /5 noise, obf_num 40 bit)
 *           229,422 B  sha256 3775f3b557ca4c95ee16d9d013f290d735b6c022af23ac17f1d0b2339fc689f6
 *
 *   af7fc32 (2026-10-19, "reduce output size"; noise halved, junk_n=1, obf_num 28 bit)
 *           229,900 B  sha256 12c2c05b572d5ede5d8992684ed4aa724096f9254b66d81563848b99fd91d638
 *
 * Anything else means the binary is a build we have no anchor for; the script
 * still reports the observed bytes/hash so it can be pinned deliberately.
 */
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, statSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const TOOLS = path.dirname(new URL(import.meta.url).pathname);
const RUNNER = path.join(TOOLS, "wasi-runner.mjs");
const wasm = process.argv[2] || path.join(TOOLS, "obsucvim.wasm");

const ANCHORS = [
  {
    name: "R080",
    note:
      "静态弱点批次 1（2026-10-25，审计 V-06/V-07/V-08）：诱饵与惰性局部改接运行期槽探针" +
      "（4 形态 × 动态槽位，跨 seed 变形，值不被消费）；噪声数值改挂字面量长度与运行期余数，" +
      "单条常量折叠/代数化简规则失效。同源码同 seed 对跑：诱饵同签名 95→25、常量三元 39→1、" +
      "恒真条件 27→0；体积 354,533→353,846 B（略降）。语义 37/37 绿（VM 相位 37 项失败为既有缺陷，" +
      "R079 与 R080 一致）。本次由仓库内 .tools 工具链正牌重编（rustc 1.88 + wasm32-wasip1），" +
      "R079 锚点已先行逐字节复现，源码树可作为唯一事实源",
    size: 179643,
    sha256: "1b1056221c386f5a7232153e5d5f6f63a2e68cc4c51f5e6e7165a46b8b5c7135",
  },
  {
    name: "R079",
    note: "V19 M2 P1 车道解剖定案（F1/F2 半折叠双挂分裂 + L1/L2/L3 车道切真机裁决，2026-10-25）：bx 字节流车道 _sv 双折叠真 Delta 独罪（L1 杀之即正常；L2/L3 保之则 char/index 下游暴雷；F1 一级独存=原型 call-nil、F2 二级独存=index-nil-'VY'），bx 落定恒等线性注入（_sv=(SALT/SLT+LC)%2^28，LC 常量保留）；ct _svk / km _svq 车道折叠真机无罪、原式复辟（L1 件两车道折叠同存正常）。盐三源 + CLK 守卫维持。本发布 wasm = R077 二进制 + l1 数据段等长手术（rustc 通道恢复期产物，源码 R079 已同义落盘 obsucvim/src/vmgen/template.rs，rustc 恢复后重编刷锚）",
    size: 178551,
    sha256: "ccf5d0839fef962a8a0c031d745d869dff7a861b6590fe8e267d93f5cf63c045",
  },
  {
    name: "R078",
    note: "V19 M2 首包真机收口（判别飞行 WS1/WS2/WS3 裁决）：R077 三车道 lane NL 像在真 Delta 定罪（call-nil），判定性回退 R076「+ SALT/SLT/SLTC 直加」线性注入；三源 SALT 强派生 + CLK 守卫保留。本发布 wasm = R077 二进制 + S2 数据段等长手术（rustc 通道不可用期产物，源码 R078 已同义落盘 obsucvim/src/vmgen/template.rs，rustc 恢复后重编刷锚）；NL 像重做登记 R1 余账（V19 §12）",
    size: 178268,
    sha256: "04572e910c7f7bb18da864bb2a51bf161fccc05e15ef0791dd95d9d4ca2654fe",
  },
  {
    name: "R077",
    note: "V19 M2 首包（R1 全量入钥流）：SALT 派生强化（三源 tostring + CLK 守卫采样，fold_key_nl 同式 NL-mix 替代 *31 线性）；bx/bdec、ct/csb(cbase/base)、km 三车道 + SALT 直加 → lane 专属 NL 像 _sv/_svk/_svq（幂二次整除，LC 常量各不相干、obf_num 覆盖、双镜面）；mu/mnp 镜面不对称悬案与 micro ms/_mt R6 纹理明载不入列；no-minify 调试通道 R076 起既有失真（非本批回归），交付 minify 通道为唯一口径",
    size: 178733,
    sha256: "fdcd689ea9868cd4ca3310e8f92b66d50ee083286dfc89bfa8e18cbe39d5e5a3",
  },
  {
    name: "R076",
    note: "V19 M1-bb（R5-b 熵坍缩次批）：字符串池 strseal 链（65521 域 NL+级联）、常量段 ConstLcg/mb 与 ct/csb-CDEC 两链（28-bit NL+级联）、oc_boot 16-bit 线性掩码（NL16+洗序后级联 _ocv）、fv 融合掩码 fuse::fv_mask_nl（NL16，1-based 下标演化）、APH 字母表/TKD 标符节/v15 分块 kn 折叠全换 fold_key_nl；新注入点一律幂二次 (x-(x%D))/D 整除（FLR 部分作用域未绑定）；R5 静态可常折面就此清零（相位三字节表除外，R3 已锁校验）",
    size: 177951,
    sha256: "20ff221c64e34abafa857c96bbd32d1e85b286923de431a905ed769a5091b728",
  },
  {
    name: "R075",
    note: "V19 M1 终锚（历史对照，R076 起已被 M1-bb 改写）：R6+R3+R5-a 齐：R6 指纹熔断 + R3 oracle 熄灭六车道 + R5-a 熵坍缩首批；见 V19 §9.6/§9.7",
    size: 175796,
    sha256: "a968a510b50a623b538c8a4814724feb39eecc87d79ac0583ad243e6354fabc5",
  },
  {
    name: "R075-preview-b",
    note: "V19 M1-b 中间锚点：R3 oracle 熄灭六车道（_bp/_rp/_hp 校验期望值、微码 MZD、QF 折叠、osum/osum2 全部扣 ev_term 入库、运行期 EVK/EVKX/EVKZ/EVK_ 加回；探针 EVK 外置声明修复 do 块词法断层）；Off 件保留旧明文 oracle 并明示勿交付；终态 R075 待 R5 落地后钉",
    size: 175190,
    sha256: "d402d1826c293a0020cde2d68aaf21cd46015378285097e5305cf25c59f17000",
  },
  {
    name: "R075-preview-a",
    note: "V19 M1-a 中间锚点（历史对照，当前引擎已不可复现）：EVKEY 默认翻 On + R6 指纹熔断；Off 自本轮起不再承诺 R073 逐字节恒等",
    size: 174163,
    sha256: "5ebefd82b3f1cc618ceef4636c4bcaf6eb0992a1685202d0d67e96f679f033c2",
  },
  {
    name: "R073",
    note: "体积收敛 R073：_mx 微解释器工厂化（5 份同源体→1 共享函数体+薄包装）+ HW2 别名表折叠；打印探针 231,571→171,881B，md 滚动方言/全 kind 注册不变",
    size: 171881,
    sha256: "b67a06e3f4ba620daaac0b81a3fb266cfb9f78fac9dda614052cbc7738024b9b",
  },
  {
    name: "R072",
    note: "V17 Phase-1b --super-insn 发射端接线收官（on/off 产物字节与 R069/R070/R071 完全相同，差异仅在 wasm 内部接线码）",
    size: 231571,
    sha256: "6c22c008c20883589a9f8ad3503151e2027880ac4454454a975842ba9731e11a",
  },
  {
    name: "R071",
    note: "V17 Phase-1a + --super-insn 演练门面（on/off 产物字节与 R069/R070 相同，差异仅在 wasm 内部规划码；与 R072 同锚点）",
    size: 231571,
    sha256: "6c22c008c20883589a9f8ad3503151e2027880ac4454454a975842ba9731e11a",
  },
  {
    name: "R070",
    note: "v16-a --key-host 固化恒开（--no-key-host 移除；产物字节与 R069 相同，差异仅在 CLI）",
    size: 231571,
    sha256: "6c22c008c20883589a9f8ad3503151e2027880ac4454454a975842ba9731e11a",
  },
  {
    name: "R069",
    note: "v16-a --key-host 翻默认（v15+executor 守卫自动注入 delta 宿主模型；--no-key-host 回退 = R067.5 字节）",
    size: 231571,
    sha256: "6c22c008c20883589a9f8ad3503151e2027880ac4454454a975842ba9731e11a",
  },
  {
    name: "R067.5",
    note: "tier-2 colon member-check uses bracket form (sanitize-rename universal poison fixed)",
    size: 231237,
    sha256: "f11a6cea20548681635f81e0d7e6c84987a12c0dbd5b4f8ab1ec81df72205c42",
  },
  {
    name: "R067.4",
    note: "exguard precheck via guard bindings (no bare getgenv); DTBL via LKP; CIDENT CENV-tolerant",
    size: 231496,
    sha256: "c1887865059cadb9b799f9defbdd132d4d685b720dd2e4787c05db1d8a79f653",
  },
  {
    name: "R067.3",
    note: "tier-2 structural (no exact PRNG floats / parser wording)",
    size: 230177,
    sha256: "70fbc8bb3f5303f4c1d4e279aaecea74bc649fb4068a8026d4aa9bf68c0e4c14",
  },
  {
    name: "R067.2",
    note: "precondition resolves getgenv() then _G (accurate missing list)",
    size: 231271,
    sha256: "8f1b5e21a71e8886704ad79068d8114d386dddd4a9a4145af969b47a984575f5",
  },
  {
    name: "R067.1",
    note: "R067 + hardened-build precondition (clear message instead of poison)",
    size: 230368,
    sha256: "8b43132eb0297a9017689acfb3a1f80950b2fc269a62630dc4cb4483c014c8fb",
  },
  {
    name: "R067",
    note: "full noise density, no precondition",
    size: 229422,
    sha256: "3775f3b557ca4c95ee16d9d013f290d735b6c022af23ac17f1d0b2339fc689f6",
  },
  {
    name: "af7fc32",
    note: "size reduction: noise halved, junk_n 2->1, obf_num 40->28 bit",
    size: 229900,
    sha256: "12c2c05b572d5ede5d8992684ed4aa724096f9254b66d81563848b99fd91d638",
  },
];

if (!existsSync(wasm)) {
  console.error(`engine not found: ${wasm}`);
  process.exit(2);
}

const work = mkdtempSync(path.join(tmpdir(), "obsucvim-verify-"));
try {
  writeFileSync(path.join(work, "in.lua"), 'print("suda")\n');
  const cfg = {
    wasmPath: path.resolve(wasm),
    workDir: work,
    args: [
      "--preset", "v15", "--env-check", "--bind-executor",
      "--seed", "67067", "/work/in.lua", "/work/out.lua",
    ],
  };
  const cfgPath = path.join(work, "cfg.json");
  writeFileSync(cfgPath, JSON.stringify(cfg));

  const proc = spawnSync(process.execPath, [RUNNER, readFileSync(cfgPath, "utf8")], {
    encoding: "utf8",
    timeout: 300000,
  });
  const outPath = path.join(work, "out.lua");
  if (proc.status !== 0 || !existsSync(outPath)) {
    console.error(`engine run failed (rc=${proc.status})`);
    console.error((proc.stderr || "").split("\n").filter((l) => l && !l.includes("ExperimentalWarning")).join("\n"));
    process.exit(1);
  }

  const bytes = statSync(outPath).size;
  const digest = createHash("sha256").update(readFileSync(outPath)).digest("hex");
  const match = ANCHORS.find((a) => a.size === bytes && a.sha256 === digest);

  console.log(`engine   : ${path.resolve(wasm)}`);
  console.log(`probe    : print("suda") --preset v15 --env-check --bind-executor --seed 67067`);
  console.log(`output   : ${bytes} bytes  ${digest}`);
  if (match) {
    console.log(`verdict  : ${match.name} — ${match.note}`);
  } else {
    console.log("verdict  : UNKNOWN build (no matching anchor)");
    console.log("known anchors:");
    for (const anchor of ANCHORS) {
      console.log(`  ${anchor.name.padEnd(8)} ${anchor.size} B  ${anchor.sha256}`);
    }
    process.exitCode = 1;
  }
} finally {
  rmSync(work, { recursive: true, force: true });
}
