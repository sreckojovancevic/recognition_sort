# Recognition Sort

> Organizing numbers by **recognition instead of comparison** — a study of
> non-comparison (distribution) sorting through the Recognition Organization
> lens, with honest, reproducible benchmarks.

**Conceptual parent:** [recognition-organization-proposition](https://github.com/sreckojovancevic/recognition-organization-proposition)
**Governing concept:** [`KONCEPT.md`](./KONCEPT.md) — the idea stands above any single
application; sorting and the benchmark are *one* application, not the boundary.

---

## What this is (and what it is not)

Radix and counting sorts are **classic, well-known non-comparison sorts** —
they belong to the "distribution sort" family and have existed for over a
century. This repository does **not** claim to invent them.

What it offers:

1. **A lens.** Each number places itself by its *own identity* — its value
   determines its position, no pairwise `a > b` is ever evaluated. A number
   "knows where it belongs." This is the same recognition principle used for
   strings in the parent project (recognize each byte/char, place it): the
   digit of a number and the character of a string are the same thing.
2. **Honest measurement.** A reproducible benchmark of time, **actually
   measured** peak memory (a counting global allocator, pure `std`, no
   external crates — see [`ALLOCATOR.md`](./ALLOCATOR.md)), and correctness
   (checked against `std::sort`) across scale — read as *behavior*, not as a
   leaderboard.

> The point is not "a new sort" and not "a faster sort in general." The point
> is the unifying principle and the honest behavioral picture.

---

## Quick look

A trimmed run of [`verbose_demo.rs`](./src/bin/verbose_demo.rs) — each number
recognizes its own place, with **zero comparisons between numbers**, negatives
and duplicates included (output in Serbian; `koren` = origin/min,
`mesto` = slot):

```text
$ cargo run --release --bin verbose_demo -- -5 42 -12 7 0 7
Ulaz (razbacano): [-5, 42, -12, 7, 0, 7]
Koren (offset) = min = -12

Broj  -5 ... -5 - -12 = 7    ->  mesto (indeks) 7
Broj  42 ... 42 - -12 = 54   ->  mesto (indeks) 54
Broj -12 ... -12 - -12 = 0   ->  mesto (indeks) 0
...
Organizovano: [-12, -5, 0, 7, 7, 42]

Poređenja IZMEĐU brojeva:         (comparisons between numbers)
   prepoznavanje : 0              (recognition — each number found its own place)
   insertion sort: 10            (a comparison sort, for contrast)

Isti skup, obrnut redosled dolaska -> isti rezultat
(same set, reversed arrival order -> same result: position comes from identity, not order)
```

Run it with your own numbers (negatives welcome): `... -- 3 -8 3 100 -8 0`.

---

## The recognition view

A number is treated as a **self-locating entity**:

```
position(n) = recognize(n)        // derived from the number itself
```

- **Dense / consecutive integers:** position is a *formula*, `value - origin`.
  No lookup table needed — the number is self-locating. (counting / direct
  addressing)
- **Arbitrary integers:** decompose into digits/bytes and recognize them one
  place at a time. Space stays bounded (base + n), independent of value range.
  (radix)

No pairwise comparison decides order in either case. This is verified at
runtime: the sort is checked against `std::sort` on every input, including
1,000,000+ element stress tests.

See [`verbose_demo.rs`](./src/bin/verbose_demo.rs) for a step-by-step walk on a
tiny array — it prints how each number recognizes its own place, making the
mechanism visible rather than abstract. For the idea in words — a number whose
identity is *value + position* (`n@3 = 2 < 3 < 4`) — see
[`RANGBROJ.md`](./RANGBROJ.md).

---

## Relation to the parent project

This is not a trick that only works on integers. The same recognition
principle drives the string engine in the parent project,
[recognition-organization-proposition](https://github.com/sreckojovancevic/recognition-organization-proposition):
there, each string recognizes itself **character by character** (MSD) and
places itself with no comparison. Here, each number recognizes itself **digit
by digit / byte by byte** (radix). **A digit of a number and a character of a
string are the same thing** to the mechanism — an entity reading one place of
its own identity and acting on it. Numbers and strings are two instances of one
idea: organization by self-recognition. The parent repo is where that broader,
non-numeric picture lives.

---

## Where the acceleration comes from (measured, with its limits)

For bounded-range integers, the comparison-free distribution approach can be
materially faster than an optimized comparison sort — and **base size is
decisive**, because it sets the number of passes:

| N | radix (base 256) | `std::sort` | ratio | peak mem (measured) |
|---|---|---|---|---|
| 100 | 5.0 µs | 1.8 µs | **0.36× (std wins)** | ~0 |
| 10,000 | 101 µs | 132 µs | 1.30× | 0.1 MB |
| 20,000,000 | 295 ms | 502 ms | 1.70× | 229 MB |
| 50,000,000 | 680 ms | 1.3 s | 1.91× | 572 MB |
| 70,000,000 | 748 ms | 1.9 s | 2.48× | 801 MB |
| 100,000,000 | 1.1 s | 2.7 s | 2.47× | 1144 MB |

*(Author's machine, `rustc -O`, `u32` values in range 0..100M. Machine-specific;
read the ratios and shape, not the absolutes.)*

**Behavior, read honestly:**

- **Crossover.** At tiny `N` the comparison sort wins — radix's per-pass and
  buffer overhead dominate when there is almost nothing to sort. The advantage
  appears only past a crossover (here around 10⁴).
- **Base matters.** Base 256 (4 passes) beats `std::sort` at 10M; base 10
  (8 passes) **loses** on the same data (473 ms vs 294 ms). More passes = more
  memory traffic = a good comparison sort catches up. Fewer passes win.
- **Growth, then plateau.** The advantage grows with `N` (O(n) vs O(n·log n))
  and settles near ~2.5× at 70–100M — most likely memory-bandwidth bound.
- **The price is memory.** Peak radix memory is ≈ **3× input** and grows
  linearly; `std::sort` is in-place (~1×). Speed is bought with space.

**Always measure with `-O`.** In debug mode `std::sort` is disproportionately
slow, which inflated an early measurement to 7.7×. The honest optimized figure
is ~1.5–2.5× depending on scale.

---

## Beyond sorting: order maintenance (`pozicioni_broj.rs`)

Sorting is one application of the recognition principle, not its boundary.
When position *cannot* be derived from the value — sparse, arbitrary elements —
it is carried as **stored context**: a rational **position key** (a string
treated as the fraction `0.<digits>`). Between any two keys there is always a
third (rationals are dense), so an element can be inserted **between** two
others by computing a key between theirs — **without re-ranking anyone else**.

This gives what radix/counting do *not*: **order maintenance under insertion**.
The cost is that repeated insertion into the same gap lengthens the key (it
grows in length instead of hitting a precision wall, as `f64` would). The
element gains a *second identity* — where it stands — without changing its value.
This is the "stored relation" branch described in [`KONCEPT.md`](./KONCEPT.md),
and it is a different capability, not a faster sort.

---

## Streaming / pipeline (`pipeline_demo.rs`)

The same principle fits a **data stream** as a two-stage pipeline (assembly line):

```
source (stream) -> [Stage 1: run generation] -> [Stage 2: k-way merge] -> output
                     block -> local direct         merge run heads into
                     addressing (NO comparison)     a global order (COMPARES)
```

Stage 1 sorts each incoming block with local direct addressing — **zero
comparisons within a block**. Stage 2 merges the sorted runs into a globally
ordered stream. Honest split: the per-block work is comparison-free, but
**gluing blocks into a global order requires comparison at the merge** — for
arbitrary values there is no way around it (a comparison-free global result
would need the whole range in memory, or a consecutive-integer stream with
emit-on-complete).

Two builds of the same structure (identical output, different execution):

```bash
cargo run --release --bin pipeline_demo -- --sekv      # stages one after another
cargo run --release --bin pipeline_demo -- --paralel   # stages as threads + channel (mpsc)
```

The parallel build runs Stage 1 and Stage 2 on separate threads connected by a
channel — a real pipeline. For sorting the gain is structural (stages overlap),
not a large speedup, since the merge must see all runs; the point is the shape.

---

## Performance suite (`radix_lens_bench/`)

A separate subproject, [`radix_lens_bench/`](./radix_lens_bench), stress-tests
the idea at 100M+ scale. It is **not pure std** — it uses `rayon`, `clap`,
`serde`, `rand` — so keep it mentally apart from the demos above: the demos
explain the idea, the suite engineers it. It does in-memory parallel radix and
a full **external + parallel** sort (block sort → multi-way merge) with a
**packed loser-tree** merge and disk I/O.

Headline measured results (author's machine, 100M `u32`, seed 42, input
fingerprint `5401f572beb93e6a`; full write-up in
[`radix_lens_bench/IZVESTAJ.md`](./radix_lens_bench/IZVESTAJ.md)):

- **In-memory, 1 thread:** radix **2.29×** faster than `std::sort_unstable`
  (788 vs 1802 ms) and **3.36×** than `std::sort` (vs 2645 ms). This extends the
  `recognition_sort` finding (which compared only to stable `std::sort`, ~2.47×)
  to the *stronger* opponent `sort_unstable`.
- **In-memory, 8 threads:** **1.26–1.59×** faster than `rayon par_sort_unstable`,
  ~2.6× than `rayon par_sort`.
- **External pipeline:** optimized from **2821 ms → ~720 ms** (≈3.9×) at ~146 MiB
  resident, writing/reading files — correct on bit-identical input every run.
- **Cost / limits (measured):** auxiliary buffer ≈ input size (~381 MiB in
  memory); scales to threads worse than comparison sorts; loses below N≈10,000;
  integer fixed-width keys only. See `IZVESTAJ.md` for the honest full ledger.

**Provenance (stated honestly):** concept & direction — Srećko Jovančević;
benchmark code — primarily **Gemini**, with revisions/fixes by **Claude**; test
design (what to measure) — **Claude** and **ChatGPT**.

---

## Limits (stated plainly)

- **Specialized, not universal.** Works on what decomposes into a bounded
  number of digits/bytes (integers, fixed keys). `std::sort` works on anything
  comparable (variable-length strings, floats, custom orders). This is not
  "radix beats sort" — it is "for bounded-range integers, non-comparison wins,
  at a memory cost."
- **Direct addressing has a space wall.** `value - min` placement needs an
  array spanning the whole value range (97 slots for 4 numbers in `[3..99]`;
  a billion for `[3, 1e9]`). Radix is precisely the fix (space = base + n).
- **Negatives and wider types (`u64`/`i64`) — how it would extend, not yet
  implemented.** The current code uses `u32`, which is enough to demonstrate
  the concept and measure performance. The principle carries over cleanly in
  theory: negatives are handled by the same **offset by `min`** already used in
  `sort_apsolutni.rs` (`value - min` makes every key non-negative, so the sign
  disappears before placement); wider types just mean **more passes** (an
  `i64`/`u64` key is up to 8 base-256 bytes instead of 4). Nothing about the
  mechanism changes — only the pass count and the offset step. This note states
  the path honestly; it does not promise an implementation that isn't here yet.
  One nice consequence of the offset model: **zero gets no special status** — it
  is just a position the sequence passes through (`n@0 = -1 < 0 < 1`), a passage
  not a wall (`verbose_demo` shows `meta(0) = (-1, 0, 1)`; see
  [`RANGBROJ.md`](./RANGBROJ.md)).

---

## Files

| File | What it shows |
|---|---|
| `verbose_demo.rs` | step-by-step on a tiny array: each number recognizes its own place (the mechanism made visible) |
| `samosvesni_broj.rs` | the self-locating number: position = value − origin, O(n), 0 comparisons |
| `rang_broj.rs` | dual identity (value + rank); when rank is redundant vs informative |
| `sort_apsolutni.rs` | arbitrary integers, 0 comparisons; the space cost |
| `rang_radix_1m.rs` | radix base 10, 1M elements |
| `rang_radix256_1m.rs` | radix base 256 (byte by byte), 1M elements |
| `benchmark_full.rs` | time + measured peak memory + correctness, 100 → 100M |
| `bench_vs_std.rs` | radix256 vs `std::sort` vs `std::sort_unstable`: time, speedup, memory, correctness (100 → 1B with `--huge`) |
| `pozicioni_broj.rs` | rational position key: insertion between two without re-ranking (order maintenance) |
| `pipeline_demo.rs` | two-stage streaming pipeline: comparison-free run generation + k-way merge (`--sekv`/`--paralel`) |
| `KONCEPT.md` | the governing concept — stands above any single application |
| `RANGBROJ.md` | the RangBroj (RankNumber) idea: a number whose identity is value + position (`n@3 = 2 < 3 < 4`) |
| `MERENJA.md` | full measurements and behavior reading (detailed) |
| `ALLOCATOR.md` | how the real peak-memory measurement works (reusable `std`-only technique) |
| `radix_lens_bench/` | performance suite (separate crate-using subproject): in-memory parallel radix + external/parallel sort with packed loser-tree merge; full report in `IZVESTAJ.md` |

## Build & run

A Cargo project with one binary per file, **no external dependencies**.
Source files live in `src/bin/`.

```bash
cargo run --release --bin benchmark_full
```

Other binaries: `verbose_demo`, `samosvesni_broj`, `rang_broj`, `sort_apsolutni`,
`rang_radix_1m`, `rang_radix256_1m`, `pozicioni_broj`, `pipeline_demo`,
`bench_vs_std`. **Always use `--release`** — debug numbers mislead.
(`verbose_demo` is the friendliest starting point.)

(100M `u32` needs ~1.6 GB peak — trim the large sizes in `sizes` if RAM
is tight.)

Prefer plain `rustc`? Each file is standalone:
`rustc -O src/bin/benchmark_full.rs && ./benchmark_full`.

---

## Acknowledgments

Concept, intuition, and direction by **Srećko Jovančević** — the idea of the
self-aware number (an entity that recognizes its own place), the framing, and
the honesty requirements are his.

Implementation, verification (the runtime checks against `std::sort`, the
fuzz/stress tests), the measurement harness, and the documentation were
developed in collaboration with **Claude (Anthropic)**, working from that
direction. Every claim here was run and measured, not asserted — the role of
the assistant was to build, test, and keep the framing honest, not to originate
the idea.

## License

MIT. If this contributes to your work, a citation is welcome.

**Status:** study and benchmark, work in progress, shown as-is.

> If this serves you, use it. If not, it's measured honestly so you can decide —
> and it's here to explore. The point was never to win, only to be measurable.
