# 已解码指令处理器（静态文本）

E 的 22 个槽见主报告。以下只去掉重复的槽别名声明，并将 MS[n] 替换为已恢复的字符串；未执行这些处理器。

## 00 — EQ（j[20]）

```lua
local x = V[b + 1];
local y = V[c + 1];
local tx = TYP(x);
local ty = TYP(y);
if tx == ty and (tx == "number" or tx == "string" or tx == "boolean" or tx == "nil") then V[a + 1] = x == y else local f = mget(x, "__eq") or mget(y, "__eq");
if f then V[a + 1] = f(x, y) else V[a + 1] = x == y end end
```

## 01 — APPEND_ONE（j[26]）

```lua
local t = V[a + 1];
local n = V[b + 1];
t[n + 1] = V[c + 1];
V[b + 1] = n + 1
```

## 02 — NEWTABLE（j[23]）

```lua
V[a + 1] = {}
```

## 03 — LT（j[16]）

```lua
local x = V[b + 1];
local y = V[c + 1];
local tx = TYP(x);
local ty = TYP(y);
if tx == "number" and ty == "number" then V[a + 1] = x < y elseif tx == "string" and ty == "string" then V[a + 1] = x < y else local f = mget(x, "__lt") or mget(y, "__lt");
if f then V[a + 1] = f(x, y) else ERR("attempt to compare " .. tx .. " with " .. ty, 0) end end
```

## 04 — FLOORDIV（j[22]）

```lua
local q = V[b + 1] / V[c + 1];
V[a + 1] = FLOOR(q)
```

## 05 — JMP_IF_FALSE（j[1]）

```lua
local t = V[a + 1];
if not t then return {j = b} end
```

## 06 — VARARG_COUNT（j[31]）

```lua
V[a + 1] = vargc
```

## 07 — NOT（j[14]）

```lua
local t = V[b + 1];
V[a + 1] = not t
```

## 08 — MUL（j[8]）

```lua
local x = V[b + 1];
local y = V[c + 1];
local tx = TYP(x);
local ty = TYP(y);
if tx == "number" and ty == "number" then V[a + 1] = x * y else local f = mget(x, "__mul") or mget(y, "__mul");
if f then V[a + 1] = f(x, y) else ERR("attempt to perform arithmetic on a " .. tx .. " value", 0) end end
```

## 09 — GE（j[19]）

```lua
local u = V[b + 1];
local w = V[c + 1];
if TYP(u) == "number" and TYP(w) == "number" then V[a + 1] = u >= w elseif TYP(u) == "string" and TYP(w) == "string" then V[a + 1] = u >= w else local f = mget(u, "__le") or mget(w, "__le");
if f then V[a + 1] = f(w, u) else ERR("attempt to compare " .. TYP(u) .. " with " .. TYP(w), 0) end end
```

## 10 — MOVE（j[5]）

```lua
local t = V[b + 1];
V[a + 1] = t
```

## 11 — NE（j[21]）

```lua
local x = V[b + 1];
local y = V[c + 1];
local tx = TYP(x);
local ty = TYP(y);
local eqv;
if tx == ty and (tx == "number" or tx == "string" or tx == "boolean" or tx == "nil") then eqv = x == y else local f = mget(x, "__eq") or mget(y, "__eq");
if f then eqv = f(x, y) else eqv = x == y end end;
V[a + 1] = not eqv
```

## 12 — SETUPVAL（j[36]）

```lua
local u = ups[a + 1];
u.v[u.i] = V[b + 1]
```

## 13 — POW（j[11]）

```lua
local u = V[b + 1];
local w = V[c + 1];
if TYP(u) == "number" and TYP(w) == "number" then V[a + 1] = u ^ w else local f = mget(u, "__pow") or mget(w, "__pow");
if f then V[a + 1] = f(u, w) else ERR("attempt to perform arithmetic on a " .. TYP(u) .. " value", 0) end end
```

## 14 — NOP（j[38]）

```lua
local _ = c * d
```

## 15 — CALL_APPEND（j[27]）

```lua
local f = V[S[a + 1]];
local fn, selfv = resolve_call(f);
local off = selfv and 1 or 0;
local nfixed = FLOOR(d / 2);
local tail = d % 2 == 1;
local ntail = tail and V[S[a + nfixed + 2]] or 0;
local nargs = nfixed + off + ntail;
local av = {};
if off == 1 then av[1] = f end;
for i = 1, nfixed + ntail do if tail and i > nfixed then local x = a + i + 2;
local s = S[x];
if s then av[off + i] = V[s] else av[off + i] = O[x] end else av[off + i] = V[S[a + i + 1]] end end;
local tb = V[b + 1];
local n0 = V[c + 1];
local ov, nout = callcap(fn, av, nargs);
for i = 1, nout do tb[n0 + i] = ov[i] end;
V[c + 1] = n0 + nout;
local _ = c * a - d
```

## 16 — RETURN（j[37]）

```lua
local out = {};
local n = b;
local total;
if n == 255 then local pre = d;
for i = 1, pre do out[i] = V[S[a + i]] end;
if c == 1 then for i = 1, vargc do out[pre + i] = vargs[i] end;
total = pre + vargc else for i = 1, E.ln do local s = S[E.lb + i];
if s then out[pre + i] = V[s] else out[pre + i] = O[E.lb + i] end end;
total = pre + E.ln end else for i = 1, n do out[i] = V[S[a + i]] end;
total = n end;
return {out, total}
```

## 17 — GETGLOBAL（j[33]）

```lua
V[a + 1] = G[C[b + 1]]
```

## 18 — MOD（j[10]）

```lua
local x = V[b + 1];
local y = V[c + 1];
if TYP(x) == "number" and TYP(y) == "number" then V[a + 1] = x % y else local f = mget(x, "__mod") or mget(y, "__mod");
if f then V[a + 1] = f(x, y) else ERR("attempt to perform arithmetic on a " .. TYP(x) .. " value", 0) end end
```

## 19 — CALL_WITH_TAIL（j[40]）

```lua
local base = a + 1;
local f = V[S[base]];
local fn, selfv = resolve_call(f);
local off = selfv and 1 or 0;
local nfixed = b;
local ntail = V[S[base + nfixed + 1]];
local nargs = nfixed + ntail + off;
local args = {};
if off == 1 then args[1] = f end;
for i = 1, nfixed + ntail do if i <= nfixed then args[off + i] = V[S[base + i]] else local x = base + i + 1;
local s = S[x];
if s then args[off + i] = V[s] else args[off + i] = O[x] end end end;
if d == 1 then for i = 1, vargc do args[nargs + i] = vargs[i] end;
nargs = nargs + vargc end;
local out, nout = callcap(fn, args, nargs);
local nres = c;
E.lb = a + 1;
E.ln = nout;
local wn = nout;
if nres ~= 255 and nres > wn then wn = nres end;
for i = 1, wn do local s = S[base + i];
if s then V[s] = out[i] else O[base + i] = out[i] end end;
local _ = (a + c) * (b - d)
```

## 20 — GETUPVAL（j[35]）

```lua
local uc = ups[b + 1];
V[a + 1] = uc.v[uc.i]
```

## 21 — APPEND_VARARGS（j[32]）

```lua
local t = V[a + 1];
local n = V[b + 1];
for i = 1, vargc do t[n + i] = vargs[i] end;
V[b + 1] = n + vargc
```

## 22 — LOADNIL_RANGE（j[3]）

```lua
for i = 1, b do V[S[a + i]] = nil end
```

## 23 — CALL（j[29]）

```lua
local base = a + 1;
local f = V[S[base]];
local fn, selfv = resolve_call(f);
local off = selfv and 1 or 0;
local nargs = b + off;
local args = {};
if off == 1 then args[1] = f end;
for i = 1, b do args[off + i] = V[S[base + i]] end;
if d == 1 then for i = 1, vargc do args[nargs + i] = vargs[i] end;
nargs = nargs + vargc end;
local out, nout = callcap(fn, args, nargs);
local nres = c;
E.lb = a + 1;
E.ln = nout;
local wn = nout;
if nres ~= 255 and nres > wn then wn = nres end;
for i = 1, wn do local s = S[base + i];
if s then V[s] = out[i] else O[base + i] = out[i] end end;
local _ = a * d + b * c
```

## 24 — LOAD_SHORT_STRING（j[41]）

```lua
local ms = (63371 * a + 41221) % 65536;
local q1 = (b - ms) % 65536;
local q2 = (c - ms) % 65536;
local q3 = (d - ms) % 65536;
local n = FLOOR(q3 / 256);
local t = {};
t[1] = q1 % 256;
t[2] = FLOOR(q1 / 256);
t[3] = q2 % 256;
t[4] = FLOOR(q2 / 256);
t[5] = q3 % 256;
local s = "";
local i = 1;
while i <= n do s = s .. CHAR(t[i]);
i = i + 1 end;
V[a + 1] = s
```

## 25 — SETTABLE（j[25]）

```lua
local t = V[a + 1];
local k = V[b + 1];
local v = V[c + 1];
if TYP(t) ~= "table" then local mt = GMT(t);
local f = mt and mt["__newindex"];
if TYP(f) == "function" then f(t, k, v) elseif f ~= nil then RSET(f, k, v) else ERR("attempt to index a " .. TYP(t) .. " value", 0) end else if RGET(t, k) == nil then local f = mget(t, "__newindex");
if TYP(f) == "function" then f(t, k, v) elseif f ~= nil then RSET(f, k, v) else RSET(t, k, v) end else RSET(t, k, v) end end
```

## 26 — JMP_IF_TRUE（j[2]）

```lua
if V[a + 1] then return {j = b} end
```

## 27 — GT（j[18]）

```lua
local x = V[b + 1];
local y = V[c + 1];
local tx = TYP(x);
local ty = TYP(y);
if tx == "number" and ty == "number" then V[a + 1] = x > y elseif tx == "string" and ty == "string" then V[a + 1] = x > y else local f = mget(x, "__lt") or mget(y, "__lt");
if f then V[a + 1] = f(y, x) else ERR("attempt to compare " .. tx .. " with " .. ty, 0) end end
```

## 28 — CLOSURE（j[28]）

```lua
V[a + 1] = makefn(b + 1, V, ups, S)
```

## 29 — JMP（j[0]）

```lua
return {j = b}
```

## 30 — UNM（j[13]）

```lua
local x = V[b + 1];
local tx = TYP(x);
if tx == "number" then V[a + 1] = -x else local f = mget(x, "__unm");
if f then V[a + 1] = f(x) else ERR("attempt to perform arithmetic on a " .. tx .. " value", 0) end end
```

## 31 — CALL_COUNTED（j[39]）

```lua
local bs = a + 1;
local f = V[S[bs]];
local fn, selfv = resolve_call(f);
local off = selfv and 1 or 0;
local nfixed = b;
local varg = d % 2 == 1;
local tail = d >= 2;
local nargs = nfixed + off;
if tail then nargs = nargs + V[S[bs + nfixed + 1]] end;
local av = {};
if off == 1 then av[1] = f end;
for i = 1, (tail and (nfixed + V[S[bs + nfixed + 1]]) or nfixed) do if tail and i > nfixed then local x = bs + i + 1;
local s = S[x];
if s then av[off + i] = V[s] else av[off + i] = O[x] end else av[off + i] = V[S[bs + i]] end end;
if varg then for i = 1, vargc do av[nargs + i] = vargs[i] end;
nargs = nargs + vargc end;
local ov, nout = callcap(fn, av, nargs);
for i = 1, nout do local s = S[bs + i];
if s then V[s] = ov[i] else O[bs + i] = ov[i] end end;
V[S[bs]] = nout;
local _ = c * a - d
```

## 32 — GETTABLE（j[24]）

```lua
local o = V[b + 1];
local j = V[c + 1];
local r;
if TYP(o) == "table" then r = RGET(o, j);
if r == nil then local f = mget(o, "__index");
if TYP(f) == "function" then r = f(o, j) elseif f ~= nil then r = f[j] end end else local m0 = GMT(o);
if m0 and m0["__index"] ~= nil then local f = m0["__index"];
if TYP(f) == "function" then r = f(o, j) else r = f[j] end else ERR("attempt to index a " .. TYP(o) .. " value", 0) end end;
V[a + 1] = r
```

## 33 — PACK_VARARGS（j[30]）

```lua
local t = {};
local i = 1;
while i <= vargc do t[i] = vargs[i];
i = i + 1 end;
V[a + 1] = t
```

## 34 — DIV（j[9]）

```lua
local x = V[b + 1];
local y = V[c + 1];
local tx = TYP(x);
local ty = TYP(y);
if tx == "number" and ty == "number" then V[a + 1] = x / y else local f = mget(x, "__div") or mget(y, "__div");
if f then V[a + 1] = f(x, y) else ERR("attempt to perform arithmetic on a " .. tx .. " value", 0) end end
```

## 35 — LEN（j[15]）

```lua
local x = V[b + 1];
local f = HAS_LEN_META and mget(x, "__len");
if f then V[a + 1] = f(x) else V[a + 1] = #x end
```

## 36 — LE（j[17]）

```lua
local x = V[b + 1];
local y = V[c + 1];
local tx = TYP(x);
local ty = TYP(y);
if tx == "number" and ty == "number" then V[a + 1] = x <= y elseif tx == "string" and ty == "string" then V[a + 1] = x <= y else local f = mget(x, "__le") or mget(y, "__le");
if f then V[a + 1] = f(x, y) else ERR("attempt to compare " .. tx .. " with " .. ty, 0) end end
```

## 37 — LOADK（j[4]）

```lua
local t = C[b + 1];
V[a + 1] = t
```

## 38 — SUB（j[7]）

```lua
local x = V[b + 1];
local y = V[c + 1];
local tx = TYP(x);
local ty = TYP(y);
if tx == "number" and ty == "number" then V[a + 1] = x - y else local f = mget(x, "__sub") or mget(y, "__sub");
if f then V[a + 1] = f(x, y) else ERR("attempt to perform arithmetic on a " .. tx .. " value", 0) end end
```

## 39 — CONCAT（j[12]）

```lua
local x = V[b + 1];
local y = V[c + 1];
local tx = TYP(x);
local ty = TYP(y);
if (tx == "number" or tx == "string") and (ty == "number" or ty == "string") then V[a + 1] = x .. y else local f = mget(x, "__concat") or mget(y, "__concat");
if f then V[a + 1] = f(x, y) else ERR("attempt to perform concatenation on a " .. tx .. " value", 0) end end
```

## 40 — ADD（j[6]）

```lua
local x = V[b + 1];
local y = V[c + 1];
local tx = TYP(x);
local ty = TYP(y);
if tx == "number" and ty == "number" then V[a + 1] = x + y else local f = mget(x, "__add") or mget(y, "__add");
if f then V[a + 1] = f(x, y) else ERR("attempt to perform arithmetic on a " .. tx .. " value", 0) end end
```

## 41 — SETGLOBAL（j[34]）

```lua
G[C[b + 1]] = V[a + 1]
```

## 42 — NOP_PATCH（补丁专用）

```lua
local _ = c * d
```
