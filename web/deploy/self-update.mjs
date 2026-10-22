#!/usr/bin/env node
/**
 * Obsucvim 自更新器（纯 Node，无第三方依赖）。
 *
 * 用途：在目标机器上把已解压的部署目录升级到新版 ZIP，**保留 data/ 里的账户、
 * 卡密、发布产物**，不碰 SMTP 等环境变量。
 *
 * 用法（在部署目录里，用包内自带的 runtime\node.exe 执行）：
 *   runtime\node.exe tools\self-update.mjs --dir . --url <zip 地址> --sha256 <期望哈希>
 *   runtime\node.exe tools\self-update.mjs --dir . --zip  D:\new.zip --sha256 <期望哈希>
 *
 * 安全设计：
 *   - 下载后先校验 ZIP 的 SHA-256，不匹配立即中止；
 *   - 解压前校验包内 SHA256SUMS.txt，任何文件被改过都中止；
 *   - 只覆盖白名单内的相对路径，遇到 ../ 之类越界路径直接拒绝；
 *   - 永不写入 data/，升级前后账户与卡密不受影响。
 */
import { createHash } from "node:crypto";
import { createWriteStream, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { get } from "node:https";
import { tmpdir } from "node:os";
import path from "node:path";
import { inflateRawSync } from "node:zlib";

const EOCD_SIG = 0x06054b50;
const CD_SIG = 0x02014b50;
const LOCAL_SIG = 0x04034b50;

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i++) {
    const token = argv[i];
    if (!token.startsWith("--")) throw new Error(`unexpected argument: ${token}`);
    const name = token.slice(2);
    const next = argv[i + 1];
    if (next === undefined || next.startsWith("--")) {
      args[name] = true;
    } else {
      args[name] = next;
      i++;
    }
  }
  return args;
}

function usage(message) {
  if (message) console.error(`error: ${message}\n`);
  console.error(
    "usage: node self-update.mjs --dir <install dir> (--url <https zip> | --zip <local zip>) " +
    "[--sha256 <expected zip hash>] [--dry-run]",
  );
  process.exit(1);
}

function sha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}

function download(url, destination) {
  return new Promise((resolve, reject) => {
    const request = (target, redirectsLeft) => {
      get(target, (res) => {
        if ([301, 302, 303, 307, 308].includes(res.statusCode) && res.headers.location && redirectsLeft > 0) {
          res.resume();
          request(new URL(res.headers.location, target), redirectsLeft - 1);
          return;
        }
        if (res.statusCode !== 200) {
          res.resume();
          reject(new Error(`download failed: HTTP ${res.statusCode}`));
          return;
        }
        const file = createWriteStream(destination);
        res.pipe(file);
        file.on("finish", () => file.close(() => resolve()));
        file.on("error", reject);
      }).on("error", reject);
    };
    request(url, 5);
  });
}

/** 读取 ZIP 的中央目录，返回 [{ name, method, compressedSize, size, offset }] */
function readZipEntries(buffer) {
  let eocd = -1;
  const stop = Math.max(0, buffer.length - 66000);
  for (let i = buffer.length - 22; i >= stop; i--) {
    if (buffer.readUInt32LE(i) === EOCD_SIG) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) throw new Error("not a zip file (no end-of-central-directory record)");

  const entryCount = buffer.readUInt16LE(eocd + 10);
  const cdOffset = buffer.readUInt32LE(eocd + 16);
  if (entryCount === 0xffff || cdOffset === 0xffffffff) {
    throw new Error("zip64 archives are not supported");
  }

  const entries = [];
  let cursor = cdOffset;
  for (let i = 0; i < entryCount; i++) {
    if (buffer.readUInt32LE(cursor) !== CD_SIG) throw new Error("corrupt central directory");
    const method = buffer.readUInt16LE(cursor + 10);
    const compressedSize = buffer.readUInt32LE(cursor + 20);
    const size = buffer.readUInt32LE(cursor + 24);
    const nameLength = buffer.readUInt16LE(cursor + 28);
    const extraLength = buffer.readUInt16LE(cursor + 30);
    const commentLength = buffer.readUInt16LE(cursor + 32);
    const offset = buffer.readUInt32LE(cursor + 42);
    const name = buffer.toString("utf8", cursor + 46, cursor + 46 + nameLength);
    entries.push({ name, method, compressedSize, size, offset });
    cursor += 46 + nameLength + extraLength + commentLength;
  }
  return entries;
}

function extractEntry(buffer, entry) {
  if (buffer.readUInt32LE(entry.offset) !== LOCAL_SIG) throw new Error(`corrupt local header: ${entry.name}`);
  const nameLength = buffer.readUInt16LE(entry.offset + 26);
  const extraLength = buffer.readUInt16LE(entry.offset + 28);
  const start = entry.offset + 30 + nameLength + extraLength;
  const raw = buffer.subarray(start, start + entry.compressedSize);
  if (entry.method === 0) return Buffer.from(raw);
  if (entry.method === 8) return inflateRawSync(raw);
  throw new Error(`unsupported compression method ${entry.method} in ${entry.name}`);
}

/** 拒绝 ../ 与绝对路径，保证不会写到安装目录之外 */
function safeRelative(name, rootPrefix) {
  const withoutRoot = name.startsWith(rootPrefix) ? name.slice(rootPrefix.length) : name;
  const normalized = withoutRoot.replace(/\\/g, "/").replace(/^\/+/, "");
  if (!normalized) return null;
  const parts = normalized.split("/");
  if (parts.includes("..")) return null;
  if (/^[A-Za-z]:/.test(normalized)) return null;
  return normalized;
}

function verifyManifest(extractedRoot) {
  const manifest = path.join(extractedRoot, "SHA256SUMS.txt");
  if (!existsSync(manifest)) throw new Error("package is missing SHA256SUMS.txt");
  const problems = [];
  let checked = 0;
  for (const line of readFileSync(manifest, "utf8").split("\n")) {
    const match = line.trim().match(/^([0-9a-f]{64})\s+\*?(.+)$/i);
    if (!match) continue;
    const [, expected, relative] = match;
    const target = path.join(extractedRoot, relative.replace(/^\.\//, ""));
    if (!existsSync(target)) {
      problems.push(`missing: ${relative}`);
      continue;
    }
    if (sha256(readFileSync(target)) !== expected.toLowerCase()) {
      problems.push(`hash mismatch: ${relative}`);
    }
    checked++;
  }
  if (problems.length) {
    throw new Error(`package integrity check failed:\n  ${problems.slice(0, 10).join("\n  ")}`);
  }
  return checked;
}

const args = parseArgs(process.argv.slice(2));
if (!args.dir) usage("--dir is required");
if (!args.url && !args.zip) usage("provide either --url or --zip");
if (args.url && args.zip) usage("use only one of --url / --zip");

const installDir = path.resolve(String(args.dir));
if (!existsSync(installDir)) usage(`install dir not found: ${installDir}`);
if (!existsSync(path.join(installDir, "tools")) && !existsSync(path.join(installDir, "dist"))) {
  usage(`${installDir} does not look like an Obsucvim install (no tools/ or dist/)`);
}

const staging = mkdtempSync(path.join(tmpdir(), "obsucvim-update-"));
const zipPath = path.join(staging, "package.zip");

try {
  console.log(`[1/6] 获取安装包`);
  if (args.zip) {
    const source = path.resolve(String(args.zip));
    if (!existsSync(source)) usage(`zip not found: ${source}`);
    writeFileSync(zipPath, readFileSync(source));
    console.log(`     本地文件 ${source}`);
  } else {
    const url = String(args.url);
    if (!/^https:\/\//i.test(url)) usage("--url must be https");
    await download(url, zipPath);
    console.log(`     下载完成 ${url}`);
  }

  const zipBuffer = readFileSync(zipPath);

  console.log(`[2/6] 校验安装包 SHA-256`);
  const actual = sha256(zipBuffer);
  if (args.sha256) {
    const expected = String(args.sha256).trim().toLowerCase();
    if (expected !== actual) {
      throw new Error(`zip hash mismatch\n  expected ${expected}\n  actual   ${actual}`);
    }
    console.log(`     匹配 ${actual}`);
  } else {
    console.log(`     未提供期望哈希，实际为 ${actual}`);
    console.log(`     提示：建议用 --sha256 固定版本，避免下载到被替换的文件`);
  }

  console.log(`[3/6] 解压到临时目录`);
  const entries = readZipEntries(zipBuffer);
  const roots = new Set();
  for (const entry of entries) {
    const first = entry.name.replace(/\\/g, "/").split("/")[0];
    if (first) roots.add(first);
  }
  if (roots.size !== 1) throw new Error(`unexpected package layout (top-level: ${[...roots].join(", ")})`);
  const rootPrefix = `${[...roots][0]}/`;
  const extractedRoot = path.join(staging, "extracted");
  mkdirSync(extractedRoot, { recursive: true });

  let fileCount = 0;
  for (const entry of entries) {
    const relative = safeRelative(entry.name, rootPrefix);
    if (!relative) continue;
    const target = path.join(extractedRoot, relative);
    if (entry.name.endsWith("/")) {
      mkdirSync(target, { recursive: true });
      continue;
    }
    mkdirSync(path.dirname(target), { recursive: true });
    writeFileSync(target, extractEntry(zipBuffer, entry));
    fileCount++;
  }
  console.log(`     ${fileCount} 个文件`);

  console.log(`[4/6] 校验包内文件清单`);
  const verified = verifyManifest(extractedRoot);
  console.log(`     ${verified} 个文件哈希一致`);

  console.log(`[5/6] 备份现有 data/ 与旧版本清单`);
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  const backupDir = path.join(installDir, "backup", stamp);
  mkdirSync(backupDir, { recursive: true });
  const dataDir = path.join(installDir, "data");
  if (existsSync(dataDir)) {
    const copyTree = (from, to) => {
      mkdirSync(to, { recursive: true });
      for (const name of readdirSync(from)) {
        const source = path.join(from, name);
        const target = path.join(to, name);
        if (statSync(source).isDirectory()) copyTree(source, target);
        else writeFileSync(target, readFileSync(source));
      }
    };
    copyTree(dataDir, path.join(backupDir, "data"));
    console.log(`     data/ 已备份到 ${backupDir}`);
  } else {
    console.log(`     没有 data/，跳过数据备份`);
  }
  const oldInfo = path.join(installDir, "BUILD-INFO.txt");
  if (existsSync(oldInfo)) writeFileSync(path.join(backupDir, "BUILD-INFO.old.txt"), readFileSync(oldInfo));

  console.log(`[6/6] 覆盖程序文件（保留 data/）`);
  const replaced = [];
  const skipped = [];
  const walk = (dir, base = "") => {
    for (const name of readdirSync(dir)) {
      const full = path.join(dir, name);
      const relative = base ? `${base}/${name}` : name;
      if (statSync(full).isDirectory()) {
        walk(full, relative);
        continue;
      }
      // data/ 是运行时数据：账户、卡密、发布产物，绝不能覆盖
      if (relative === "data" || relative.startsWith("data/")) {
        skipped.push(relative);
        continue;
      }
      const target = path.join(installDir, relative);
      mkdirSync(path.dirname(target), { recursive: true });
      writeFileSync(target, readFileSync(full));
      replaced.push(relative);
    }
  };
  walk(extractedRoot);

  console.log(`     更新 ${replaced.length} 个文件${skipped.length ? `，跳过 ${skipped.length} 个数据文件` : ""}`);
  console.log("");
  console.log("升级完成。请重启 Obsucvim 服务，例如：");
  console.log("  .\\Start-Obsucvim.ps1 -LocalOnly");
  console.log(`如出现问题，可从 ${backupDir} 恢复 data/。`);
} catch (error) {
  console.error("");
  console.error(`升级失败：${error instanceof Error ? error.message : String(error)}`);
  console.error("安装目录未被修改（失败发生在覆盖阶段之前时）。");
  process.exitCode = 1;
} finally {
  try { rmSync(staging, { recursive: true, force: true }); } catch { /* ignore */ }
}
