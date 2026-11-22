local function values()
    return "first", "second", "third"
end

local a, b, c = values()
local one, missing = (values())
local expanded = {values()}
local grouped = {(values())}

local function arity(...)
    return select("#", ...)
end

local function sum(...)
    local total = 0
    for i = 1, select("#", ...) do
        total = total + select(i, ...)
    end
    return total
end

local function call_arity(...)
    return arity(...)
end

local nested_newindex = setmetatable({}, {
    __newindex = function(t, key, value)
        rawset(t, key, "nested:" .. tostring(value))
    end,
})
local index_proxy = setmetatable({}, {
    __index = function(_, key)
        return "proxy:" .. tostring(key)
    end,
})
local proxy = setmetatable({}, {
    __index = index_proxy,
    __newindex = nested_newindex,
})
proxy.answer = 7

local locked_meta = {
    __add = function(x, y) return x.value + y.value end,
    __idiv = function(x, y) return x.value // y.value end,
    __unm = function(x) return -x.value end,
    __concat = function(x, y) return tostring(x.value) .. ":" .. tostring(y.value) end,
    __metatable = "locked",
}
local left = setmetatable({value = 17}, locked_meta)
local right = setmetatable({value = 5}, locked_meta)

local long = [==[
long string with 'single', "double", and \\ slash
second line
]==]
local long_expected = [==[long string with 'single', "double", and \\ slash
second line
]==]

local function emit(...)
    print(...)
end
((emit))("parenthesized callee", "ok")

print("returns", a, b, c, one, missing, #expanded, #grouped)
print("arity", arity(values()), arity((values())), call_arity((values())), sum((1), (2), (3)))
print("table-meta", proxy.answer, nested_newindex.answer, proxy.unknown)
print("operators", "12" + "3", "9" // "2", left + right, left // right, -left, left .. right)
print("long", #long, long == long_expected)

local function report_error(label, fn)
    local ok, err = pcall(fn)
    local detail = tostring(err)
    detail = string.match(detail, "attempt to .*$") or detail
    print("error", label, ok, detail)
end

report_error("add", function() return true + 2 end)
report_error("sub", function() return true - 2 end)
report_error("mul", function() return true * 2 end)
report_error("div", function() return true / 2 end)
report_error("mod", function() return true % 2 end)
report_error("pow", function() return true ^ 2 end)
report_error("idiv", function() return true // 2 end)
report_error("unm", function() return -false end)
report_error("numeric-string-type", function() return "4" + false end)
report_error("same-type", function() return "x" + "y" end)
report_error("index", function() return (42)["foo"] end)
report_error("newindex", function() local n = 42; n["foo"] = 1 end)
report_error("concat", function() return false .. {} end)
report_error("compare-mixed", function() return false < 1 end)
report_error("compare-same", function() return false < true end)
report_error("bad-meta", function()
    local x = setmetatable({}, {__add = false})
    return x + 1
end)
print("VM-SEMANTICS-OK")
