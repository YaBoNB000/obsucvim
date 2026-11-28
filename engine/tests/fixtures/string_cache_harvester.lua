-- Generic string-cache harvester (Phase 18 P1-1/P1-2 test fixture).
--
-- This is the "off-the-shelf harvester" the Phase 18 threat model talks about.
-- It does two independent things, because they answer two different questions:
--
--  1. It globally replaces `string.char` with a recorder and, after the
--     artifact has run, rebuilds every decoded string from the recorded
--     arguments with `table.concat` + pattern matching. This is the
--     "generic script kiddie harvester": it understands nothing about the VM.
--  2. It reports what the VM actually stored in its string cache. Those
--     channels (OBSUCVIM_SC_DUMP / OBSUCVIM_SC_CLR) are audit-only and are
--     compiled out of production artifacts, so section 2 is only meaningful
--     against `--features audit-hooks` builds.
--
-- Output protocol (one record per line, parsed by the Python harness):
--   RUN_OK:<true|false>
--   H:<value>   one line per string rebuilt from the string.char hook
--   D:<value>   one line per cache write observed
--   C:<value>   one line per cache clear observed
OBSUCVIM_SC_DUMP = {}
OBSUCVIM_SC_CLR = {}

local CHAR_ARGS = {}
local REAL_CHAR = string.char
string.char = function(...)
	local n = select("#", ...)
	local row = table.pack(...)
	row.n = n
	CHAR_ARGS[#CHAR_ARGS + 1] = row
	return REAL_CHAR(...)
end

local ok, err = pcall(function()
	return loadstring(ART, "artifact")()
end)
print("RUN_OK:" .. tostring(ok))
if not ok then
	print("ERR:" .. tostring(err))
end
string.char = REAL_CHAR

print("HARVEST_BEGIN")
for i = 1, #CHAR_ARGS do
	local row = CHAR_ARGS[i]
	local parts = table.create(row.n or #row)
	for j = 1, row.n or #row do
		parts[j] = string.char(row[j])
	end
	print("H:" .. table.concat(parts))
end
print("HARVEST_END")

print("DUMP_BEGIN")
for _, v in ipairs(OBSUCVIM_SC_DUMP) do
	print("D:" .. tostring(v))
end
print("DUMP_END")
print("CLR_BEGIN")
for _, v in ipairs(OBSUCVIM_SC_CLR) do
	print("C:" .. tostring(v))
end
print("CLR_END")
