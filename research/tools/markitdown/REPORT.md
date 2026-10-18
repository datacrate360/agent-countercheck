# MarkItDown — research report

> Subject: **microsoft/markitdown**, shallow clone at commit `4cc9fa1` (2026-10-03 20:59 -0700, "Restore JSON regression coverage for late non-ASCII text (#2601)").
> Clone: `~/.hermes/profiles/datacrate360/cache/scratch/agents/markitdown` (195 files incl. `.git`-excluded test fixtures). Citations are repo-relative `path:line` at that commit.
> Tags: **[Verified]** read in source · **[Docs]** README/docs claim only · **[Inference]** analyst reasoning.
> Not done: nothing installed, built or run. Test counts are lexical (`def test_` occurrences), not executed. The clone is shallow, so history/contributor counts are not derivable.

## Executive summary

- MarkItDown is Microsoft's MIT-licensed Python library + CLI that turns files and URLs into Markdown for LLM consumption; v0.1.8, "Development Status :: 4 - Beta".
- Its core is a tiny, clean **registry + guess + try-in-order** dispatcher: build `StreamInfo` guesses from extension, MIME, HTTP headers **and Google Magika** (a local ML file-type model), then try every converter sorted by a float priority; the first one that `accepts()` and returns wins.
- 18 converters are always registered (PDF, DOCX, XLSX, XLS, PPTX, HTML, CSV, JSON/text, EPUB, IPYNB, Outlook MSG, ZIP, RSS, images, audio, Wikipedia, YouTube, Bing SERP) plus 2 opt-in Azure cloud converters (Document Intelligence, Content Understanding).
- Plugins are Python entry points in group `markitdown.plugin`, **off by default**; the first-party `markitdown-ocr` plugin overrides PDF/DOCX/PPTX/XLSX at priority −1.0 and OCRs embedded images with an OpenAI-compatible vision LLM.
- `markitdown-mcp` exposes exactly **one tool**, `convert_to_markdown(uri)`, over STDIO or Streamable HTTP/SSE, with **no authentication**; it will read any file and fetch any URL the process can reach.
- Security model is "it does I/O with your privileges — sanitise inputs yourself": no SSRF guard, no request timeout, no size caps, no ZIP-bomb or nesting limits.
- Output quality is "good enough for LLM context", not layout-faithful: PDFs lose headings, tables come only from heuristics, images become metadata/captions, result carries only `markdown` + `title` (no page/offset provenance).
- **For omni-agent:** adopt as an in-process library behind a hardened `ingest()` wrapper (own fetcher, path allowlist, size/time caps, sandboxed worker, plugins allowlisted). Do not expose the stock MCP server to untrusted callers. Treat `markitdown-ocr` separately because it pulls in PyMuPDF (AGPL-3.0/commercial).

```
 source (str | Path | Response | BinaryIO)
        │ convert()  _markitdown.py:322
        ▼
 ┌──────────────┐  file:/data:/http(s)  ┌──────────────┐
 │ convert_local│◄──────────────────────│ convert_uri  │── requests.get(stream=True) (no timeout)
 └──────┬───────┘                       └──────┬───────┘
        │ StreamInfo base guess (ext, mime, filename, url, charset)
        ▼
 _get_stream_info_guesses: mimetypes ⇄ ext  +  Magika identify_stream  +  charset_normalizer
        │  [compatible → 1 merged guess | incompatible → 2 guesses]  + final empty StreamInfo()
        ▼
 _convert: for guess in guesses: for conv in sorted(priority): accepts()? → convert() → first result wins
        │  (exceptions collected → FileConversionException; none accepted → UnsupportedFormatException)
        ▼
 DocumentConverterResult{markdown, title}  → line rstrip + collapse ≥3 newlines
```

---

## T1 — Identity

In plain words: MarkItDown is a small Microsoft open-source tool that converts documents (PDF, Word, Excel, PowerPoint, web pages, and more) into Markdown text that an AI model can read. It is MIT-licensed Python, still labelled beta, and has no paid tier inside the repo. Two optional paths call Microsoft Azure paid services.

- Package `markitdown`, description "Utility tool for converting various files to Markdown", `requires-python >=3.10,<3.15`, `license = "MIT"`, author Adam Fourney (Microsoft). `packages/markitdown/pyproject.toml:5-15` [Verified]
- Version `0.1.8`. `packages/markitdown/src/markitdown/__about__.py:4` [Verified]; maturity "Development Status :: 4 - Beta". `packages/markitdown/pyproject.toml:17` [Verified]; class docstring "(In preview) An extremely simple text-based document reader". `packages/markitdown/src/markitdown/_markitdown.py:142` [Verified]
- License text "MIT License / Copyright (c) Microsoft Corporation." `LICENSE:1-3` [Verified]
- Monorepo of 4 packages: `markitdown` (core), `markitdown-mcp` v0.0.1a7 (`packages/markitdown-mcp/src/markitdown_mcp/__about__.py:4`), `markitdown-ocr` v0.1.1 (`packages/markitdown-ocr/src/markitdown_ocr/__about__.py:4`), `markitdown-sample-plugin` (RTF demo). [Verified]
- Size: core library ≈ 7.4K Python LOC in `src/` (largest: `_markitdown.py` 847, `_pdf_converter.py` 589, `_cu_converter.py` 570, `_rss_converter.py` 488 lines); 42 test fixture files in `packages/markitdown/tests/test_files`. [Verified via `wc -l`]
- Upsell: none in-repo, but the two highest-fidelity paths are Azure Document Intelligence and Azure Content Understanding (paid cloud). `packages/markitdown/src/markitdown/_markitdown.py:254-296` [Verified]

## T2 — Core mechanism (step by step)

In plain words: you hand MarkItDown a path, URL or byte stream. It first works out what kind of file it probably is, using the file name, the web server's headers and a small AI file-type detector called Magika. Then it walks a priority-ordered list of converters and asks each "can you handle this?"; the first one that says yes and succeeds produces the Markdown.

1. **Entry dispatch on source type.** `convert()` sends a `str` whose scheme is `http/https/file/data` to `convert_uri`, any other `str` or `Path` to `convert_local`, a `requests.Response` to `convert_response`, and a binary stream to `convert_stream`; text streams are rejected. `packages/markitdown/src/markitdown/_markitdown.py:322-366` [Verified]
2. **Base guess.** `StreamInfo` is a frozen dataclass of six optional fields: `mimetype, extension, charset, filename, local_path, url`. `copy_and_update` merges non-None fields. `packages/markitdown/src/markitdown/_stream_info.py:5-32` [Verified]
   - Local files: extension, basename and path from the filesystem. `_markitdown.py:381-385` [Verified]
   - HTTP: MIME and charset from `Content-Type`; filename from `Content-Disposition` (RFC 2231 extended `filename*` preferred); else the extension from the URL path. `_markitdown.py:56-70`, `_markitdown.py:538-578` [Verified]
   - `data:` URIs: MIME and charset parsed from the URI. `_markitdown.py:496-512` [Verified]
3. **Guess enrichment** in `_get_stream_info_guesses`. `_markitdown.py:737-836` [Verified]
   - Uses stdlib `mimetypes` to fill a missing MIME from the extension, or a missing extension from the MIME. `_markitdown.py:748-760` [Verified]
   - Calls `self._magika.identify_stream(file_stream)` (Magika instance created per `MarkItDown()` at `_markitdown.py:168`). `_markitdown.py:765` [Verified]
   - If Magika says text, it runs `charset_normalizer` on a 64 KiB sample. The sample is extended by up to 3 bytes so a split UTF-8 character at the boundary is completed. `_markitdown.py:73-98`, `_markitdown.py:769-776` [Verified]
   - **Compatibility rule.** The Magika guess is incompatible if the caller's MIME differs, the caller's extension is not in Magika's extension list, or the normalised charset differs. `_markitdown.py:783-802` [Verified]
   - If compatible, one merged guess is made (caller values win, Magika fills the gaps). If incompatible, **two guesses** are made in order: the caller's enhanced guess, then Magika's. `_markitdown.py:804-829` [Verified]
4. **Try loop** in `_convert`. `_markitdown.py:602-695` [Verified]
   - Registrations are copied and **stable-sorted by priority** on every call. `_markitdown.py:613` [Verified]
   - Outer loop: each guess plus a final empty `StreamInfo()` as a last-chance pass. Inner loop: each converter. `_markitdown.py:618-619` [Verified]
   - Global options (`llm_client`, `llm_model`, `llm_prompt`, `style_map`, `exiftool_path`) and `_parent_converters` are injected into kwargs. `_markitdown.py:628-645` [Verified]
   - `accepts()` is called; a `NotImplementedError` counts as "no". It asserts that the stream position did not move. `_markitdown.py:655-665` [Verified]
   - Any exception from `convert()` is captured as a `FailedConversionAttempt` and the loop **continues** (so a missing optional dependency does not stop later converters). The stream is always re-seeked. `_markitdown.py:668-678` [Verified]
   - The first non-None result is normalised (rstrip every line, collapse 3+ newlines to 2) and returned. `_markitdown.py:680-686` [Verified]
   - If nothing succeeded: `FileConversionException(attempts)` when some converter tried, else `UnsupportedFormatException`. `_markitdown.py:688-695` [Verified]
5. **Priority semantics.** Lower value = tried first. `PRIORITY_SPECIFIC_FILE_FORMAT = 0.0`, `PRIORITY_GENERIC_FILE_FORMAT = 10.0`. `_markitdown.py:101-107` [Verified]
   - `register_converter` **inserts at index 0**, so within one priority the most recently registered converter runs first. `_markitdown.py:705-735` [Verified]
   - Built-ins are registered generic-first: PlainText, Zip and Html at 10.0, then the specific converters at 0.0. Azure DocIntel and CU are registered last when an endpoint is given, so they run first among the 0.0 converters. `_markitdown.py:229-296` [Verified]
6. **Converter contract.** `accepts(file_stream, stream_info, **kw) -> bool` must be cheap and must restore the stream position. `convert(...) -> DocumentConverterResult(markdown, title)`. `packages/markitdown/src/markitdown/_base_converter.py:42-105` [Verified]
   - Most `accepts()` implementations are just an extension list plus MIME-prefix matching, e.g. `converters/_pdf_converter.py:502-518`. [Verified]
   - URL-specific converters gate on regexes: Wikipedia `^https?://[a-zA-Z]{2,3}(\.m)?\.wikipedia.org/` (`converters/_wikipedia_converter.py:37`), Bing `^https://www\.bing\.com/search\?q=` (`converters/_bing_serp_converter.py:43`), YouTube video-id parsing (`converters/_youtube_converter.py:41-75`). [Verified]
   - `PlainTextConverter` accepts **anything with a detected charset** (`converters/_plain_text_converter.py:34-37`). That is why it must sit at the generic priority. [Verified]
7. **Composite formats re-enter the dispatcher.** `ZipConverter` reads each member into memory and calls `markitdown.convert_stream()` with the member's extension. `UnsupportedFormatException` and `FileConversionException` are silently skipped. `converters/_zip_converter.py:108-130` [Verified]
   - `DocxConverter` runs DOCX pre-processing (OMML math to LaTeX `$…$`/`$$…$$`, strike-through, styles, zip filename casing), then `mammoth.convert_to_html`, then the shared `HtmlConverter`. `converters/_docx_converter.py:90-123`; `converter_utils/docx/pre_process.py:34-70,250` [Verified]
   - `HtmlConverter` drops `<script>`/`<style>` and converts `<body>` with a custom `markdownify` subclass. `converters/_html_converter.py:56-90` [Verified]

### Built-in converters and their dependencies

| Converter | Accepts (key) | Priority | Dependency (extra) | Evidence |
|---|---|---|---|---|
| PlainTextConverter | `text/*`, JSON, `.txt .md .json .jsonl`, or any charset | 10.0 | charset-normalizer (core) | `converters/_plain_text_converter.py:6-19,34-37` |
| ZipConverter | `.zip` | 10.0 | stdlib `zipfile` | `converters/_zip_converter.py:15-19,108` |
| HtmlConverter | `text/html`, `.html .htm` | 10.0 | beautifulsoup4, markdownify (core) | `converters/_html_converter.py:10-19` |
| RssConverter | RSS/Atom XML | 0.0 | defusedxml, bs4 (core) | `converters/_rss_converter.py:6-9,91-113` |
| WikipediaConverter | wikipedia.org URL + HTML | 0.0 | bs4 (core) | `converters/_wikipedia_converter.py:23-47` |
| YouTubeConverter | YouTube watch URL + HTML | 0.0 | `youtube-transcript-api~=1.2.3` (`[youtube-transcription]`), network | `converters/_youtube_converter.py:20,167`; `pyproject.toml:61` |
| BingSerpConverter | bing.com/search URL | 0.0 | bs4 (core) | `converters/_bing_serp_converter.py:29-53` |
| DocxConverter | `.docx` | 0.0 | `mammoth~=1.11.0`, lxml (`[docx]`) | `converters/_docx_converter.py:17,28`; `pyproject.toml:55` |
| XlsxConverter | `.xlsx` | 0.0 | pandas, openpyxl (`[xlsx]`) | `converters/_xlsx_converter.py:16-17,31`; `pyproject.toml:56` |
| XlsConverter | `.xls` | 0.0 | pandas, xlrd (`[xls]`) | `converters/_xlsx_converter.py:23-24,37`; `pyproject.toml:57` |
| PptxConverter | `.pptx` | 0.0 | python-pptx (`[pptx]`) | `converters/_pptx_converter.py:21,31`; `pyproject.toml:54` |
| AudioConverter | audio MIME/ext | 0.0 | exiftool binary; pydub + SpeechRecognition (`[audio-transcription]`) | `converters/_audio_converter.py:9-21,55`; `converters/_transcribe_audio.py:22-23` |
| ImageConverter | `.jpg .jpeg .png` | 0.0 | exiftool binary; optional LLM client | `converters/_image_converter.py:13,48-81` |
| IpynbConverter | `.ipynb` | 0.0 | stdlib json | `converters/_ipynb_converter.py:12,70-93` |
| PdfConverter | `.pdf` | 0.0 | `pdfminer.six>=20251230`, `pdfplumber>=0.11.9` (`[pdf]`) | `converters/_pdf_converter.py:63-65,75`; `pyproject.toml:58` |
| OutlookMsgConverter | `.msg` | 0.0 | olefile (`[outlook]`) | `converters/_outlook_msg_converter.py:16,25` |
| EpubConverter | `.epub` | 0.0 | stdlib zipfile + defusedxml | `converters/_epub_converter.py:3-5,20` |
| CsvConverter | `.csv`, `text/csv` | 0.0 | stdlib csv + charset-normalizer | `converters/_csv_converter.py:1-13` |
| DocumentIntelligenceConverter | DOCX/PPTX/XLSX/HTML/PDF/JPEG/PNG/BMP/TIFF (configurable) | 0.0, only if `docintel_endpoint` | azure-ai-documentintelligence, azure-identity (`[az-doc-intel]`) | `converters/_doc_intel_converter.py:130-191`; `pyproject.toml:62` |
| ContentUnderstandingConverter | documents, images, audio, video | 0.0, only if `cu_endpoint` | `azure-ai-contentunderstanding>=1.2.0b1` (`[az-content-understanding]`) | `converters/_cu_converter.py:441-570`; `pyproject.toml:64` |

All rows [Verified]. Core always-installed deps: beautifulsoup4, requests, markdownify, `magika~=0.6.1`, charset-normalizer, defusedxml (`packages/markitdown/pyproject.toml:27-34`) [Verified]. A missing optional dependency raises `MissingDependencyException` at `convert()` time, not at import (e.g. `converters/_pdf_converter.py:526-535`). [Verified]

## T3 — Architecture and process model

In plain words: MarkItDown is a plain library. Nothing runs in the background: your Python process calls it, it reads the input, and it returns a string. The CLI and the MCP server are thin wrappers over the same `MarkItDown` class.

- Single in-process object `MarkItDown`. It holds a `requests.Session` with an `Accept: text/markdown, text/html;q=0.9, …` header (asks servers that support it for Markdown), a Magika model, global LLM/exiftool options, and the converter list. `_markitdown.py:145-186` [Verified]
- Synchronous and single-threaded. The whole input is buffered: non-seekable streams are copied into `BytesIO` (`_markitdown.py:435-440`), HTTP bodies are fully buffered (`_markitdown.py:590-594`), and PDFs are copied again into `BytesIO` (`converters/_pdf_converter.py:540`). [Verified]
- External processes: only `exiftool`, run via `subprocess.run([path, "-json", "-"])` with list arguments (`converters/_exiftool.py:21-55`). The exiftool path is accepted only from a fixed set of system directories when it is auto-discovered. `_markitdown.py:202-224` [Verified]
- CLI `markitdown` entry point is `markitdown.__main__:main` (`pyproject.toml:74-75`). Flags: `--output --extension --mime-type --charset --use-docintel --use-cu --endpoint --cu-endpoint --cu-analyzer --cu-file-types --use-plugins --list-plugins --keep-data-uris`. `packages/markitdown/src/markitdown/__main__.py:53-145` [Verified]
- The MCP server builds a **new `MarkItDown` per tool call**, which re-initialises Magika every time. `packages/markitdown-mcp/src/markitdown_mcp/__main__.py:22` [Verified]; the per-call model load cost is [Inference].

## T4 — Data model

In plain words: there is no database and nothing is saved. Inputs are byte streams with a small "what is this file" record attached, and the output is a Markdown string with an optional title.

- `StreamInfo` (6 optional fields, frozen). `_stream_info.py:5-18` [Verified]
- `DocumentConverterResult(markdown, title=None)`; `text_content` is a soft-deprecated alias. **No page numbers, offsets, per-section metadata or source map.** `_base_converter.py:5-39` [Verified]
- `ConverterRegistration(converter, priority)` frozen dataclass. `_markitdown.py:133-138` [Verified]
- `FailedConversionAttempt` / `FileConversionException` carry `exc_info` per converter. `packages/markitdown/src/markitdown/_exceptions.py` (82 lines) [Verified, file read by size only]
- Output conventions: ZIP → `## File: <name>` sections (`converters/_zip_converter.py:123-124`); XLSX → `## <sheet>` + HTML table converted to Markdown (`converters/_xlsx_converter.py:152-153`); PPTX → per-slide content plus `### Notes:` (`converters/_pptx_converter.py:139-146`); Content Understanding → YAML front matter of extracted fields via the SDK's `to_llm_input()` (`converters/_cu_converter.py:1-8,566-570`). [Verified]

## T5 — Interfaces

In plain words: you can use it as a Python library, as a `markitdown` command, as an MCP server with one tool, or through plugins. The MCP server has no login of any kind.

- Python: `MarkItDown(enable_builtins=None|bool, enable_plugins=bool, llm_client, llm_model, llm_prompt, exiftool_path, style_map, docintel_endpoint, docintel_credential, docintel_file_types, docintel_api_version, cu_endpoint, cu_credential, cu_analyzer_id, cu_file_types, requests_session)`. `_markitdown.py:145-296` [Verified]
  - Methods: `convert`, `convert_local`, `convert_stream`, `convert_uri`, `convert_url` (alias), `convert_response`, `register_converter(converter, priority=0.0)`, `register_page_converter` (deprecated), `enable_builtins`, `enable_plugins`. `_markitdown.py:188-735` [Verified]
- CLI: see T3; `--list-plugins`, `--use-plugins`. `README.md:113-127` [Docs]
- MCP: `MCPServer("markitdown")` with a single tool, `convert_to_markdown(uri: str) -> str`: "Convert a resource described by an http:, https:, file: or data: URI to markdown". `packages/markitdown-mcp/src/markitdown_mcp/__main__.py:16-24` [Verified]
  - Transports: STDIO by default. `--http` (alias `--sse`) serves Streamable HTTP (stateless, JSON responses) **and** SSE on one Starlette app, default `127.0.0.1:3001`. `__main__.py:58-135` [Verified]
  - Errors are mapped to `ToolError` with sanitised messages: HTTP status only, `os.strerror` instead of the path. `__main__.py:26-47` [Verified]
  - Plugins in MCP are enabled only via env `MARKITDOWN_ENABLE_PLUGINS=true|1|yes`. `__main__.py:50-55` [Verified]
- Plugin API: a module exposing `__plugin_interface_version__ = 1` and `register_converters(markitdown, **kwargs)`, published under entry-point group `markitdown.plugin`. `packages/markitdown-sample-plugin/src/markitdown_sample_plugin/_plugin.py:13-31`; `packages/markitdown-sample-plugin/pyproject.toml:39-40`; `packages/markitdown-ocr/pyproject.toml:56-57` [Verified]
  - Loading: `entry_points(group="markitdown.plugin")`, `.load()`, failures become warnings; then `plugin.register_converters(self, **kwargs)`. `_markitdown.py:113-130,302-320` [Verified]
  - **The core never reads `__plugin_interface_version__`** (grep finds it only in plugins). [Verified]

## T6 — How it plugs into agents

In plain words: agents usually reach MarkItDown through its MCP server (Claude Desktop and other MCP clients), or by importing it as a library in Python agent code. Either way the agent gets back one Markdown string.

- The MCP README documents a Claude Desktop `claude_desktop_config.json` setup and Docker runs with a volume mount (`docker run -it --rm -v /home/user/data:/workdir markitdown-mcp:latest`). `packages/markitdown-mcp/README.md:47-61` [Docs]
- MCP Inspector instructions cover the STDIO, Streamable HTTP and SSE transports. `packages/markitdown-mcp/README.md:112-122` [Docs]
- The MCP package is "meant for **local use**, with local trusted agents". `packages/markitdown-mcp/README.md:4` [Docs]
- Library route: any Python agent can call `MarkItDown().convert_stream(...)`. The README recommends "call the narrowest `convert_*` function needed". `README.md:7` [Docs]

## T7 — LLM usage

In plain words: the core works fully offline for normal office files and PDFs. AI models are used only if you pass in an LLM client (to caption images or OCR them) or switch on the Azure services. Audio transcription quietly uses Google's online speech service.

- Magika is a local ML model, created at `_markitdown.py:168` with no network call in this code path. [Verified]. The Docker image sets `ORT_DISABLE_TELEMETRY=1` (`Dockerfile:6`) [Verified], which implies ONNX Runtime underneath [Inference].
- Image captions: if `llm_client` and `llm_model` are set, `ImageConverter` sends the image as a base64 data URI to `client.chat.completions.create`. Default prompt "Write a detailed caption for this image." `converters/_image_converter.py:68-97`; `converters/_llm_caption.py:8-49` [Verified]
- PPTX picture captions use the same helper. `converters/_pptx_converter.py:181-191` [Verified]. The README says LLM captions are "currently only for pptx and image files". `README.md:284-291` [Docs]
- OCR plugin: `LLMVisionOCRService` makes one vision chat call per image (or per page at 300 DPI for scanned PDFs). Default prompt "Extract all text from this image. Return ONLY the extracted text…". `packages/markitdown-ocr/src/markitdown_ocr/_ocr_service.py:27-109`; `_pdf_converter_with_ocr.py:340-410` [Verified]. Cost scales with the number of images or pages [Inference].
- Azure Document Intelligence: model `prebuilt-layout`, `output_content_format="markdown"`, features FORMULAS + OCR_HIGH_RESOLUTION + STYLE_FONT for PDF and images; HTML comments are stripped from the result. Credential: `AZURE_API_KEY`, else `DefaultAzureCredential`. `converters/_doc_intel_converter.py:174-258` [Verified]
- Azure Content Understanding: per-modality prebuilt analyzers (`prebuilt-documentSearch`, `prebuilt-videoSearch`, `prebuilt-audioSearch`) or a custom analyzer routed by modality. `begin_analyze_binary` → `to_llm_input(result)`. `converters/_cu_converter.py:240-245,488-570` [Verified]
- Audio: `speech_recognition.Recognizer().recognize_google(...)` (Google Web Speech, online). `converters/_transcribe_audio.py:56-59` [Verified]
- YouTube: fetches transcripts with `YouTubeTranscriptApi` (online), retrying 3 times with a 2 s delay. `converters/_youtube_converter.py:167,244` [Verified]

## T8 — Security, privacy, safety, legal

In plain words: MarkItDown trusts its caller completely. If an attacker can choose the input string, they can make it read local files or call internal URLs, a server-side request forgery (SSRF) risk. Converted text also goes straight into the model, so hidden instructions inside documents reach the agent. All of this must be wrapped before any multi-user use.

- Official stance: "MarkItDown performs I/O with the privileges of the current process… Sanitize your inputs… call the narrowest `convert_*` function". It names path restriction, URI-scheme limits and blocking "private, loopback, link-local, or metadata-service addresses" as the **caller's** job. `README.md:7,394-398` [Docs]
- **Auto-routing hazard.** `convert(str)` silently turns any `http(s)://`, `file:` or `data:` string into a fetch or read. `_markitdown.py:337-349` [Verified]
- **SSRF.** `requests.Session.get(uri, stream=True)` with **no timeout** and no host or IP filtering. `_markitdown.py:514-516` [Verified]. Requests follows redirects on GET by default, so even a pre-validated URL can bounce to `169.254.169.254` or an intranet host [Inference].
- **File reads.** `file:` URIs are allowed when the netloc is empty or `localhost` (`_markitdown.py:482-494`); `_uri_utils.py:9` contains a UNC/device-path check. The MCP tool therefore reads **any file the server user can read**, which the MCP README states explicitly. `packages/markitdown-mcp/README.md:132-134` [Verified/Docs]
- **Resource exhaustion.** Unbounded in-memory buffering of HTTP bodies (`_markitdown.py:590-594`) and of every ZIP member (`converters/_zip_converter.py:112`). Recursive ZIP-in-ZIP via `convert_stream` has no depth limit (`converters/_zip_converter.py:117-121`). [Verified]; zip-bomb exposure is [Inference].
- MCP HTTP mode: **no auth**. Binds `127.0.0.1` by default and prints a loud warning for any other host ("NO authentication and runs with your user's privileges"). `packages/markitdown-mcp/src/markitdown_mcp/__main__.py:114-127` [Verified]
  - The Starlette app is created with `debug=True`. `__main__.py:128` [Verified]. Unhandled errors could then return tracebacks [Inference].
- Defences that do exist:
  - `defusedxml` for RSS and EPUB (XXE). `converters/_rss_converter.py:6`, `converters/_epub_converter.py:5` [Verified]
  - Link schemes other than http/https/file are dropped (neutralises `javascript:`). `converters/_markdownify.py:77-78` [Verified]
  - Data-URI images are truncated unless `keep_data_uris`. `converters/_markdownify.py:123-141` [Verified]
  - Exiftool auto-discovery is pinned to system dirs. `_markitdown.py:205-224` [Verified]
  - Docker image runs as `nobody:nogroup`. `Dockerfile:28-32` [Verified]
- **Prompt injection.** Converted Markdown is untrusted content. Only `<script>`/`<style>` are removed from HTML (`converters/_html_converter.py:56-58`), so hidden HTML text, PPTX speaker notes (`converters/_pptx_converter.py:139-146`) and white-on-white PDF text all flow into the agent's context [Verified for notes/script; rest Inference].
- **Plugins are code execution.** Entry points run at `enable_plugins()` (`_markitdown.py:123-125`). They are off by default (`_markitdown.py:185-186`). [Verified]
- **Data egress.** Images go to the LLM provider, audio to Google, whole files to Azure DI/CU (`converters/_doc_intel_converter.py:248-253`). [Verified]. Each needs a data-processing decision before use on client documents [Inference].
- **Licence of dependencies.** Core deps are permissive [Inference from names]. `markitdown-ocr` hard-depends on `PyMuPDF>=1.24.0` (`packages/markitdown-ocr/pyproject.toml:31`) [Verified]. PyMuPDF is dual-licensed AGPL-3.0 / Artifex commercial (https://pymupdf.readthedocs.io/en/latest/about.html) [Docs]. The import is only used as a fallback renderer (`_pdf_converter_with_ocr.py:387-398`) [Verified], but the dependency is installed regardless.
- Vulnerability reporting goes to MSRC, not public issues. `SECURITY.md:1-17` [Verified]

## T9 — Quality: tests, benchmarks, CI

In plain words: the project has a solid test suite (about 470 test functions) running on five Python versions, plus Windows and Windows ARM. There are no published quality benchmarks for conversion fidelity. Tests that need the network or an OpenAI key are skipped unless those are provided.

- Lexical test count is **474** `def test_` across packages. The heaviest files: `test_docx.py` 54, `test_cu.py` 48, `test_rss.py` 41, `test_module_misc.py` 37, `test_pptx.py` 35, `test_pdf.py` 32; the OCR plugin has 78 and MCP 13. [Verified via grep; not executed]
- CI (`.github/workflows/tests.yml`) runs on pull requests only. `.github/workflows/tests.yml:2` [Verified]
  - Three jobs (`tests`, `ocr-tests`, `mcp-tests`), each on ubuntu × Python 3.10–3.14, plus windows-latest and windows-11-vs2026-arm on 3.12. `.github/workflows/tests.yml:12-102` [Verified]
  - exiftool and ffmpeg are installed on Linux. `.github/workflows/tests.yml:31-38` [Verified]
  - Actions are pinned by SHA and there is a constraints file. `.github/workflows/tests.yml:9-10,27-28` [Verified]
- Remote and LLM tests are gated (`skip_remote`, `skip_llm` unless `OPENAI_API_KEY`). `packages/markitdown/tests/test_module_misc.py:35-45` [Verified]
- No fidelity benchmark (table-structure accuracy, heading recall) exists in the repo. [Verified by file inventory]

## T10 — Operations

In plain words: you install it with `pip install 'markitdown[all]'`. Exiftool and ffmpeg are optional system tools. Python 3.10 to 3.14 is supported, and there is a small Docker image.

- `pip install 'markitdown[all]'` or per-format extras (`[pdf]`, `[docx]`, `[pptx]`, `[xlsx]`, `[xls]`, `[outlook]`, `[audio-transcription]`, `[youtube-transcription]`, `[az-doc-intel]`, `[az-content-understanding]`). `packages/markitdown/pyproject.toml:36-64` [Verified]
- Docker: `python:3.13-slim-trixie` + ffmpeg + exiftool. Installs `markitdown[all]` + the sample plugin; `ENTRYPOINT ["markitdown"]`. `Dockerfile:1-34` [Verified]
- MCP: `pip install markitdown-mcp` (depends `mcp>=2.1.1,<3.0.0`, `markitdown[all]`). Script `markitdown-mcp`. `packages/markitdown-mcp/pyproject.toml:26-42` [Verified]
- Footprint driver: `[all]` pulls pandas, lxml, pdfminer/pdfplumber, the Azure SDKs and Magika (ONNX model) [Verified list; size Inference].

## T11 — What is actually new vs marketing

In plain words: the clever bit is the honest, simple dispatch design: mix several file-type guesses and let converters volunteer in priority order. Most of the actual conversion is done by well-known libraries (mammoth, pdfminer, pandas, python-pptx). A few claims in docs and docstrings do not match the code.

- **Genuinely good:** Magika + mimetypes + header guesses with an explicit compatibility check that yields *multiple* guesses (`_markitdown.py:783-829`). The stable-sort priority registry lets plugins slot in at any float (`_markitdown.py:705-735`). The `Accept: text/markdown` preference for "markdown for agents" servers (`_markitdown.py:158-164`). [Verified]
- **Delegation, not invention:** DOCX = mammoth → HTML → markdownify; XLSX = `pandas.to_html` → markdownify; PDF = pdfminer / pdfplumber heuristics. `converters/_docx_converter.py:113-123`, `converters/_xlsx_converter.py:153`, `converters/_pdf_converter.py:552-584` [Verified]
- **Claim vs code:**
  - The ZIP docstring says it "extracts the ZIP contents to a temporary directory … cleaned up after processing". The code reads members into memory and never touches a temp dir. `converters/_zip_converter.py:25-27` vs `converters/_zip_converter.py:108-121` [Verified]
  - The README says the OCR plugin needs "No new ML libraries or binary dependencies". `README.md:131` [Docs]. Yet it hard-requires PyMuPDF and Pillow (binary wheels). `packages/markitdown-ocr/pyproject.toml:27-38` [Verified]
  - `__plugin_interface_version__` is advertised but never checked by the core. [Verified by grep]

## T12 — Weaknesses, limits, risks

In plain words: tables and layout are only partly preserved, scanned documents yield nothing without OCR, images are reduced to metadata or a caption, and you cannot tell which page a sentence came from. The security gaps in T8 are the biggest risk for a multi-user platform.

- **PDF.** If no page looks like a form or table, the whole document goes through `pdfminer.extract_text`, so **no headings, bold or lists**. Tables are reconstructed only by word-position heuristics. `converters/_pdf_converter.py:120,398,552-584` [Verified]
- **Scanned PDFs** return empty or near-empty text in the core: there is no OCR without the plugin or Azure. [Inference from code path; `converters/_pdf_converter.py:581-584` only retries pdfminer]
- **Images.** Only `.jpg/.jpeg/.png` are accepted (`converters/_image_converter.py:13`). The output is EXIF fields (ImageSize, Title, Caption, …, GPSPosition) plus an optional LLM caption (`converters/_image_converter.py:53-81`). GPS coordinates are emitted into the Markdown, a privacy point [Verified/Inference].
- **Spreadsheets.** Every sheet is rendered as a full table (`converters/_xlsx_converter.py:151-153`), so large workbooks explode token counts. Merged cells and formulas are not represented [Inference: pandas reads values].
- **No provenance.** The result has only `markdown` and `title` (`_base_converter.py:8-25`), so no page or slide anchors for citations or a knowledge graph. [Verified]
- Silent drops: ZIP members that fail are skipped without a note (`converters/_zip_converter.py:125-128`). OCR errors become empty text (`packages/markitdown-ocr/src/markitdown_ocr/_ocr_service.py:108-109`). [Verified]
- Stability: beta and "in preview"; legacy kwargs (`file_extension`, `url`) are still threaded through. `_markitdown.py:142,370-397,647-653` [Verified]

## T13 — How it fits into omni-agent

In plain words: MarkItDown should be omni-agent's **ingestion layer**. Every spec, PDF, slide deck, spreadsheet, email or notebook a team touches becomes Markdown, which is chunked, cited and stored in the knowledge graph. Use it as a Python library inside a locked-down worker, not as an open MCP server, and keep the OCR plugin optional because of its AGPL dependency.

- **Lifecycle role** [Inference]:
  - Discover: research PDFs, market decks, interview transcripts via `[audio-transcription]` only with consent.
  - Plan: PRDs (DOCX), backlog exports (CSV/XLSX), roadmap decks (PPTX).
  - Build: notebooks (IPYNB), API docs (HTML), architecture PDFs.
  - Verify: test reports (HTML/CSV/JSON).
  - Release: release notes and changelogs.
  - Operate: incident emails (`.msg`) and runbooks.
  - Learn: retro decks and survey spreadsheets.
- **Integration mode** [Inference]:
  - Import as a library (MIT, compatible with omni-agent's MIT licence; keep the notice).
  - Never expose `convert()`/`convert_uri()` to model-chosen strings. Call `convert_stream()` with bytes that omni-agent's own fetcher has already obtained.
  - Run conversion in a sandboxed subprocess or container with no network, CPU/RAM/time limits and a read-only mount.
  - Allowlist plugins by entry-point name. Ship `markitdown-ocr` only as an opt-in, separately installed worker, after a PyMuPDF AGPL review or a commercial licence.
  - Put Azure DI/CU and LLM captioning behind an org-level data-egress policy flag.
- **Hardening to add in the wrapper:**
  - Hardened fetch: http/https only; DNS-resolve and block private, loopback, link-local and metadata IPs; re-validate every redirect or disable redirects.
  - Limits: max 50 MB per file, timeouts, ZIP member count/size/ratio and nesting caps.
  - Strip GPS EXIF lines.
  - Tag all output `trust: untrusted` so the agent's injection defences apply. [Inference]
- **Fidelity upgrades:**
  - Add page and slide anchors by converting per page or slide (pdfplumber pages, python-pptx slides) in custom converters registered at priority < 0, which the registry supports (`_markitdown.py:705-735`).
  - Chunk on Markdown headings for the graph. [Inference]
- **Proposed interface contract** [Inference]:

```text
ingest(req: IngestRequest) -> IngestResult
IngestRequest  { source: {kind: "upload"|"repo_path"|"url", ref: str}, project_id, max_bytes=50MB,
                 allow_cloud=false, allow_llm_vision=false, plugins=[] }
IngestResult   { doc_id: sha256(bytes), source_ref, detected: {mimetype, extension, charset, magika_label},
                 converter: "PdfConverter@markitdown-0.1.8", title, markdown,
                 chunks: [{chunk_id, heading_path[], text, anchor: {page|slide|sheet|member}?}],
                 warnings: [str], egress: ["azure-docintel"|"llm:<model>"|...], trust: "untrusted",
                 ingested_at }
errors: UNSUPPORTED_FORMAT | CONVERSION_FAILED(attempts[]) | TOO_LARGE | TIMEOUT | BLOCKED_SOURCE
```

- Effort [Inference]: wrapper + fetcher + sandbox ≈ 2–3 days. Per-page/slide anchors + chunker ≈ 3–5 days. Graph upsert keyed by `doc_id` with re-ingest on hash change ≈ 2 days.
- Do **not** reuse `markitdown-mcp` as-is for a shared or hosted omni-agent: it has no auth, any-file read, and `debug=True` (T8). If MCP is wanted, write a thin omni-agent MCP tool that accepts upload IDs only. [Inference]

## T14 — Open questions

In plain words: these could not be settled from the source alone and need a quick experiment or a vendor answer.

- Magika accuracy and latency on truncated or polyglot files, and the per-call model load cost in MCP (`markitdown_mcp/__main__.py:22`). Needs a benchmark. [Inference]
- Table fidelity of the pdfplumber heuristics (`converters/_pdf_converter.py:120-494`) on real Agile artefacts (Jira PDF exports, scanned SOWs). No benchmark exists in the repo. [Verified absence]
- Whether a future release enforces `__plugin_interface_version__` or adds size and timeout options, so the wrapper limits could be dropped. [Open]
- PyMuPDF licensing posture for a distributed or hosted omni-agent, if OCR is required. Legal review needed. [Open]
- Behaviour of `requests` redirects plus `stream=True` with omni-agent's own `requests_session` injection (`_markitdown.py:155-166`). Can a custom session with an SSRF-safe adapter fully replace the default? It looks supported [Inference] but is untested.
