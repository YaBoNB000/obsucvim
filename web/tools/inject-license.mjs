#!/usr/bin/env node
/**
 * Inject the Obsucvim license guard into a Luau source file.
 *
 *   node tools/inject-license.mjs --source in.lua --script-id scr_<24 hex> --out guarded.lua
 *        [--endpoint https://obsucvim.l2.ink/api/licenses/validate]
 *
 * The guard is prepended (never appended) so the license check runs before any user
 * code. The project ID is public; no server secret is embedded. Feed the guarded file
 * to the obfuscator so the injected check is protected together with user code.
 */
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const TEMPLATE = path.join(__dirname, "license-guard.luau");
const SCRIPT_ID_RE = /^scr_[\da-f]{24}$/i;
const MAX_SOURCE_BYTES = 1024 * 1024;

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 2) {
    const flag = argv[i];
    const value = argv[i + 1];
    if (!flag?.startsWith("--") || value === undefined) {
      throw new Error(`invalid argument: ${flag ?? ""}`);
    }
    args[flag.slice(2)] = value;
  }
  return args;
}

function usage(message) {
  if (message) console.error(`error: ${message}`);
  console.error(
    "usage: node tools/inject-license.mjs --source <file> --script-id <scr_...> --out <file> " +
    "[--endpoint <https url>]",
  );
  process.exit(1);
}

const args = parseArgs(process.argv.slice(2));
if (!args.source || !args["script-id"] || !args.out) usage("missing required arguments");
if (!SCRIPT_ID_RE.test(args["script-id"])) usage("--script-id must look like scr_<24 hex>");

const endpoint = args.endpoint ?? "https://obsucvim.l2.ink/api/licenses/validate";
const isHttps = /^https:\/\/[^\s"']+$/i.test(endpoint);
const isLoopbackHttp = /^http:\/\/(127\.0\.0\.1|localhost|\[::1\])(:\d+)?\/[^\s"']*$/i.test(endpoint);
if (!isHttps && !isLoopbackHttp) usage("--endpoint must be an https URL (http is allowed only for loopback testing)");
if (isLoopbackHttp) console.warn("warning: loopback HTTP endpoint accepted for testing only; production releases must use https");

const source = readFileSync(args.source, "utf8");
if (Buffer.byteLength(source, "utf8") > MAX_SOURCE_BYTES) {
  usage("source exceeds the 1 MB limit");
}
if (source.includes("__obsucvim_license")) {
  usage("source already contains a license guard; remove it first");
}

const guard = readFileSync(TEMPLATE, "utf8")
  .replaceAll("__OBSUCVIM_SCRIPT_ID__", args["script-id"])
  .replaceAll("__OBSUCVIM_ENDPOINT__", endpoint);

const assignmentsOk =
  guard.includes(`local guardScript = "${args["script-id"]}"`) &&
  guard.includes(`local guardUrl = "${endpoint}"`);
if (!assignmentsOk) usage("guard template has unreplaced placeholders");

const banner = "-- Injected by Obsucvim license guard v1 (runs before user code)\n";
const output = `${banner}${guard}\n${source}`;
writeFileSync(args.out, output, "utf8");

console.log(`guard:    ${path.relative(process.cwd(), TEMPLATE)}`);
console.log(`scriptId: ${args["script-id"]}`);
console.log(`endpoint: ${endpoint}`);
console.log(`input:    ${Buffer.byteLength(source, "utf8")} B`);
console.log(`output:   ${Buffer.byteLength(output, "utf8")} B -> ${args.out}`);
console.log("next: obfuscate the guarded file; the injected check is protected with the rest of the source.");
