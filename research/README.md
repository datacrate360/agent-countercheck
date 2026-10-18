# omni-agent research

Source-level research behind omni-agent: one platform that runs an Agile workflow end to end
(discover → plan → build → verify → release → operate → learn) by combining the best ideas from 12 open-source projects.

- **[TRACKER.md](TRACKER.md)**: which analyst is studying what, live status, report links
- **[RUBRIC.md](RUBRIC.md)**: the questions every report answers, and the citation rules
- `agents/`: the three full agents (Hermes Agent, OpenClaw, Odysseus)
- `tools/`: the nine building blocks (claude-mem, graphify, MiroFish, OpenUI, Paperclip, Strix, REA, MarkItDown, watermarks-remover)
- `synthesis/`: agent comparison, capability map, the nine agent roles, proposed architecture and build order

omni-agent is a standalone, general-purpose project; the research and plans are not tied to any other product.

## Rules

1. Every fact cites `path:line` at a pinned commit (or a docs URL) and is tagged [Verified], [Docs] or [Inference].
2. Code wins when it disagrees with docs or a README. Marketing claims are checked, not repeated.
3. License is checked before any idea becomes code. AGPL projects are learned from, not copied.
4. Research lives on `DEV`. Promotion to `TEST`/`PROD` follows the usual flow.
