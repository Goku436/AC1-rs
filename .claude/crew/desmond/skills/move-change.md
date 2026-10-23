# A movement change, start to finish

1. **What does AC1 do?** In order of trust: a live comparison or `where`/trace from the real game (Lucy), the exe
   (Malik: address in PARKOUR.md), the move graph, then video. Write down which one you have.
2. **Which clip, which block?** `cargo run --release -p forge --bin actions -- "<game>" <block>` for the block that
   holds the move (`HumanClimb*`, `HumanGround*`, ...). The graph's next steps: `cargo run --release -p forge
   --example graph_next -- ...`. A clip's duration, root motion and yaw: `--bin animinfo -- "<game>" <clip name>`.
3. **Where in ours?** PARKOUR.md's section for the context (0 data, 3 ground, 4 climb, 5 air, 6 jump targets, 7 ledge,
   8 beams, 9 wall run/ladders/hay/bars, 10 IK) says which file and system. Read "How far ours follows AC1's tables"
   and "Open questions" before deciding something is new.
4. **Change it** with the decision from AC1's table or action id, not a tuned constant. If a constant is unavoidable,
   name it, comment where the number came from.
5. **Test**: the `scenarios` skill, tags from the files touched. Add a scenario when the bug wasn't caught by one (or
   leave Lucy an inbox line: "needs a pad scenario: <route, start, what to check>").
6. **Write it down**: PARKOUR.md (what AC1 does, the address, what ours does now), and if it closes one of the open
   questions, CLAUDE.md's open work line.
