-- kkksu-mini: 斐波那契打印（v18-check 同族小件，真机冒烟用）
local function fib(n) if n < 2 then return n end return fib(n-1) + fib(n-2) end
local t = {}
for i = 1, 10 do t[i] = fib(i) end
print("kkksu-mini:" .. table.concat(t, ","))
