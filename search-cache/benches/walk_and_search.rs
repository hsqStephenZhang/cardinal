//! Benchmarks for `SearchCache` walk and query performance.
//!
//! Set `BENCH_CORPUS_DIR` to the root of a large directory tree (e.g. a shallow
//! clone of the Linux kernel) to get meaningful numbers.  When the variable is
//! absent the benchmark falls back to `/opt/homebrew`, which is always present
//! on macOS GitHub Actions runners and contains thousands of files.
//!
//! Run:
//!   BENCH_CORPUS_DIR=/path/to/linux cargo bench -p search-cache

use criterion::{BatchSize, BenchmarkId, Criterion, SamplingMode, criterion_group, criterion_main};
use search_cache::SearchCache;
use search_cancel::CancellationToken;
use std::{path::PathBuf, time::Duration};

fn corpus() -> PathBuf {
    std::env::var("BENCH_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/opt/homebrew"))
}

// ---------------------------------------------------------------------------
// walk_fs — measures filesystem traversal + slab construction
// ---------------------------------------------------------------------------

fn bench_walk_fs(c: &mut Criterion) {
    let path = corpus();
    let mut group = c.benchmark_group("walk_fs");
    // Walking a large tree takes ~seconds; reduce sample count and extend
    // measurement time so Criterion doesn't time-out or flood the FS.
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));
    group.sampling_mode(SamplingMode::Flat);

    group.bench_function("corpus", |b| {
        b.iter(|| SearchCache::walk_fs(&path));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// query_files — measures search latency on an already-built cache
// ---------------------------------------------------------------------------

const QUERIES: &[&str] = &[
    // Exact file name
    "Makefile",
    // Extension glob
    "*.rs",
    // Multi-segment (path + name)
    "src main",
    // Common header name
    "*.h",
    // Term that should match very few results
    "ffffffff_no_match_xyzzy",
];

fn bench_query_files(c: &mut Criterion) {
    let path = corpus();
    let mut group = c.benchmark_group("query_files");
    group.measurement_time(Duration::from_secs(10));

    for query in QUERIES {
        group.bench_with_input(BenchmarkId::new("query", query), query, |b, q| {
            // Build a fresh cache once per *batch*, not per iteration, so we
            // measure only the search hot-path and not directory traversal.
            b.iter_batched(
                || SearchCache::walk_fs(&path),
                |mut cache| {
                    cache
                        .query_files(q.to_string(), CancellationToken::noop())
                        .ok()
                },
                BatchSize::PerIteration,
            );
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// query_files — warm cache repeated queries (amortises walk cost)
// ---------------------------------------------------------------------------

fn bench_query_files_warm(c: &mut Criterion) {
    let path = corpus();
    let mut cache = SearchCache::walk_fs(&path);

    let mut group = c.benchmark_group("query_files_warm");
    group.measurement_time(Duration::from_secs(10));

    for query in QUERIES {
        group.bench_with_input(BenchmarkId::new("query", query), query, |b, q| {
            b.iter(|| {
                cache
                    .query_files(q.to_string(), CancellationToken::noop())
                    .ok()
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_walk_fs,
    bench_query_files,
    bench_query_files_warm
);
criterion_main!(benches);
