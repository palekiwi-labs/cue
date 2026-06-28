# Rust Markdown Chunking / Splitting Crates — Research Report

Research date: 2026-06-28
Sources: crates.io API, direct crate metadata

---

## Existence Checks

| Crate | Exists? |
|---|---|
| `markdown-splitter` (hyphen) | YES — but it is `markdown_splitter` (underscore) |
| `document-splitter` | NO — 404 on crates.io |

---

## Group 1: Markdown-Specific Chunking / Splitting

### 1. text-splitter

| Field | Value |
|---|---|
| crates.io | https://crates.io/crates/text-splitter |
| Repository | https://github.com/benbrandt/text-splitter |
| Version | 0.32.0 |
| Last release | 2026-06-16 |
| Downloads | 1,576,991 |
| License | MIT |
| Markdown support | YES — explicit `markdown` feature flag |

**Markdown feature:** enables `dep:pulldown-cmark`. Parses the CommonMark AST
and splits at semantic markdown boundaries (headings, paragraphs, code blocks,
etc.) rather than splitting raw bytes.

**Other feature flags:** `code` (tree-sitter), `tiktoken-rs`, `tokenizers`.

**API summary:** Trait-based. Construct a `TextSplitter` with a chunk-size
strategy (character count, token count, or a custom tokenizer). Call
`.chunks(&text)` or `.chunk_indices(&text)` to get an iterator over chunks.
With the `markdown` feature enabled, the splitter understands markdown
structure and avoids splitting in the middle of inline elements.

```rust
use text_splitter::{ChunkConfig, MarkdownSplitter};
let splitter = MarkdownSplitter::new(ChunkConfig::new(500));
let chunks: Vec<&str> = splitter.chunks(markdown_text).collect();
```

**Assessment:** The most mature, most-downloaded crate in this space.
Actively maintained as of June 2026. The clear default choice.

---

### 2. markdown-chunk

| Field | Value |
|---|---|
| crates.io | https://crates.io/crates/markdown-chunk |
| Repository | https://github.com/MukundaKatta/markdown-chunk |
| Version | 0.1.0 |
| Last release | 2026-05-16 |
| Downloads | 15 |
| License | MIT OR Apache-2.0 |
| Markdown support | YES — heading-hierarchy-aware |

**Description:** "Split Markdown into RAG-friendly chunks that respect heading
hierarchy. Keeps each chunk under a soft char cap; never splits inside a
fenced code block. Zero deps."

**API summary:** Single-purpose library. Splits a markdown string by heading
structure. Respects fenced code block boundaries. Takes a character-count cap
as a parameter. Zero external dependencies.

**Assessment:** Very new (May 2026), minimal downloads. Extremely focused
scope. Zero-dep is attractive, but the library is only 53 lines of Rust code.
Suitable for simple use cases but untested at scale.

---

### 3. markdown-rag

| Field | Value |
|---|---|
| crates.io | https://crates.io/crates/markdown-rag |
| Repository | https://github.com/menjaraz/markdown-rag |
| Version | 0.1.0 |
| Last release | 2026-05-29 |
| Downloads | 16 |
| License | MIT OR Apache-2.0 |
| Markdown support | YES — explicitly markdown + RAG focused |

**Description:** "A semantic markdown document loader and splitter for RAG
pipelines."

**Keywords:** chunking, markdown, nlp, rag, text-splitting.

**API summary:** 476 lines of Rust across 5 files. Loads and splits markdown
documents semantically, targeting RAG (Retrieval-Augmented Generation)
ingestion pipelines.

**Assessment:** Very new, minimal downloads, single version. Interesting for
RAG-specific workflows, but too early to evaluate reliability.

---

### 4. markdown_splitter (annotation-based)

| Field | Value |
|---|---|
| crates.io | https://crates.io/crates/markdown_splitter |
| Repository | https://github.com/romainbou/markdown-splitter |
| Version | 0.1.1 |
| Last release | 2020-04-18 |
| Downloads | 1,755 |
| License | Apache-2.0 |
| Markdown support | Partial — annotation-based, not semantic |

**Description:** "Utility tool to split a Markdown file into chunks from
annotations."

**API summary:** CLI binary (`mds`). Splits a markdown file at user-placed
annotations (comments or markers within the file), not by heading structure
or token count. 89 lines of Rust total.

**Assessment:** Effectively abandoned (2020, single version). Not a
semantic splitter. Useful only if you control the source markdown and can
place explicit split markers. Not suitable for general RAG chunking.

---

### 5. md-scatter

| Field | Value |
|---|---|
| crates.io | https://crates.io/crates/md-scatter |
| Repository | https://github.com/ncipollo/md-scatter |
| Version | 0.1.2 |
| Last release | 2025-12-23 |
| Downloads | 174 |
| License | MIT |
| Markdown support | YES — splits/reassembles markdown files |

**Description:** "A tool to split up and reassemble markdown files."

**API summary:** CLI binary. Splits markdown files (likely by headings) into
separate files on disk, and can reassemble them. 1,951 lines of Rust across
24 files. More of a document management tool than a chunking library for
in-process use.

**Assessment:** Useful as a build/document-management tool (e.g., splitting
a large markdown book). Not designed for programmatic chunking in an
embedding pipeline.

---

## Group 2: General Semantic Chunking (Markdown Included)

### 6. chunkedrs

| Field | Value |
|---|---|
| crates.io | https://crates.io/crates/chunkedrs |
| Repository | https://github.com/goliajp/rust-chunker |
| Version | 1.0.4 |
| Last release | 2026-06-07 |
| Downloads | 123 |
| License | MIT |
| Markdown support | YES — explicitly "markdown-aware" |

**Description:** "AI-native text chunking — recursive, markdown-aware, and
semantic splitting with token-accurate boundaries."

**Feature flags:**
- `default`: base chunking
- `semantic`: enables `dep:embedrs` for embedding-based semantic splitting

**API summary:** 1,378 lines of Rust across 5 files. Recursive text splitter
that understands markdown structure. Optional semantic mode uses embeddings
to find split points. Token-accurate boundary calculation. Uses Rust 2024
edition.

**Assessment:** Active as of June 2026. Explicitly markdown-aware. The
optional embedding-based semantic mode makes it more flexible than most
alternatives. Low download count despite being a v1 release, which warrants
caution.

---

### 7. niblits

| Field | Value |
|---|---|
| crates.io | https://crates.io/crates/niblits |
| Repository | https://github.com/casualjim/niblits |
| Version | 0.3.14 |
| Last release | 2026-06-22 |
| Downloads | 486 |
| License | MIT |
| Markdown support | Likely — "multi-format" |

**Description:** "Token-aware, multi-format text chunking library with
language-aware semantic splitting."

**Keywords:** chunking, embeddings, parsing, text, tokenization.
**Categories:** algorithms, asynchronous, parsing, text-processing.

**API summary:** 5,908 lines of Rust across 17 files — the largest library
in this survey by source size. Token-aware chunking across multiple input
formats. Language-aware semantic boundary detection. Async support
(tokio-compatible based on the `asynchronous` category). Actively iterated:
13 versions since January 2026.

**Assessment:** The most substantial and actively developed general-purpose
chunking library found. Multi-format strongly implies markdown support,
though this is not explicitly stated in the API metadata. Worth verifying
the README for confirmed format list.

---

### 8. chunk (chonkie-inc)

| Field | Value |
|---|---|
| crates.io | https://crates.io/crates/chunk |
| Repository | https://github.com/chonkie-inc/chunk |
| Version | 0.10.2 |
| Last release | 2026-05-28 |
| Downloads | 12,002 |
| License | MIT OR Apache-2.0 |
| Markdown support | Unconfirmed — SIMD semantic splitting |

**Description:** "The fastest semantic text chunking library — up to 1TB/s
chunking throughput."

**Keywords:** chunking, nlp, simd, text, tokenization.

**API summary:** 2,203 lines of Rust across 6 files. SIMD-accelerated
semantic text chunking. No explicit markdown feature in the API metadata.
Focuses on throughput. From the chonkie-inc organisation (also behind
`memchunk` below).

**Assessment:** Highest download count of the non-text-splitter crates in
this list. The "1TB/s" claim is a marketing figure — likely refers to
character throughput under ideal conditions. Markdown support is unclear
from the metadata alone; the library appears sentence/token-boundary
oriented rather than document-structure oriented.

---

### 9. kiru

| Field | Value |
|---|---|
| crates.io | https://crates.io/crates/kiru |
| Repository | https://github.com/bitswired/kiru |
| Version | 0.1.11 |
| Last release | 2025-11-11 |
| Downloads | 4,121 |
| License | MIT |
| Markdown support | Unconfirmed |

**Description:** "Fast text chunking for Rust."

**Keywords:** chunking, nlp, rag, text.

**API summary:** 905 lines of Rust across 6 files. Fast general text
chunking aimed at RAG pipelines. No explicit markdown feature. No updates
since November 2025.

**Assessment:** Reasonable download count for its age. General-purpose;
no evidence of markdown-structure awareness. Development appears to have
slowed.

---

### 10. memchunk (chonkie-inc)

| Field | Value |
|---|---|
| crates.io | https://crates.io/crates/memchunk |
| Repository | https://github.com/chonkie-inc/memchunk |
| Version | 0.4.0 |
| Last release | 2026-01-05 |
| Downloads | 1,926 |
| License | MIT OR Apache-2.0 |
| Markdown support | Unconfirmed |

**Description:** "The fastest semantic text chunking library — up to 1TB/s
chunking throughput." (identical to `chunk`)

**Assessment:** Predecessor to `chunk` from the same organisation and
author. Both share the same description and keyword set. `chunk` is the
current active crate; `memchunk` appears to have been superseded. Prefer
`chunk` if evaluating this family.

---

## Group 3: Document Conversion Engine (includes chunking)

### 11. transmutation

| Field | Value |
|---|---|
| crates.io | https://crates.io/crates/transmutation |
| Repository | https://github.com/hivellm/transmutation |
| Docs | https://docs.rs/transmutation |
| Version | 0.3.3 |
| Last release | 2026-06-18 |
| Downloads | 2,838 |
| License | MIT |
| Markdown support | YES — one of 27 supported formats |

**Description:** "High-performance document conversion engine for AI/LLM
embeddings - 27 formats supported."

**Feature flags:**
- `default`: office (docx, xlsx)
- `office`: docx-rs, umya-spreadsheet
- `pdf-to-image`: pdfium-render
- `image-ocr`: tesseract
- `audio`, `video`, `archives-extended`, `cli`
- `docling-ffi`: ort, ndarray (ML inference)
- `full`: everything

**API summary:** 12,638 lines of Rust across 57 files plus C++ bindings.
Not a chunking library per se; it is a document-format conversion pipeline
that extracts text from 27 formats (PDF, DOCX, XLSX, images with OCR,
audio, video, archives) into a unified representation suitable for LLM
embeddings. Markdown is one input format among many. Has an optional
ML-backed layout analysis via `docling-ffi`.

**Assessment:** Useful if the problem is ingesting diverse document types,
not just splitting existing markdown. Much heavier dependency footprint than
the other crates here.

---

## Group 4: Infrastructure (Parser, Not Splitter)

### 12. pulldown-cmark

| Field | Value |
|---|---|
| crates.io | https://crates.io/crates/pulldown-cmark |
| Repository | https://github.com/raphlinus/pulldown-cmark |
| Version | 0.13.4 |
| Last release | 2026-05-20 |
| Downloads | 111,356,577 |
| License | MIT |
| Markdown support | YES — it IS the markdown parser |

**Description:** "A pull parser for CommonMark."

**Feature flags:**
- `default`: getopts, html
- `html`: pulldown-cmark-escape
- `simd`: SIMD-accelerated escape

**API summary:** Event-stream pull parser for CommonMark markdown. Returns
an iterator of `Event` values (Start, End, Text, Code, etc.) that describe
the document structure. Not a text splitter; used as the underlying parser
by `text-splitter`'s `markdown` feature.

**Assessment:** The de facto standard CommonMark parser in the Rust
ecosystem with 111M downloads. If building a custom markdown-aware splitter
from scratch, this is the parser to use.

---

## Summary Table

| Crate | Version | Last Release | Downloads | Markdown-Aware | Maturity |
|---|---|---|---|---|---|
| text-splitter | 0.32.0 | 2026-06-16 | 1,576,991 | YES (feature flag) | Production |
| pulldown-cmark | 0.13.4 | 2026-05-20 | 111,356,577 | YES (IS the parser) | Production |
| chunkedrs | 1.0.4 | 2026-06-07 | 123 | YES (explicit) | Early |
| niblits | 0.3.14 | 2026-06-22 | 486 | Likely (multi-format) | Early |
| chunk | 0.10.2 | 2026-05-28 | 12,002 | Unclear | Early |
| kiru | 0.1.11 | 2025-11-11 | 4,121 | Unclear | Early |
| transmutation | 0.3.3 | 2026-06-18 | 2,838 | YES (one of 27) | Early |
| memchunk | 0.4.0 | 2026-01-05 | 1,926 | Unclear | Superseded |
| markdown_splitter | 0.1.1 | 2020-04-18 | 1,755 | Partial (annotations) | Abandoned |
| md-scatter | 0.1.2 | 2025-12-23 | 174 | YES (file tool) | Early |
| markdown-rag | 0.1.0 | 2026-05-29 | 16 | YES (RAG focused) | Alpha |
| markdown-chunk | 0.1.0 | 2026-05-16 | 15 | YES (heading-aware) | Alpha |
| document-splitter | — | — | — | — | Does not exist |

---

## Recommendations

1. **For production use:** `text-splitter` with the `markdown` feature is
   the only battle-tested option. It understands the CommonMark AST via
   `pulldown-cmark` and respects heading/paragraph/code-block boundaries.

2. **For lightweight zero-dep use:** `markdown-chunk` (0.1.0) splits by
   heading hierarchy and respects fenced code blocks with no dependencies,
   but at 53 lines of code it is immature.

3. **For custom parsing:** Build on `pulldown-cmark` directly. Its event
   stream gives full AST access to implement any splitting strategy.

4. **For multi-format ingestion pipelines:** `transmutation` handles 27
   input formats but is a much heavier dependency graph.

5. **For embedding-guided semantic splits:** `chunkedrs` offers an optional
   `semantic` feature using embeddings, though it is early-stage.
