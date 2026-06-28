# sqlite-vec Research Report

Researched: 2026-06-28
Sources: alexgarcia.xyz/sqlite-vec, github.com/asg017/sqlite-vec, docs.rs/sqlite-vec

---

## 1. What is sqlite-vec?

A vector search SQLite extension written in **pure C with zero dependencies**.
Successor to `sqlite-vss`. Mozilla Builders sponsored project.

- **Version:** v0.1.9 stable (2026-03-31), v0.1.10-alpha.4 pre-release
- **Status:** Pre-v1 — breaking changes expected
- **License:** MIT/Apache-2.0

### Search type

**Exact KNN only** (brute-force scan). The core `vec0` virtual table does NOT
use an approximate nearest-neighbour (ANN) index. The repo contains experimental
files (`sqlite-vec-ivf.c`, `sqlite-vec-diskann.c`, `sqlite-vec-ivf-kmeans.c`)
but these are not part of the stable API.

### Distance functions

| Function                  | Vectors        | Notes                          |
|---------------------------|----------------|--------------------------------|
| `vec_distance_L2(a, b)`   | float32, int8  | Default metric in vec0         |
| `vec_distance_cosine(a, b)`| float32, int8 | Set per-column at table create |
| `vec_distance_hamming(a, b)`| bit only     | For binary/bit vectors         |

No native dot-product distance. For normalized float32 vectors, cosine distance
is equivalent.

### Vector types

| Type    | Bytes/element | Range              |
|---------|---------------|--------------------|
| float32 | 4             | any float          |
| int8    | 1             | -128 to 127        |
| bit     | 1 per 8 elems | binary (0 or 1)    |

Dimensions: Any integer. Common examples: 768, 1024. No hard limit documented.

### vec0 virtual table features

- KNN query: `WHERE embedding MATCH :query AND k = 10`
- Distance metric specified at table creation: `float[768] distance_metric=cosine`
- **Metadata columns** (WHERE-filterable): TEXT, INTEGER, FLOAT, BOOLEAN; max 16
- **Partition key columns**: shards the index; max 4; ~100+ vectors per key needed
- **Auxiliary columns** (`+prefix`): unindexed, appear in SELECT without JOIN; max 16

### Compile-time SIMD options

- `SQLITE_VEC_ENABLE_AVX` - x86 AVX acceleration
- `SQLITE_VEC_ENABLE_NEON` - ARM NEON acceleration
- `SQLITE_VEC_STATIC` - for static linking

---

## 2. Rust Integration

### Crate

- **Name:** `sqlite-vec` on crates.io
- **Install:** `cargo add sqlite-vec`
- **Docs.rs version:** 0.1.9

### What the crate provides

A single FFI function — nothing else:

```rust
// lib.rs (verbatim)
#[link(name = "sqlite_vec0")]
extern "C" {
    pub fn sqlite3_vec_init();
}
```

0% of the crate is documented per docs.rs (it's a thin FFI wrapper).

### Cargo.toml (template)

```toml
[dependencies]
# none - zero runtime dependencies

[build-dependencies]
cc = "1.0"

[dev-dependencies]
rusqlite = "0.31.0"   # only for the crate's own tests
```

`rusqlite` is a **dev dependency only** in the crate itself. Your application
must separately depend on `rusqlite` (or another SQLite wrapper).

### How to wire it in your Rust app

```toml
# Your Cargo.toml
[dependencies]
sqlite-vec = "0.1.9"
rusqlite = { version = "0.32", features = ["bundled"] }
zerocopy = "0.7"  # recommended for zero-copy Vec<f32> -> &[u8]
```

```rust
use sqlite_vec::sqlite3_vec_init;
use rusqlite::{ffi::sqlite3_auto_extension, Connection, Result};
use zerocopy::AsBytes;

fn main() -> Result<()> {
    // Register extension before opening any connection
    unsafe {
        sqlite3_auto_extension(
            Some(std::mem::transmute(sqlite3_vec_init as *const ())),
        );
    }
    let db = Connection::open_in_memory()?;
    // Now all connections automatically have vec0 available
    Ok(())
}
```

### SQLite wrapper compatibility

- **rusqlite**: officially supported and tested
- **sqlx**: no official support; would require loading via raw SQLite API
- **No own high-level Rust API**: the crate is purely an FFI bridge; all
  interaction is via SQL strings

---

## 3. Build / Packaging

### C compilation

YES — `build.rs` compiles C at build time:

```rust
// bindings/rust/build.rs (verbatim)
fn main() {
    cc::Build::new()
        .file("sqlite-vec.c")
        .define("SQLITE_CORE", None)
        .compile("sqlite_vec0");
}
```

- Uses **`cc` crate** — not cmake, not pkg-config
- `SQLITE_CORE` define means it integrates as part of SQLite core (statically
  linked), not as a dynamically loaded extension
- `sqlite-vec.c` (the amalgamation) is **bundled inside the crate** — no
  network access needed during build

### External system libraries

**None required.** The extension is pure C with no external deps. With
`rusqlite` using `features = ["bundled"]`, SQLite itself is also compiled
from source — no system libsqlite3 needed.

### Pure cargo dependency

**Yes**, effectively. `cargo build` handles everything without system library
installation, as long as a C compiler is available in PATH.

### Nix flake implications

1. **C compiler required in build environment.** The `cc` crate uses `$CC`
   or detects gcc/clang from PATH. Add to `buildInputs`:
   - `pkgs.gcc` or `pkgs.clang`
   - Or via `pkgs.rustPlatform` which handles this automatically

2. **No system sqlite needed** when using `rusqlite` with `features = ["bundled"]`.
   If not using `bundled`, add `pkgs.sqlite` to `buildInputs`.

3. **Network-free build**: `sqlite-vec.c` is bundled in the crate tarball.
   Works with Nix's restricted-network sandbox.

4. **`rustPlatform.buildRustPackage`**: compatible without special overrides
   beyond ensuring a C compiler is present.

5. **No cmake**: `cc` crate only needs a C compiler, not a build system.

---

## 4. Limitations

### Exact search only (core)
The `vec0` virtual table is brute-force. No HNSW, no IVF in stable API.
Experimental IVF/DiskANN files exist in the repo but are not packaged in
the crate or stable release.

### No persistence of approximate index
Because there is no ANN index in `vec0`, there is nothing to "persist" — every
query scans all stored vectors. The vectors themselves are persisted in the
SQLite database file via the virtual table.

### Performance at ~5000 vectors, 768 dimensions
- Raw data volume: 5000 × 768 × 4 bytes ≈ 14.6 MB (float32)
- At this scale, brute-force is fast — sub-millisecond to low-millisecond
  range with AVX on modern hardware
- No published official benchmarks for this exact configuration; the
  performance guide page is an unfilled stub
- SIMD must be opted in at compile time (`SQLITE_VEC_ENABLE_AVX`); the
  `cc` crate build in the Rust crate does NOT enable these by default

### Pre-v1 API instability
- Breaking changes in minor/patch versions
- Documentation is self-described as "work-in-progress"

### Distance metric is fixed at table creation
`distance_metric=cosine` is set in the CREATE TABLE statement, not per-query.
Cannot mix distance metrics in a single KNN query.

### Metadata column type restrictions
Only TEXT, INTEGER, FLOAT, BOOLEAN — no BLOB. Only basic comparison operators
(=, !=, <, <=, >, >=) in KNN WHERE clause. No LIKE, IS NULL, functions.

### No dot product distance function
`vec_distance_cosine` and `vec_distance_L2` only. For normalized vectors,
cosine distance is equivalent to negative dot product similarity.

---

## 5. Project Status

| Metric         | Value                                           |
|----------------|-------------------------------------------------|
| Maintainer     | Alex Garcia (asg017, alexgarcia.xyz)            |
| GitHub stars   | 7.8k                                            |
| Forks          | 328                                             |
| Total releases | 88                                              |
| Latest stable  | v0.1.9 (2026-03-31) — DELETE bug fix           |
| Latest pre-rel | v0.1.10-alpha.4                                 |
| Open issues    | 154                                             |
| Open PRs       | 46                                              |
| Sponsors       | Mozilla, Fly.io, Turso, SQLite Cloud, Shinkai   |
| Activity       | Actively maintained                             |
