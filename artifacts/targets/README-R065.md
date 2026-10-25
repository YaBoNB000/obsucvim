# R065 minimal Luau sample

## Source

```lua
local a = "bysuda"
```

Exact source file: `R065_src.lua` (19 bytes including the trailing newline).

## Obfuscated output

- Artifact: `R065_target.luau.lua`
- Profile: `--preset v15 --dialect luau --seed 65065`
- Size: 168,788 bytes
- SHA-256: `be049ee7abb482b1264ab945504332015a8188a3e6a134aeded2cfbd83220bec`
- `luau-compile` passes; source and target both exit successfully with empty stdout.
- The literal `bysuda` does not occur in the generated artifact.

The source is intentionally tiny and has no observable output; this is a pipeline/string-constant canary, not a representative business-logic or anti-trace benchmark. In a disposable white-box instrumented build, the R065 v15 program decoded five micro records for this source. That trace counter was not added to the artifact. The broader R064 trace and hook measurements remain in `docs/r064-deep-self-audit-r2.md`.
