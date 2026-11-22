-- Static semantic reconstruction of the business payload only.
-- This is NOT the original source text. Identifier names and expression history
-- cannot be recovered uniquely from the supplied representation.
-- The obfuscator's environment checks, anti-hook traps, and loaders are omitted.
-- Neither the uploaded Lua nor its decoded code was executed during analysis.
--
-- Source facts used (see disassembly.md / prototype.json):
--   root prototype: nparams=0, upsrc={}, no child prototypes
--   S = {8, 9, 4}; 6 instructions; constant pool has C[2]=12, C[3]="print"
--   PC1 GETGLOBAL V[9]=ENV["print"]; PC2 LOADK V[4]=12;
--   PC3 CALL V[9](V[4]); PC4 MOVE V[8]=V[4]; PC5 RETURN

print(12)
