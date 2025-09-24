# Plugin Streaming Guide

## Overview

The Unity plugin now keeps a shared `memmap2::Mmap` handle on `UnityArchive` so that
archive contents are streamed directly from disk instead of being repeatedly loaded
with `std::fs::read`. This refactor eliminates redundant allocations for large
bundles, keeps provenance hashing in place, and lays the groundwork for
parallel extraction pipelines across plugins.

## Implementation Highlights

- `UnityArchive::open` memory-maps the target bundle once and reuses that view for
  detection, provenance hashing, and metadata extraction.
- `extract_bundle_block` and `extract_serialized_object` operate on slices of the
  mapped file, only allocating when decompression or conversion requires it.
- A `Drop` implementation logs when the archive is closed so mapped resources are
  released promptly.

## Guidance for Plugin Authors

- Prefer reusing a `File` handle or `memmap2::Mmap` view instead of re-reading an
  archive into fresh buffers for each extraction step.
- Convert on-disk offsets to `usize` with `try_from` and guard slices with
  `checked_add` to prevent overflow when working with mapped byte slices.
- Decompression helpers should accept `&[u8]` so they can operate on borrowed
  slices rather than temporary `Vec<u8>` copies.
- Keep provenance hashing and compliance data generation backed by the shared
  reader so additional plugins can follow the same streaming pattern.

## Benchmark Summary

The repository includes `examples/streaming_benchmark.rs` to compare peak memory
between the legacy `std::fs::read` workflow and the new memory-mapped approach.
Running the benchmark with a 64 MiB synthetic bundle produced the following
results:

```
$ cargo run -p aegis-unity-plugin --example streaming_benchmark read
Mode: std::fs::read
File: /tmp/aegis_unity_streaming_bench_<pid>_67108864.bin (64 MiB)
Elapsed: 293.61ms
RSS delta: 65536 KiB
Peak delta: 65536 KiB
Final RSS: 67800 KiB
Peak RSS: 67800 KiB

$ cargo run -p aegis-unity-plugin --example streaming_benchmark mmap
Mode: memmap
File: /tmp/aegis_unity_streaming_bench_<pid>_67108864.bin (64 MiB)
Elapsed: 9.92µs
RSS delta: 0 KiB
Peak delta: 0 KiB
Final RSS: 2296 KiB
Peak RSS: 2296 KiB
```

The `<pid>` placeholder represents the process identifier inserted into the
temporary file name for each run.

The `std::fs::read` path spikes resident memory by roughly 64 MiB, while the
`memmap` path keeps both RSS and peak usage effectively flat. These numbers
confirm the streaming refactor reduces peak memory pressure and prepares the
Unity plugin for future parallel extraction work. Other plugins can adopt the
same pattern and use the benchmark to validate their implementations.
