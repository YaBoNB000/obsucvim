#!/usr/bin/env bash
# 一键重建工具链（沙箱重置后运行）。
#   组件 = Rust 1.88(@rustbin npm) + Lua 5.1.5(GitHub 镜像) +
#          Luau 0.735(g++ 直编 + 仓库内自写 main，复刻官方 CLI 语义)
# 默认装到仓库内 .tools/（gitignored）；可用 OBSUCVIM_TOOLS=<dir> 覆盖。
# 网络前提：npm registry + codeload.github.com 可达；不需要 crates.io / rustup。
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
T="${OBSUCVIM_TOOLS:-$ROOT/.tools}"
mkdir -p "$T/bin" "$T/lib"
cd /tmp

# 1) Rust 1.88（npm @rustbin；rustc / std / cargo 分开解包再合并）
if [ ! -x "$T/bin/rustc" ]; then
	for p in rustc cargo rust-std; do
		npm pack "@rustbin/$p-1.88.0-x86_64-unknown-linux-gnu" >/dev/null
		rm -rf "x_$p" && mkdir -p "x_$p"
		tar xzf rustbin-$p-*.tgz -C "x_$p"
	done
	( cd x_rustc/package && ./install.sh --prefix="$T" --disable-ldconfig >/dev/null )
	cp -a x_rust-std/package/rust-std-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu \
		"$T/lib/rustlib/" 2>/dev/null || {
		mkdir -p "$T/lib/rustlib"
		cp -a x_rust-std/package/rust-std-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu \
			"$T/lib/rustlib/"
	}
	cp x_cargo/package/cargo/bin/cargo "$T/bin/cargo"
	chmod +x "$T/bin/cargo"
	echo "[ok] rust 1.88"
fi

# 2) Lua 5.1.5（lua.org 不可达 → GitHub 镜像；无 readline → make generic）
if [ ! -x "$T/bin/lua51" ]; then
	curl -sL -o l.tar.gz https://codeload.github.com/zgpxgame/lua-5.1.5/tar.gz/refs/heads/master
	rm -rf lua-5.1.5-master && tar xzf l.tar.gz
	make -C lua-5.1.5-master/src generic MYLDFLAGS="-ldl -lm" -j4 >/dev/null
	cp lua-5.1.5-master/src/lua "$T/bin/lua51"
	cp lua-5.1.5-master/src/luac "$T/bin/luac51"
	echo "[ok] lua 5.1.5"
fi

# 3) Luau 0.735（无 cmake → g++ 直编 7 库 + 仓库内自写 main）
if [ ! -x "$T/bin/luau" ]; then
	curl -sL -o luau.tar.gz https://codeload.github.com/luau-lang/luau/tar.gz/refs/tags/0.735
	rm -rf luau-0.735 && tar xzf luau.tar.gz
	cd luau-0.735
	cp "$ROOT"/engine/tools/luau-cli-mains/main_luau*.cpp .
	SRC=$(ls VM/src/*.cpp Common/src/*.cpp Ast/src/*.cpp Bytecode/src/*.cpp \
		Compiler/src/*.cpp Require/src/*.cpp Config/src/*.cpp)
	g++ -O2 -std=c++17 -DLUA_USE_LONGJMP=1 '-DLUA_API=extern "C"' \
		-I VM/include -I Common/include -I Ast/include -I Bytecode/include \
		-I Compiler/include -I Require/include -I Config/include \
		main_luau.cpp $SRC -o "$T/bin/luau" -pthread
	SRC=$(ls VM/src/*.cpp Common/src/*.cpp Ast/src/*.cpp Bytecode/src/*.cpp \
		Compiler/src/*.cpp)
	g++ -O2 -std=c++17 -DLUA_USE_LONGJMP=1 '-DLUA_API=extern "C"' \
		-I VM/include -I Common/include -I Ast/include -I Bytecode/include \
		-I Compiler/include \
		main_luau_compile.cpp $SRC -o "$T/bin/luau-compile" -pthread
	cd /tmp
	echo "[ok] luau 0.735"
fi

"$T/bin/rustc" --version
"$T/bin/lua51" -v
echo "toolchain ready: $T/bin"
