# Rust RAG Ecosystem Research

Date: 2026-06-28

## Sources

All data sourced from crates.io API, GitHub READMEs, and crate metadata.
Data is concrete and verified — not from training memory.

---

## 1. rig / rig-core

- **Crate**: `rig-core` (core) + `rig` (facade with feature-gated companion crates)
- **Version**: 0.39.0
- **Last updated**: 2026-06-19
- **Created**: 2024-05-29
- **Downloads**: 1,354,317
- **Repository**: https://github.com/0xPlaygrounds/rig
- **Description**: "An opinionated library for building LLM-powered applications."
- **Status**: Actively maintained. Warns of breaking changes in README.

### RAG Support

Yes. Provides a unified `VectorStore` trait with 12+ backend integrations.
Also provides `EmbeddingModel` trait for generating embeddings.
Supports full retrieval pipelines: embed -> store -> retrieve -> prompt.

### Vector Store Integrations (via companion crates)

| Crate | Feature flag | Type |
|---|---|---|
| rig-lancedb | `lancedb` | Embedded/serverless |
| rig-qdrant | `qdrant` | External server |
| rig-sqlite | `sqlite` | Embedded |
| rig-mongodb | `mongodb` | External server |
| rig-postgres | `postgres` | External server (pgvector) |
| rig-surrealdb | `surrealdb` | Embedded or external |
| rig-milvus | `milvus` | External server |
| rig-neo4j | `neo4j` | External server |
| rig-scylladb | `scylladb` | External server |
| rig-vectorize | `vectorize` | Cloudflare (external) |
| rig-s3vectors | `s3vectors` | AWS (external) |
| rig-helixdb | `helixdb` | External |

### LLM Providers

20+ providers under a single unified interface. Includes OpenAI, Anthropic,
Gemini, Bedrock, Ollama, etc.

### Notable production users

St. Jude (genomics), Neon (app.build), Dria (decentralized AI), ilert
(incident management), Ryzome (AI workspace).

### Assessment

Most actively maintained Rust LLM framework. Rapid release pace (v0.39 in
~1 year). Breaking changes are expected and flagged. Closest to production-ready
for RAG workloads in Rust.

---

## 2. langchain-rust

- **Crate**: `langchain-rust`
- **Version**: 4.6.0
- **Last updated**: 2024-10-06 (8+ months stale as of 2026-06-28)
- **Created**: 2024-02-24
- **Downloads**: 145,509
- **Repository**: https://github.com/Abraxas-365/langchain-rust
- **Description**: "LangChain for Rust, the easiest way to write LLM-based programs in Rust"
- **Status**: Likely stalled. No release in 8+ months.

### RAG Support

Yes. Full RAG pipeline:
- Document loaders: PDF, HTML, Pandoc, CSV, source code, git commits
- Embeddings: OpenAI, Azure OpenAI, Ollama, FastEmbed (local), MistralAI
- Vector stores: see below
- Chains: Conversational Retriever Chain, Q&A Chain

### Vector Stores

| Backend | Feature flag | Type |
|---|---|---|
| Qdrant | `qdrant` | External server |
| PostgreSQL (pgvector) | `postgres` | External server |
| SQLite (sqlite-vss / sqlite-vec) | `sqlite` | Embedded |
| OpenSearch | `opensearch` | External server |
| SurrealDB | `surrealdb` | Embedded or external |

### Assessment

More feature-complete than rig for document-oriented RAG pipelines (document
loaders, chunking, full chain abstraction). However the project appears
abandoned as of late 2024.

---

## 3. chonkie (Rust)

- **Crate**: `chonkie`
- **Version**: 0.1.1
- **Created**: 2025-06-04
- **Last updated**: 2026-01-21
- **Downloads**: 729 (very new)
- **Repository**: not listed in crates.io metadata
- **Published by**: Bhavnick @ chonkie.ai (same team as Python chonkie)
- **Description**: "No-nonsense, ultra-fast, ultra-light chunking library"

### What it is

A text chunking library for splitting documents into chunks for RAG ingestion.
Not a full RAG framework. A port of the Python `chonkie` library.

### Features

- `tokenizers`: HuggingFace tokenizers backend
- `tiktoken`: OpenAI tiktoken backend
- `json`: JSON output support

### Assessment

Very early (0.1.1). Minimal adoption. Correct assessment: chonkie started as
Python but there is now a Rust port. Useful for the chunking step in RAG.

---

## 4. llm-chain

- **Crate**: `llm-chain`
- **Version**: 0.13.0
- **Last updated**: 2023-11-15 (2.5+ years stale)
- **Created**: 2023-03-25
- **Downloads**: 89,483
- **Repository**: https://github.com/sobelio/llm-chain/
- **Status**: Abandoned.

### Notes

Early LangChain-inspired Rust library. Had Qdrant as a default feature
(`qdrant = ["dep:qdrant-client"]`). No longer maintained.

---

## 5. candle-core (HuggingFace)

- **Crate**: `candle-core`
- **Version**: 0.11.0
- **Last updated**: 2026-06-26 (actively maintained)
- **Downloads**: 5,556,435
- **Repository**: https://github.com/huggingface/candle
- **Description**: "Minimalist ML framework."
- **Status**: Actively maintained by HuggingFace.

### What it is

A Rust ML inference framework (like PyTorch for Rust). NOT a RAG framework.
Used for running local HuggingFace models (LLMs, embedding models) in Rust.

Supports: CUDA, Metal, MKL, CPU. Used as the inference backend when you
want to run local models rather than API calls.

---

## 6. Vector Storage Approaches

### External services (separate server required)

| Crate | Version | Updated | Downloads | Notes |
|---|---|---|---|---|
| qdrant-client | 1.18.0 | 2026-05-11 | 2,883,077 | gRPC client only, no embedded mode |
| pgvector | 0.4.2 | 2026-05-22 | 14,134,557 | PostgreSQL extension client |

### Embedded / in-process options

| Crate | Version | Updated | Downloads | Notes |
|---|---|---|---|---|
| lancedb | 0.30.0 | 2026-05-28 | 583,132 | Serverless, file-based, no server |
| sqlite-vec | 0.1.9 | 2026-05-18 | 1,823,493 | SQLite extension FFI bindings |
| usearch | 2.25.3 | 2026-05-24 | 707,822 | Single-file in-process engine |

### fastembed (local embeddings)

- **Crate**: `fastembed`
- **Version**: 5.17.2
- **Updated**: 2026-06-15
- **Downloads**: 1,674,827
- **Repository**: https://github.com/Anush008/fastembed-rs
- **Description**: "Library for generating vector embeddings, reranking locally."

Runs HuggingFace embedding models locally via ONNX runtime. Used by both
`rig` and `langchain-rust` for local (no API call) embeddings.

---

## 7. Qdrant Embedded

**No embedded mode exists.** The `qdrant-client` crate is a pure network
client talking to a Qdrant server over gRPC. There is no `libqdrant` or
way to embed Qdrant in-process in Rust.

To use Qdrant locally you must run:
- `docker run -p 6334:6334 qdrant/qdrant`
- Or download the Qdrant binary and run it as a separate process

For true in-process vector search without a server, use:
- **LanceDB** (lancedb crate) — serverless, file-backed
- **sqlite-vec** — SQLite extension, in-process
- **usearch** — single-file in-process engine

---

## Patterns Observed

1. Qdrant is the most commonly integrated external vector store in Rust RAG
   projects (both rig and langchain-rust support it).
2. LanceDB is gaining traction as the preferred embedded option (rig has
   first-class support via rig-lancedb).
3. sqlite-vec is the preferred lightweight embedded option.
4. fastembed is the de facto standard for local (no API) embeddings in Rust.
5. The dominant pattern for production is: fastembed (local embeddings) +
   Qdrant or LanceDB + rig.
6. The Rust RAG ecosystem is 1-2 years behind the Python ecosystem in
   maturity, but rig is moving fast.
