本目录下的测试脚本编写时只参考以下公开资料里描述的通用 Lua/Luau 反混淆技术，
不查看 `engine/src/vmgen/` 下任何具体实现代码：

1. 常量折叠（constant folding）：识别并求值编译期可确定的常量表达式。
2. 死代码消除（dead code elimination）：识别不可达分支/从不被读取的赋值。
3. 基于执行轨迹的动态去虚拟化（trace-based devirtualization）：通过记录一次
   完整执行的指令轨迹，尝试反推原始控制流。
4. 控制流图简化（CFG simplification）：合并/折叠因控制流打散而产生的冗余基本块。

与 `engine/tests/attack/` 目录下已有的白盒测试（`constants_attack.py`、
`interp_attack.py` 等，这些本来就是针对内部实现细节写的）明确区分，不要合并。

运行方式（在 `engine/` 下）：

    python3 tests/attack_blackbox/constant_folding.py
    python3 tests/attack_blackbox/dead_code_elimination.py
    python3 tests/attack_blackbox/trace_devirtualization.py
    python3 tests/attack_blackbox/cfg_simplification.py

每个脚本输出「成功 / 部分成功 / 失败」三档结论之一，并打印支撑该结论的原始数字。
这些脚本是**分析性报告**，不接入 `run_tests.sh` 的 PASS/FAIL 计数。
