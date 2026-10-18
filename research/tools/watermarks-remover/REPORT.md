# watermarks-remover — research report

> Subject: **guillaumemeyer/watermarks-remover**, shallow clone at commit `980c0b4` (2026-10-17 10:58 +0530, "fix(container): value-scan Markdown frontmatter values that span lines… (#402)"), MIT.
> Clone: `~/.hermes/profiles/datacrate360/cache/scratch/agents/watermarks-remover` (299 files outside `.git`). Citations are repo-relative `path:line` at that commit.
> Local copy compared: `~/Technology/5_Projects/4_aiEngineering_Projects/ai-watermark-remover` (301 files, not a git repo, all mtimes 2026-10-15 13:16).
> Tags: **[Verified]** read in source · **[Docs]** README/docs or external page only · **[Inference]** analyst reasoning.
> Not done: nothing installed, built or run. Test counts are lexical (`def test_`). Legal points are a research summary, **not legal advice**.

## Executive summary

- A stdlib-Python HTTP service + CLI scripts + two agent skills + a Claude Code plugin that **strips AI provenance signals** from text, images, documents, audio and video. Release line v0.7.0; ~27k lines in `service/scripts`, `stealer/`, skills.
- Three removal layers: **Layer A** deterministic invisible-Unicode/space-homoglyph scrub; **Layer B** LLM/masked-LM **rewrite** to defeat statistical (token-sampling) text watermarks; **file layer** byte-level stripping of C2PA / EXIF / XMP / docProps across ~20 formats. Optional heavy backends regenerate pixels (CtrlRegen, DiffusionPurification) and degrade audio/video to kill in-signal watermarks.
- Detection exists but is a by-product of removal: `/inspect`, `/detect`, a stylometry "AI-text" scorer, MarkLLM/keyed-Gumbel same-key detectors, a SynthID pixel scorer. Reports separate "definitive / deterministic / best-effort" evidence classes — the best idea in the repo.
- The "**content you own**" guardrail is **text only**: README, `ethics.md`, and one skill line. No code checks ownership, consent or disclosure duties; the skill tells the agent to warn about fraud and *still clean* (`skills/remove-ai-marks/SKILL.md:136`). The skill's own trigger text says "anti-detect clean AI output" (`SKILL.md:9`, `:15`).
- It also ships a **black-box watermark-stealing module** (`stealer/`) and a "humanize" rewrite tuned against AI-text detectors (Pangram etc. in benchmarks). It is an evasion toolkit with hygiene features, not a hygiene tool with research features.
- Since **2 Aug 2026** EU AI Act Art. 50 and California's AI Transparency Act (SB 942 as amended by AB 853) both run on machine-readable marking + detection. A tool whose default `/clean` rewrites text to defeat those marks works directly against them.
- **Local copy:** an **older upstream snapshot plus local edits**. It lacks about 20 upstream commits (including a path-traversal hardening fix). Local additions: `ai-watermark/` Docker build that publishes `0.0.0.0:8765` with an optional, empty-by-default API key; a `/docs` HTML page in `server.py`; `.docker/` notes for Docker's "Gordon" assistant; and **`docs/SBU/python_week1_assignment.md`, a university coursework file** inside the tree.
- **Recommendation for omni-agent: do not include removal.** Port the *detection/provenance* half: Layer A invisible-Unicode **detection** (as a lint), C2PA/metadata **reading**, and the evidence-class report. Build forward: **sign and label** AI-produced deliverables (C2PA manifests, disclosure notes), and never strip them.

```
            ┌─────────────── service/scripts/server.py (stdlib ThreadingHTTPServer, :8765) ───────────────┐
 base64 ──► │ /inspect  /detect  /clean  /watermark (+ /batch)   auth: optional Bearer key  :1418-1425      │
            └──────┬──────────────────────────────────────────────────────────────────────────────────────┘
                   │ classify_bytes(): extension → magic bytes → "unknown" (refuse)   format_dispatch.py:129
     ┌─────────────┼───────────────────┬──────────────────────────┬──────────────────────────┐
     ▼ text        ▼ image             ▼ container                ▼ av
 Layer A           strip chunks/APPn/  docProps, customXml,       ISOBMFF boxes, ID3, RIFF
 text_unicode.py   boxes + exiftool    XMP, HTML meta, MD front-  (+ optional destructive
   ▼               (+ optional         matter, PDF via exiftool→  tempo/pitch/EQ re-encode,
 Layer B rewrite   CtrlRegen/Diffusion qpdf→ghostscript           per-frame purification)
 rewrite_text.py   pixel regeneration) container_meta.py          av_meta.py / clean_audio.py
     └────────────────────► report: actions + residual scan + evidence classes ◄──────────────────┘
```

## T1. Identity

In plain words: a small open-source project by one main author that removes AI "made-by" marks from files and text. It is MIT-licensed Python with no required third-party packages, and plugs into coding agents as a skill and a hook.

- Purpose (README): "strip **multi-vendor AI provenance marks** from text and files — for privacy and hygiene on content **you own**" — `README.md:9` [Docs]
- License MIT — `LICENSE`, `README.md:1475-1477` [Verified]
- Version v0.7.0 in plugin manifest; README changelog headline names "watermark-stealing module, audio/video watermark removal" — `.claude-plugin/plugin.json:5`, `README.md:1226-1228` [Verified]
- Stack: stdlib Python 3.10+ core; `pytest`/`ruff` dev deps; optional heavy stacks torch/diffusers/transformers (CtrlRegen, MarkLLM, MarkDiffusion, SynthID scorer) — `service/scripts/requirements-*.txt` [Verified]
- Size: largest files `container_meta.py` 4,749 lines, `bench_synthid_text.py` 3,828, `image_meta.py` 2,359, `server.py` 1,712, `rewrite_text.py` 1,460 (`wc -l`) [Verified]
- Upsell: none of its own. An "Ecosystem" section lists a third-party SaaS ("Simple Unmark", TEE "confidential mode") and GUIs built on it — `README.md:1172-1198` [Docs]
- Maturity: CI, CodeQL, Dependabot, CodeRabbit config, pinned Docker digests, 1,122 lexical tests — `.github/workflows/ci.yml`, `.github/workflows/codeql.yml`, `tests/` [Verified]

## T2. Core mechanism (step by step)

In plain words: the service guesses what kind of file it got, then runs one of four cleaners. Text gets invisible characters deleted and is then reworded by a model. Images and documents get their metadata blocks cut out at the byte level. Afterwards it re-scans the output and reports what was removed and what may still be left.

**Step 1 — routing.**
- `classify_bytes(data, suffix)`: known extension wins (`IMAGE_EXTS`, `CONTAINER_EXTS`, `TEXT_EXTS`, `AV_EXTS`), else magic bytes for images/AV, else ZIP central-directory sniff for OOXML/ODT/EPUB, else `"unknown"`. Nothing falls back to "text" anymore — `service/scripts/format_dispatch.py:129-157` [Verified]
- On-disk variant reads only a 4,096-byte header unless it starts `PK\x03\x04` — `format_dispatch.py:24`, `:160-189` [Verified]
- `/clean` raises on `unknown` (HTTP 400); text that `looks_binary` is refused — `service/scripts/server.py:1226-1242` [Verified]

**Step 2 — text Layer A (deterministic Unicode scrub).**
- `STRIP_CODEPOINTS`: soft hyphen, CGJ, ZWSP/ZWNJ/ZWJ, LRM/RLM, bidi embeddings/overrides/isolates (U+202A–E, U+2066–9), word joiner and invisible operators, BOM, VS1–16, Hangul fillers, interlinear annotations — `service/scripts/text_unicode.py:11-74` [Verified]
- Unicode tag characters U+E0020–E007F (the "ASCII smuggling" carrier) and reserved default-ignorables / VS17–256 — `text_unicode.py:170-187`, `:344` [Verified]
- `SPACE_HOMOGLYPHS`: 16 exotic spaces (NBSP, en/em/thin/hair, NNBSP, ideographic…) → U+0020 by default — `text_unicode.py:76-93` [Verified]
- `LATIN_CONFUSABLES` (Cyrillic А/В/Е/…, full-width A–z → ASCII) **only with `--aggressive-homoglyphs`** — `text_unicode.py:96-168`, `:513-514` [Verified]
- `_decide()` is context-aware: keeps ZWJ inside emoji sequences, VS after CJK ideographs, valid flag tag runs, script joiners between same-script letters, Mongolian/Khmer/Hangul legit uses; strips any other `Cf` char — `text_unicode.py:454-527` [Verified]
- Optional NFKC normalisation, with change counts via `SequenceMatcher` — `text_unicode.py:688-701` [Verified]

**Step 3 — text Layer B (rewrite against statistical watermarks), on by default in `/clean`.**
- `/clean` **requires** Layer B for text: default strategy from `config/clean_strategy.json` = `"paraphrase@0.8,mlm@0.2"`; returns 400 if no backend — `server.py:1255-1265`, `config/clean_strategy.json:2` [Verified]
- Tactics: `paraphrase`, `humanize`, `code`, `backtranslate`, `structural` (outline→regenerate), `chunk`, `mlm` (mask content words, infill with `roberta-large`) — `service/scripts/rewrite_text.py:301-330`, `:443-525` [Verified]
- Backends: `print-prompt` (default CLI), `ollama`, `openai-compatible`; non-loopback endpoints refused unless `WATERMARKS_REWRITE_ALLOW_REMOTE=1` — `rewrite_text.py:1-16`, `:212` [Verified]
- Loop: generate N candidates, evaluate, stop when a detector says "not watermarked". Evaluator priority: keyed-Gumbel same-key replay > MarkLLM same-config detector > bigram-Jaccard divergence (pick the most-changed candidate, penalise >2× or <0.5× length) — `rewrite_text.py:158-186`, `:260-279`, `:730` [Verified]
- `humanize` also runs a deterministic pass: straight quotes, no em/en dashes, filler collapses, `utilize→use` — `rewrite_text.py:33-39`, `service/scripts/humanize_pass.py` [Verified]

**Step 4 — images (metadata strip, optional pixel regeneration).**
- PNG: drop `eXIf`, `caBX`, any `c2**` chunk, all `tEXt/zTXt/iTXt` (or only AI ones), any non-critical chunk containing C2PA markers; decompression capped at 1 MiB; truncated tails kept — `service/scripts/image_meta.py:1940-2001`, `:183` [Verified]
- JPEG APPn/COM, WebP RIFF chunks, AVIF/HEIC ISOBMFF `jumb`/XMP `uuid`/C2PA `uuid` boxes, BMP trailers, GIF comment/XMP extensions, TIFF IFD tags — `image_meta.py:2004`, `:2091`, `:2129`, `:817`, `:959`, `:1323` [Verified]
- Then `exiftool -all= -overwrite_original` if installed — `image_meta.py:2260-2280` [Verified]
- Optional `remove_pixel="ctrlregen"` (controllable regeneration, intensity 0.25) or `"diffusion"` (MarkDiffusion DiffusionPurification, 0.3), run as subprocesses with RLIMITs — `image_meta.py:2282-2323`, `:46-62` [Verified]
- Residual scan: re-inspect output, report `still_has_c2pa`, `still_has_ai_metadata`, `post_findings`, `synthid_before/after` — `image_meta.py:2325-2359` [Verified]
- Detection hints are byte needles (`c2pa`, `digitalSourceType`, `trainedAlgorithmicMedia`, "Claude", "OpenAI", "SynthID"…); vendor product names are matched only in generator fields to avoid "Claude Monet" false positives — `image_meta.py:89-176` [Verified]

**Step 5 — documents and AV.**
- OOXML: scrub `docProps/*`, drop `customXml/` and web-extension parts, fix content types/relationships; refuse while an Office lock file `~$name` exists — `service/scripts/container_meta.py:2022-2060`, `:2065-2075`, `:2852` [Verified]
- PDF: exiftool, then `qpdf --linearize` (exiftool edits are incremental and recoverable), then Ghostscript `pdfwrite` for metadata inside embedded images (lossless rung first, re-encode only on evidence) — `container_meta.py:4260`, `README.md:1054-1122` [Verified]/[Docs]
- HTML/Markdown: AI `<meta>`, JSON-LD, `data-ai*`, AI-naming comments, YAML frontmatter AI keys — `README.md:1044-1045` [Docs]
- Audio: deliberately destructive ffmpeg chain (tempo + pitch + EQ + low-bitrate re-encode) aimed at AudioSeal/WavMark/silentcipher — `service/scripts/clean_audio.py:1-12` [Verified]
- Video: per-frame pixel purification so a TrustMark temporal vote collapses — `service/scripts/clean_video.py:1-15` [Verified]

## T3. Architecture and process model

In plain words: one Python HTTP server does the work. Agents talk to it over HTTP from a skill. Heavy ML jobs run as separate Docker sidecars or subprocesses, and they are optional.

- `ThreadingHTTPServer` on `127.0.0.1:8765` by default; warns when bound elsewhere or keyless — `server.py:1669-1706` [Verified]
- Each request works in a `tempfile.TemporaryDirectory`; names are reduced to a basename and `_tmp_path` refuses escapes — `server.py:868-895`, `:1234-1236` [Verified]
- Compose profiles: `wr-core` (default), `harness` (`wr-markllm`, `wr-synthid-text`, `wr-markdiffusion`), `heavy` (`wr-ctrlregen`, `wr-synthid`, `wr-synthid-score`); core published on `127.0.0.1:8765` only — `compose.yaml:20-26`, `:44-163` [Verified]
- The container image itself binds `0.0.0.0` inside the container — `service/Dockerfile:76` [Verified]
- External tools by subprocess: exiftool, qpdf, ghostscript, ffmpeg, c2patool — `image_meta.py:1452`, `README.md:1030-1052` [Verified]/[Docs]

## T4. Data model

In plain words: there is no database. Files go in and out as base64 JSON, and results are JSON reports.

- Request `{"file": base64, "name", "options"}`; response `{"ok","kind","cleaned","report"}`; batch ≤ `WATERMARKS_MAX_BATCH_FILES` (50) — `README.md:353-367`, `server.py:103` [Verified]
- Size caps: `MAX_INPUT_BYTES` 256 MiB, body ≤ 1.5× that — `service/scripts/common.py:18`, `server.py:98` [Verified]
- Evidence classes in `suspicious`: `provenance` ("definitive"), `layer_a_unicode` ("deterministic"), detector/stylometry (score ≥ 0.65) — `server.py:1070-1130` [Verified]
- OpenAPI 3.0.3 generated at runtime (`/openapi.json`) — `server.py:458-740` [Verified]
- Audit output can be SARIF (CI) — `service/scripts/audit_website.py`, `README.md:1268-1270` [Verified]/[Docs]
- Strategy config JSON — `config/clean_strategy.json` [Verified]

## T5. Interfaces (exact names)

In plain words: you can call it as an HTTP API, as command-line scripts, as an agent skill, as an automatic hook after every file write, or as a git pre-commit hook.

- HTTP: `GET /health`, `/capabilities`, `/openapi.json`; `POST /inspect`, `/detect`, `/clean`, `/watermark` and `/…/batch` variants — `server.py:1451-1510` [Verified]
- CLIs: `inspect_file.py`, `clean_file.py`, `inspect_text.py`, `clean_text.py`, `rewrite_text.py`, `inspect_image.py`, `clean_image.py`, `clean_ctrlregen.py`, `audit_dir.py`, `audit_website.py`, `score_stylometry.py`, `detect_gumbel.py` — `service/scripts/` [Verified]
- Skills: `remove-ai-marks` (service-backed), `clean-user-facing-text` (self-contained) — `skills/*/SKILL.md` [Verified]
- Claude Code plugin + single-plugin marketplace — `.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json` [Verified]
- `PostToolUse` hook on `Write|Edit|MultiEdit|NotebookEdit` → `hooks/run_hook.js` → `hook_written_file.py`, modes `check` (default, exit 2 to the model) / `clean` (in place) — `hooks/hooks.json`, `service/scripts/hook_written_file.py:55-145` [Verified]
- pre-commit hooks — `.pre-commit-hooks.yaml`, `service/scripts/check_staged.py`, `clean_staged.py` [Verified]
- Installer targets: claude-code, claude-project, codex, codex-project, cowork zip, cursor, grok — `install_skill.py`, `README.md:42-52` [Verified]/[Docs]
- Cursor rule — `integrations/cursor/clean-user-facing-text.mdc` [Verified]

## T6. How it plugs into agents

In plain words: it is built so the agent removes the marks from its *own* output automatically. The hook is the reliable part. The skill only works if the model decides to call it.

- README explains the hook is "the deterministic half" because "the model is the thing producing the marks" — `README.md:95-103` [Docs]
- `check` mode feeds findings back to the model so it "can offer to clean them"; the hook prints the exact `clean_file.py --in-place` command — `hook_written_file.py:113-140` [Verified]
- Hosts: Claude Code, Codex, Cursor, Grok, Claude.ai/Cowork uploads — `README.md:42-52`, `:229-240` [Docs]
- [Inference] In omni-agent this design would mean the platform hides its own AI provenance from every artifact it writes. That runs against a provider/deployer's marking and labelling duties (T8).

## T7. LLM usage

In plain words: the Unicode and metadata cleaners need no model and run offline. Rewording text needs a model (local Ollama or any OpenAI-style API), and the pixel cleaners need GPU diffusion models.

- Layer B backends `ollama` / `openai-compatible`; API key from env only; remote blocked by default — `rewrite_text.py:1-16`, `:212`, `:597-668` [Verified]
- `mlm` tactic loads `roberta-large` locally (CUDA if available) — `rewrite_text.py:433-525` [Verified]
- README advises a **non-origin** model so the rewrite does not re-stamp the watermark — `README.md:1005` [Docs]
- Cost drivers: one or more full rewrites per text (candidates × loops); diffusion regeneration per image or per video frame — `rewrite_text.py:17-22`, `clean_video.py:1-15` [Verified]
- `stealer/` sends a 30k-prompt corpus (C4 RealNewsLike by default) to a model to estimate green-token boosts — `stealer/README.md:1-15`, `stealer/steal.py:1-9` [Verified]

## T8. Security, privacy, safety, legal and ethical

In plain words: the code is careful about ordinary security bugs. The real problem is what it is for. It is built to remove the marks that new laws require AI providers to add, and the only "you must own it" check is a sentence in the docs. The local copy also opens the service to the network with no password by default.

**Engineering security (good)**
- Path-escape guard on temp writes (CodeQL-driven) — `server.py:868-895` [Verified]
- ReDoS fixes, SSRF fix (redirects blocked) in the SynthID scorer client, zTXt bomb cap, SVG DOCTYPE/ENTITY stripping — `README.md:1271-1272`, `image_meta.py:183` [Docs]/[Verified]
- Rewrite: remote LLM endpoints opt-in; keys env-only — `rewrite_text.py:212` [Verified]
- Heavy backends run with RLIMIT_AS 32 GiB / RLIMIT_FSIZE 2 GiB — `image_meta.py:46-62` [Verified]
- Weak spots: API key optional and compared with `==` (not constant-time); no rate limit; 256 MiB bodies decoded in memory — `server.py:1421-1425`, `common.py:18` [Verified]

**The "content you own" guardrail — not enforced**
- Stated in README "Responsible use" and `ethics.md` (appropriate: privacy, hygiene, research; not appropriate: academic fraud, circumventing transparency rules, "human-written" claims) — `README.md:1166-1170`, `skills/remove-ai-marks/references/ethics.md:1-32` [Verified]
- Skill instruction: if the user "clearly wants academic fraud or illegal non-disclosure, warn … and **still only perform technical cleaning they own**" — so it warns and proceeds — `skills/remove-ai-marks/SKILL.md:136` [Verified]
- Skill trigger text includes "**anti-detect clean AI output**" and calls itself "Multi-vendor **anti-detection** hygiene" — `SKILL.md:9`, `:15` [Verified]
- No code path asks about ownership, consent, purpose or a required disclosure. `office_owner_file` is the Office **lock file** check (`~$name`), not an ownership check — `container_meta.py:2065-2075` [Verified]
- Evasion tooling beyond hygiene: watermark **stealing** (`stealer/`), destructive audio removal, per-frame video purification, a benchmark that tunes rewrites against AI-text detectors including the Pangram API — `stealer/README.md:1-15`, `clean_audio.py:1-12`, `README.md:1238-1243` [Verified]/[Docs]
- To its credit, reports never claim "undetectable", and the `clean-user-facing-text` skill says to keep required disclosures — `README.md:1003`, `skills/clean-user-facing-text/SKILL.md:8`, `references/responsible-use.md:1-7` [Verified]

**Legal landscape (research summary, not legal advice)**
- **EU AI Act Art. 50** (Reg. 2024/1689), applicable from **2 Aug 2026**. Art. 50(2): *providers* of generative systems must mark outputs (audio, image, video, text) machine-readably and detectably. Art. 50(4): *deployers* must disclose deepfakes and AI-generated text on matters of public interest, unless it had human editorial review. Fines up to €15M / 3%. A narrow runway to 2 Dec 2026 applies to 50(2) for systems already on the market — https://digital-strategy.ec.europa.eu/en/policies/code-practice-ai-generated-content , https://artificialintelligenceact.eu/transparency-rules-article-50/ [Docs]
- The repo itself cites Anthropic's marking of models launched on/after 2026-08-02 under the Art. 50(2) Code of Practice — `skills/remove-ai-marks/references/how-claude-marks.md:3-12` [Verified]/[Docs]
- **California AI Transparency Act (SB 942, amended by AB 853)**, operative **2 Aug 2026**. Covered providers must embed latent disclosures, offer manifest disclosures, and provide a free public detection tool. Further platform and capture-device duties phase in through 2028 — https://www.morganlewis.com/pubs/2026/08/new-california-ai-disclosure-rules-become-operative , https://leginfo.legislature.ca.gov/faces/billTextClient.xhtml?bill_id=202320240SB942 [Docs]
- [Inference] These laws put duties on providers, deployers and platforms more than on end users. But a platform that ships an automatic "strip marks from everything we write" step makes its own marking and labelling ineffective, and it helps users in deployer roles skip disclosure. That is the exposure that matters for omni-agent.
- **C2PA**: hard-bound manifests are cryptographically bound to the asset; "soft binding" (an in-content watermark that re-links a manifest) is out of scope here, so stripping does not erase all provenance — `README.md:1124`, https://c2pa.org/ [Docs]
- **Platform ToS / provider policies**: generative-AI providers and publishing platforms generally forbid removing or misrepresenting provenance and disclosures. Check each provider's current usage policy before any use. [Inference — terms not re-read this session]
- **Deception / fraud**: removing marks to pass content off as human-written is academic misconduct, consumer deception or fraud, depending on context. The repo's own ethics file lists these as inappropriate uses — `ethics.md` [Verified]

## T9. Quality: tests, benchmarks, CI

In plain words: the tests are thorough and CI is serious. The benchmarks measure how well it evades detectors, not how faithful it is to the original.

- 1,122 `def test_` across ~80 test files (formats, security hardening, SVG, ReDoS, server, hooks, plugin manifest) — `tests/` [Verified]
- CI: pytest on 3.12, ruff check + format, optional-backend requirement resolution, CodeQL — `.github/workflows/ci.yml:21-97`, `codeql.yml` [Verified]
- Benchmarks: SynthID-text removal recipe search, DeepSeek smoke test, an English corpus (8 + 31 docs) and a Polish corpus (50 + extras) — `benchmarks/`, `service/scripts/bench_synthid_text.py`, `docs/synthid-text-benchmark.md` [Verified]
- README's honest claim: no tool can certify that a vendor detector fails — `README.md:1003` [Docs]
- Not verified here: any removal-rate number (nothing was run).

## T10. Operations

In plain words: the basic tool needs only Python. Full metadata cleaning wants a few command-line tools, and the pixel and rewrite features need large ML stacks and often a GPU.

- `make serve` or `python3 service/scripts/server.py --host 127.0.0.1 --port 8765` — `README.md:266-290` [Docs]
- System tools: exiftool, qpdf, ghostscript, ffmpeg, c2patool (pinned with sha256 in Docker) — `service/Dockerfile` [Verified]
- Optional: torch 2.14, transformers 5.15, diffusers 0.27.2, markdiffusion 1.0.2, sentence-transformers — `service/scripts/requirements-*.txt` [Verified]
- Setup scripts clone external repos: `mertizci/noai-watermark` (CtrlRegen), `THU-BPM/MarkLLM` (Apache-2.0), `THU-BPM/MarkDiffusion`, `aloshdenny/reverse-SynthID` — `service/scripts/setup_*.sh:6-83` [Verified]

## T11. What is new vs. marketing

In plain words: the careful format coverage and the honest reports are real. The "privacy-first, your own content" pitch does not match a repo that ships watermark stealing and detector-tuned rewriting.

- Real: byte-level strippers for ~20 formats with residual re-scan; PDF exiftool→qpdf→ghostscript chain that fixes the "incremental exiftool leak" — `README.md:1054-1122`, `container_meta.py:4260` [Verified]
- Real: context-aware invisible-Unicode handling that keeps emoji ZWJ, flags and CJK variation selectors — `text_unicode.py:454-527` [Verified]
- Real: evidence-class report (definitive / deterministic / best-effort) — `server.py:1070-1130` [Verified]
- "Privacy-first … content you own": no enforcement; default `/clean` always rewrites text — `server.py:1255-1265`, `SKILL.md:136` [Verified]
- "Covers SynthID / Claude marks": statistical text removal is best-effort rewriting, and the vendor detectors are unavailable (Claude) or retired (SynthID-text, per repo) — `README.md:981-991`, `rewrite_text.py:260-271` [Docs]/[Verified]

## T12. Weaknesses, limits, risks

In plain words: rewording damages the writing, pixel regeneration changes the image, and nothing guarantees the marks are really gone. The biggest risk is legal and reputational, not technical.

- Layer B degrades voice and precision; the README itself asks why you would pay for a premium model and then reword it — `README.md:1009-1027` [Docs]
- Out of scope: C2PA soft binding, secret-key detectors, training backdoors; audio/video only via destructive chains — `README.md:1124`, `SKILL.md:328`, `:372` [Verified]
- Layer A strips legit characters when context rules miss (e.g. RTL bidi controls unless preserved, soft hyphens) — `text_unicode.py:11-74` [Inference]
- Hook `clean` mode silently rewrites agent-written files in place — `hook_written_file.py:143-210` [Verified]
- Legal and reputational risk for anyone who ships it as a feature (T8) [Inference]

## T13. How it fits into omni-agent

In plain words: **removal should not be part of omni-agent.** Keep the detection ideas. Use them to *verify and label* AI-made content in deliverables, plus a harmless invisible-character lint for code and docs.

- **Lifecycle role:** cross-cutting *provenance*. Verify (lint deliverables), Release (attach Content Credentials / AI-disclosure labels), Learn (record which artifacts were AI-assisted in the knowledge graph). [Inference]
- **Skip:** `/clean` Layer B, pixel/audio/video removal, `stealer/`, the `PostToolUse` clean hook, the `remove-ai-marks` skill. Reason: they defeat legally mandated marking (EU AI Act Art. 50, CA SB 942/AB 853). [Inference]
- **Port the idea (MIT, compatible):**
  1. Invisible-Unicode **detector**, report-only: tag chars, bidi overrides, ZWSP in code and specs. This is a real supply-chain / "Trojan Source" and prompt-injection risk — `text_unicode.py:11-74`, `:578` [Verified]
  2. Read-only C2PA / AI-metadata **inspector**, with `c2patool` for signature validation — `image_meta.py:1861`, `container_meta.py:4480` [Verified]
  3. Evidence-class report shape — `server.py:1070-1130` [Verified]
- **Build forward instead:** sign omni-agent outputs with C2PA manifests (`digitalSourceType=trainedAlgorithmicMedia`), add a disclosure footer/frontmatter for AI-drafted docs, and keep an audit log. [Inference]
- **Effort:** detector port ~1–2 days. C2PA signing via `c2pa` SDK ~1 week. [Inference]
- **Interface contract (proposed):**

```python
@dataclass
class ProvenanceFinding:
    kind: Literal["c2pa", "ai_metadata", "invisible_unicode", "detector"]
    strength: Literal["definitive", "deterministic", "best_effort"]
    location: str            # path[:line[:col]] or container part
    detail: str
def inspect_provenance(path: Path) -> list[ProvenanceFinding]   # read-only, never mutates
def label_artifact(path: Path, *, ai_assisted: bool, model: str, run_id: str) -> Path  # adds, never strips
# Policy: no strip/clean API exists; a Unicode "fix" is offered only for code/config files
# and only for Trojan-Source classes (bidi overrides, tag chars), with a human approval gate.
```

- **About the local copy** (`ai-watermark-remover`) [Verified, `diff -rq` excluding `.git`, `__pycache__`, `.DS_Store`]:
  - **Older upstream snapshot.** 44 files differ, mostly in the upstream direction (e.g. `container_meta.py` upstream +1,627 / local +37 lines). It is missing upstream-only files: `bench_deepseek_smoke.py`, `docs/frontmatter-scanning.md`, `docs/deepseek-benchmark.md`, `benchmarks/providers/`, 9 test files (`test_pdf_attachments.py`, `test_latex_meta.py`, `test_frontmatter_*`…). Its manifests say `0.5.0` while its README says v0.7.0.
  - Missing an upstream security fix: upstream rejects path separators in the MarkLLM `config` option; local passes it through — `service/scripts/text_watermark.py:132-135` (upstream) vs local `:132`.
  - **Local changes:** (a) `server.py` adds `GET /docs`, an HTML page titled "Claude Watermark Remover" (+~150 lines); (b) `ai-watermark/Dockerfile` + `docker-compose.yml` publish **`0.0.0.0:8765`** with `WATERMARKS_SERVER_API_KEY` defaulting to empty (upstream publishes `127.0.0.1:8765`, `compose.yaml:26`); (c) `.docker/` memory/runbook/rules for Docker's "Gordon" assistant; (d) **`docs/SBU/python_week1_assignment.md`**, a personal university assignment ("Introduction to Big Data Analytics and IoT").
  - [Inference] Advice: remove the coursework file from this tree, and never run the remover on graded or submitted work (the repo's own `ethics.md` lists academic fraud as inappropriate). Bind the local Docker build to `127.0.0.1` and set an API key. Do not reuse this copy in omni-agent.

## T14. Open questions

In plain words: what needs a lawyer or a closer look before any of this is reused.

- Licenses of the external backends (`noai-watermark`/CtrlRegen, `reverse-SynthID`, MarkDiffusion model weights) are not checked here. [Open]
- Does the final EU Code of Practice on AI-generated content say anything about tools that remove marks? Not read in full this session. [Open]
- Is omni-agent a *provider* or a *deployer* under Art. 50 for content it generates for clients? That decides the marking vs labelling duties. [Open — legal]
- Which generative-AI provider terms (Anthropic, OpenAI, Google) explicitly forbid removing provenance? To be checked against the current policy text. [Open]
- Is the local `0.5.0` manifest from an upstream state or a local edit? The shallow clone has no history to tell. [Open]
