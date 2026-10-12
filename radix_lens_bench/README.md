# radix_lens_bench — performance suite

> **My Radix LENS v1.3.0 (Packed Loser Tree Engine).** A performance-engineering
> benchmark for the recognition/radix idea at scale: in-memory sorts, and a full
> **external + parallel** sorting pipeline (block sort → multi-way merge) with a
> **loser-tree** merge, disk I/O, and honest correctness checks.

This is a **separate subproject** of `recognition_sort`. Unlike the pedagogical
demos in the parent repo (pure `std`, no crates), this suite **uses external
crates** (`rayon`, `clap`, `serde`, `serde_json`, `rand`) and measures real
engineering tradeoffs. Keep the two mentally separate: the demos explain the
idea; this suite stress-tests it.

> **Full optimization report:** [`IZVESTAJ.md`](./IZVESTAJ.md) — the authoritative
> write-up (2821 ms → 720 ms ≈ 3.9× external; bugs fixed; 9 falsified hypotheses;
> environment diagnostics; recommended config; honest advantages/limits). The
> summary below is a short version of it.

---

## What it does

- **In-memory comparison**: `radix-lens-radix` (parallel LSD radix, 4×8 bits, no
  comparisons) vs `std::sort`, `std::sort_unstable`, `rayon par_sort`,
  `rayon par_sort_unstable`.
- **External sorting pipeline** (data does not fit in RAM; writes/reads files):
  generate sorted blocks → multi-way merge. Built as staged optimizations.
- Options (clap): `--scenario {ram,streaming,external,parallel}`,
  `--elements`, `--block-size-mb`, `--read-buffer-kb`, `--write-buffer-kb`,
  `--target-dir`, `--threads`, `--repetitions`, `--distribution {random,sorted,
  reverse,duplicates}`, `--rng {chacha,xoshiro}`, `--seed`, `--strategy
  {standard-heap,optimized-heap,loser-tree,packed-loser-tree}`, `--pipeline`,
  `--pipeline-depth`, `--merge-threads`, `--block-sort {std,radix}`,
  `--writers`, `--compare-std`, `--label`.
- **Merge strategies**: StandardHeap, OptimizedHeap, LoserTree, and
  **PackedLoserTree** (key = `(value as u64) << 32 | run_index`; one `u64`
  compare gives the same order as `(value, run)` — no double indexing, no
  `Option` branching). PackedLoserTree is the recommended/default merge.
- **Input fingerprint**: an order-independent multiset hash (wrapping sum of
  per-value hashes). With `--seed 42 --rng chacha` it is `5401f572beb93e6a`.
  **Results are only comparable across runs while this fingerprint holds.**

## Build & run (Windows / PowerShell used by the author)

```powershell
cargo build --release

# Full comparison (in-memory + external phases f1..f6), writes logs + summary CSV:
powershell -ExecutionPolicy Bypass -File .\run_compare.ps1
# In-memory only:
powershell -ExecutionPolicy Bypass -File .\run_compare.ps1 -OnlyMem

# Or run the binary directly, e.g. the recommended external config:
.\target\release\radix_lens_bench.exe --elements 100000000 --block-size-mb 32 ^
  --read-buffer-kb 2048 --write-buffer-kb 4096 --seed 42 ^
  --scenario external --target-dir C:\temp_sort_pipe1 --repetitions 3 ^
  --threads 6 --strategy packed-loser-tree --pipeline --pipeline-depth 0 ^
  --merge-threads 8 --block-sort radix --writers 2
```

`run_disks.ps1` runs the external pipeline across different drives
(nvme/ssd/hdd, fsync/nofsync) — see `results/` for those raw logs.

---

## Results (author's machine; see `results/` for raw CSV/logs)

Machine-specific numbers; read the ratios and shape, not the absolutes.
`u32` values, 100,000,000 elements, `--seed 42 --rng chacha`
(fingerprint `5401f572beb93e6a`). All runs reported `PROLAZ` (correct).

### In-memory (median of 5)

| method | median ms | mem MiB |
|---|---|---|
| radix-lens-radix (parallel, no comparison) | **372** | 381 |
| radix-lens-radix-1nit (single thread) | 788 | 381 |
| rayon par_sort_unstable | 468 | 0 |
| rayon par_sort | 923 | 383 |
| std::sort_unstable | 1731 | 0 |
| std::sort | 2613 | 191 |

Parallel radix is fastest in memory (~372 ms), ahead of even
`rayon par_sort_unstable` (~468 ms) — at a memory cost (~381 MiB; the
comparison-based unstable sorts are in-place, ~0 extra).

### External pipeline (staged optimizations, 100M elements, block 32 MiB)

| phase | config | total ms | prep ms | merge ms | mem MiB |
|---|---|---|---|---|---|
| f1 | OptimizedHeap, sequential prep, std sort | 2821 | 1202 | 1584 | 38 |
| f2 | + pipeline depth 0 | 2181 | 552 | 1608 | 102 |
| f3 | + PackedLoserTree (sequential merge) | 1571 | 655 | 895 | 102 |
| f4 | + parallel merge, 8 threads | 874 | 641 | 206 | 102 |
| f5 | + radix block sort, 2 writer threads | 772 | 520 | 224 | 132 |
| f6 | + 6 rayon threads (recommended) | 990* | 737 | 215 | 166 |

*f6 median 990 ms (min 702, max 1310); the recommended config typically lands
around ~720 ms on the author's machine. External sorting works with ~150 MiB
resident while writing/reading files; the in-memory runs hold the whole array.

---

## Honest notes

- **Not pure std.** This suite depends on crates (`rayon` etc.); the parent
  repo's demos are pure std. Different purpose, different rules.
- **Hardware/disk specific.** External numbers depend on the drive (nvme/ssd/hdd)
  and fsync policy; `results/` keeps the raw per-drive logs so nothing is hidden.
- **Comparability gate.** If the input fingerprint changes from
  `5401f572beb93e6a`, the data changed and the report numbers are no longer
  comparable. Re-verify after any change to generation/seed/RNG.
- **Correctness.** Every run checks the output and reports `PROLAZ`/fail; the
  test suite (`cargo test --release`) covers the merge structures.

## Provenance / acknowledgments

- **Concept & direction:** Srećko Jovančević.
- **Benchmark code:** primarily **Gemini**; **Claude (Anthropic)** did revisions
  and fixes.
- **Test design (what to measure / which scenarios):** **Claude** and **ChatGPT**.

Stated honestly so credit is accurate: this suite is a multi-tool collaboration,
not the work of a single assistant.

**Status:** performance suite, work in progress, shown as-is.
