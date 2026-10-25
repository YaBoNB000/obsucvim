--!strict
-- Obsucvim demo: a small game module with license check, math, and tables
local GameConfig = {
    version = "2.1.0",
    max_players = 32,
    tick_rate = 1 / 60,
    features = { "pvp", "coop", "spectate" },
}

local License = {}
local SIGNATURE = "obsucvim_demo_2026"

local function digest(input: string): number
    local h = 5381
    for i = 1, #input do
        h = (h * 33 + string.byte(input, i)) % 2147483647
    end
    return h
end

function License.verify(key: string): boolean
    local parts = {}
    for seg in key:gmatch("[^-]+") do
        table.insert(parts, seg)
    end
    if #parts ~= 3 then return false end
    return digest(parts[1] .. parts[2]) == tonumber(parts[3])
        and parts[2]:sub(1, 4) == SIGNATURE:sub(1, 4)
end

local Player = {}
Player.__index = Player

function Player.new(name: string, score: number)
    return setmetatable({
        name = name,
        score = score,
        kills = 0,
        deaths = 0,
        active = true,
    }, Player)
end

function Player:getKDR(): number
    if self.deaths == 0 then return self.kills end
    return self.kills / self.deaths
end

function Player:addKill()
    self.kills += 1
    self.score += 100
end

function Player:deactivate()
    self.active = false
end

-- Simulate a match
local function runMatch(players: { any }): number
    local total_score = 0
    for _, p in ipairs(players) do
        if p.active then
            p:addKill()
            total_score += p.score
        end
    end
    return total_score
end

-- Fibonacci for benchmark
local function fib(n: number): number
    if n <= 1 then return n end
    local a, b = 0, 1
    for _ = 2, n do
        a, b = b, a + b
    end
    return b
end

local roster = {}
for i = 1, 8 do
    table.insert(roster, Player.new("player_" .. tostring(i), i * 50))
end

local total = runMatch(roster)
local kdr = roster[1]:getKDR()
local bench = fib(30)

print(("game=%s v%s total=%d kdr=%.2f fib30=%d"):format(
    GameConfig.features[1],
    GameConfig.version,
    total,
    kdr,
    bench
))

if License.verify("abc123-obsucvim_demo-12345678") then
    print("license: valid")
else
    print("license: invalid")
end
