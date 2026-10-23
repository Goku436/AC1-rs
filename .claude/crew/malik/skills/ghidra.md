# Ghidra on the exe

Headless Ghidra MCP on a **copy** of the project: `F:/Claude/re/mcp/ac1.gpr`, program `/AssassinsCreed_Dx9.exe`
(~107k functions, analysis saved). The original ghidra-cli project stays locked by an old bridge: don't kill what you
didn't start; work on the copy.

## Start / check
- Is it up? `curl -s localhost:8099/` (any answer). Port **8099**; 8089 is the user's AMP panel.
- Start (background): `bash F:/Claude/re/mcp/start_server.sh`, then
  `curl -s -X POST localhost:8099/open_program -d '{"path":"/AssassinsCreed_Dx9.exe"}'` if it didn't open it.

## Calls that matter
`force_decompile` (by address), `get_xrefs_to`, `search_strings`, `search_byte_patterns`, `read_memory`,
`run_script_inline` (full Java GhidraScript source), `save_program` (a write: claim "Ghidra MCP :8099 writes" in
COWORK.md first).

## Gotchas
- Git Bash mangles args starting with `/` (`/AssassinsCreed_Dx9.exe`): put them in a JSON body, or `MSYS_NO_PATHCONV=1`.
- Script args split on spaces and the user folder has one: keep script files and outputs under `F:/Claude/re/work`.
- The subsystem map (classes, functions, who calls whom) is in docs/SUBSYSTEMS.md and `F:/Claude/re/work/subsys`:
  look there before a fresh xref crawl.
- Class ids are CRC32 of class names; the exe's reflection descriptors/deserializers give field layouts.

## Out
Notes in your own words with addresses (NOTES.md / PARKOUR.md / SUBSYSTEMS.md), never pasted decompiler output.
Decompiler dumps stay in `F:/Claude/re/work` or the scratchpad.
