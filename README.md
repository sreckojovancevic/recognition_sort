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
   external crates), and correctness (checked against `std::sort`) across
   scale — read as *behavior*, not as a leaderboard.

> The point is not "a new sort" and not "a faster sort in general." The point
> is the unifying principle and the honest behavioral picture.

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

## Limits (stated plainly)

- **Specialized, not universal.** Works on what decomposes into a bounded
  number of digits/bytes (integers, fixed keys). `std::sort` works on anything
  comparable (variable-length strings, floats, custom orders). This is not
  "radix beats sort" — it is "for bounded-range integers, non-comparison wins,
  at a memory cost."
- **Direct addressing has a space wall.** `value - min` placement needs an
  array spanning the whole value range (97 slots for 4 numbers in `[3..99]`;
  a billion for `[3, 1e9]`). Radix is precisely the fix (space = base + n).

---

## Files

| File | What it shows |
|---|---|
| `samosvesni_broj.rs` | the self-locating number: position = value − origin, O(n), 0 comparisons |
| `rang_broj.rs` | dual identity (value + rank); when rank is redundant vs informative |
| `sort_apsolutni.rs` | arbitrary integers, 0 comparisons; the space cost |
| `rang_radix_1m.rs` | radix base 10, 1M elements |
| `rang_radix256_1m.rs` | radix base 256 (byte by byte), 1M elements |
| `benchmark_full.rs` | time + measured peak memory + correctness, 100 → 100M |
| `pozicioni_broj.rs` | rational position key: insertion between two without re-ranking (order maintenance) |
| `KONCEPT.md` | the governing concept — stands above any single application |
| `MERENJA.md` | full measurements and behavior reading (detailed) |

## Build & run

A Cargo project with one binary per file, **no external dependencies**.
Source files live in `src/bin/`.

```bash
cargo run --release --bin benchmark_full
```

Other binaries: `samosvesni_broj`, `rang_broj`, `sort_apsolutni`,
`rang_radix_1m`, `rang_radix256_1m`, `pozicioni_broj`. **Always use `--release`**
— debug numbers mislead.

(100M `u32` needs ~1.6 GB peak — trim the large sizes in `sizes` if RAM
is tight.)

Prefer plain `rustc`? Each file is standalone:
`rustc -O src/bin/benchmark_full.rs && ./benchmark_full`.

---

## License

MIT. If this contributes to your work, a citation is welcome.

**Status:** study and benchmark, work in progress, shown as-is.
