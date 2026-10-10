# Measuring real peak memory with a counting global allocator

The benchmarks in this repo do **not** estimate memory — they measure the
actual peak heap allocation, in pure `std` Rust, with **no external crates**.
The technique is small, reusable, and useful well beyond this project, so it is
documented here on its own.

## The idea

Rust lets you replace the global allocator with your own type that implements
[`GlobalAlloc`](https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html).
By wrapping the system allocator and counting every allocation and
deallocation, you get the exact number of bytes the program holds on the heap
at any moment — and, by tracking the maximum, the **peak**.

## The code

```rust
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

static CUR:  AtomicUsize = AtomicUsize::new(0);   // bytes currently on the heap
static PEAK: AtomicUsize = AtomicUsize::new(0);   // max of CUR ever seen

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            let c = CUR.fetch_add(l.size(), SeqCst) + l.size();
            // raise PEAK to c if c is larger (lock-free)
            let mut pk = PEAK.load(SeqCst);
            while c > pk {
                match PEAK.compare_exchange_weak(pk, c, SeqCst, SeqCst) {
                    Ok(_)  => break,
                    Err(x) => pk = x,
                }
            }
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        CUR.fetch_sub(l.size(), SeqCst);
        System.dealloc(p, l);
    }
}

#[global_allocator]
static A: Counting = Counting;

fn cur()        -> usize { CUR.load(SeqCst) }
fn peak()       -> usize { PEAK.load(SeqCst) }
fn reset_peak()          { PEAK.store(CUR.load(SeqCst), SeqCst); }
```

## How to use it

To measure the peak **working set of one phase** (not the whole program),
snapshot the baseline, reset the peak, run the phase, then subtract:

```rust
let base = cur();          // memory already live (e.g. the input)
reset_peak();              // peak now starts from the baseline
let out = some_algorithm(&input);
let phase_peak = peak() - base;   // extra bytes the algorithm needed at its worst
```

That is exactly how this repo reports "radix mem" separately from the input
and from `std::sort`'s own working set.

## Why it works (and its limits)

- **Every heap allocation goes through the global allocator**, including all
  `Vec`, `String`, `Box`, and container growth. `Vec` growth uses `realloc`,
  and the default `GlobalAlloc::realloc` is implemented in terms of
  `alloc` + `dealloc`, so it is counted too.
- **Thread-safe.** The counters are atomics, so the allocator stays correct
  under concurrency. `SeqCst` is simple and safe here; `Relaxed` would be
  faster if you profile and decide you need it.
- **Heap only.** It measures heap bytes requested through the allocator. It
  does **not** capture stack usage, memory-mapped regions, or allocator
  internal fragmentation/overhead. For array-heavy numeric code (like sorting)
  the heap figure is the dominant and relevant one; for other workloads, know
  what you are and aren't seeing.
- **Bytes requested, not RSS.** This is what the program asked for, which is
  usually what you want to reason about. It can differ from the OS resident
  set size (RSS), which includes pages the OS has actually backed.

## When to reach for this

Any time you want to answer "how much memory did *this* actually use?" without
pulling in a profiler or an external crate — benchmarks, regression tests on
memory, comparing two implementations' footprints, or teaching. It is a few
lines, it is `std`-only, and it is honest: a measured number, not a guess.
