// ============================================================================
// bench_vs_std.rs  —  radix256 vs std::sort vs std::sort_unstable
// ============================================================================
//
// Pošteno: Rust std NEMA radix sort. "Radix strana" je naš radix256; poredi
// se sa std::sort (stabilan, Timsort) i std::sort_unstable (pdqsort, brz).
// Meri: vreme, ubrzanje (std / radix), stvarnu vrsnu memoriju (broječi
// alokator, pure std), i korektnost (== std sort).
//
// Vrednosti su u32, opseg do 100M. Veličine do 100M rade pouzdano; 1 milijarda
// je iza --huge (traži ~16 GB RAM-a) da ne ubije mašinu slučajno.
//
// Run (OBAVEZNO -O):
//     rustc -O bench_vs_std.rs && ./bench_vs_std
//     rustc -O bench_vs_std.rs && ./bench_vs_std --huge      # uključi 1 milijardu
// ============================================================================

use std::alloc::{GlobalAlloc, Layout, System};
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::time::Instant;

// --- broječi alokator: stvarna vrsna memorija ------------------------------
static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            let c = CUR.fetch_add(l.size(), SeqCst) + l.size();
            let mut pk = PEAK.load(SeqCst);
            while c > pk {
                match PEAK.compare_exchange_weak(pk, c, SeqCst, SeqCst) {
                    Ok(_) => break,
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

fn cur() -> usize { CUR.load(SeqCst) }
fn peak() -> usize { PEAK.load(SeqCst) }
fn reset_peak() { PEAK.store(CUR.load(SeqCst), SeqCst); }
fn gb(bytes: usize) -> f64 { bytes as f64 / 1_073_741_824.0 }

// --- radix256 za u32 (bajt po bajt), bez poređenja -------------------------
fn radix256(vals: &[u32]) -> Vec<u32> {
    if vals.is_empty() { return vec![]; }
    let mut mn = vals[0];
    let mut mx = vals[0];
    for &v in &vals[1..] {
        if v < mn { mn = v; }
        if v > mx { mx = v; }
    }
    let mut tek: Vec<u32> = vals.iter().map(|&v| v - mn).collect();
    let maxr = mx - mn;
    let mut buf = vec![0u32; tek.len()];
    let mut shift: u32 = 0;
    while shift < 32 && (maxr >> shift) > 0 {
        let mut count = [0usize; 256];
        for &v in &tek { count[((v >> shift) & 0xFF) as usize] += 1; }
        let mut sum = 0usize;
        for c in count.iter_mut() { let t = *c; *c = sum; sum += t; }
        for &v in &tek {
            let d = ((v >> shift) & 0xFF) as usize;
            buf[count[d]] = v;
            count[d] += 1;
        }
        std::mem::swap(&mut tek, &mut buf);
        shift += 8;
    }
    tek.iter().map(|&v| v + mn).collect()
}

fn ms(d: std::time::Duration) -> f64 { d.as_secs_f64() * 1000.0 }

fn main() {
    let huge = env::args().any(|a| a == "--huge");

    let sve_velicine: [usize; 6] =
        [100, 1_000, 10_000, 10_000_000, 100_000_000, 1_000_000_000];

    println!("radix256 vs std::sort vs std::sort_unstable  (u32, opseg do 100M, -O)\n");
    println!("{:>13} | {:>9} | {:>10} | {:>12} | {:>8} | {:>8} | {:>9} | {}",
             "N", "radix256", "std::sort", "sort_unstable",
             "r/sort", "r/unst", "radix mem", "ok");
    println!("{}", "-".repeat(100));

    let mut state: u64 = 0x1234_5678_9ABC_DEF0;
    let mut rnd = || { state ^= state << 13; state ^= state >> 7; state ^= state << 17; state };

    for &n in &sve_velicine {
        // Procena memorije: ulaz + radix(tek+buf) + jedan klon ~ n*4*4 bajta.
        let est_gb = (n as f64 * 4.0 * 4.0) / 1_073_741_824.0;
        if est_gb > 8.0 && !huge {
            println!("{:>13} | preskočeno — treba ~{:.1} GB RAM-a; pokreni sa --huge",
                     n, est_gb);
            continue;
        }

        let vals: Vec<u32> = (0..n).map(|_| (rnd() % 100_000_000) as u32).collect();

        // radix256 + izmerena memorija
        let base = cur();
        reset_peak();
        let t0 = Instant::now();
        let out = radix256(&vals);
        let t_radix = t0.elapsed();
        let radix_mem = gb(peak().saturating_sub(base));

        // std::sort (stabilan)
        let mut a = vals.clone();
        let t1 = Instant::now();
        a.sort();
        let t_sort = t1.elapsed();

        // std::sort_unstable (pdqsort)
        let mut b = vals.clone();
        let t2 = Instant::now();
        b.sort_unstable();
        let t_unst = t2.elapsed();

        let ok = out == a && out == b;

        println!("{:>13} | {:>7.1}ms | {:>8.1}ms | {:>10.1}ms | {:>7.2}x | {:>7.2}x | {:>6.2} GB | {}",
                 n, ms(t_radix), ms(t_sort), ms(t_unst),
                 t_sort.as_secs_f64() / t_radix.as_secs_f64(),
                 t_unst.as_secs_f64() / t_radix.as_secs_f64(),
                 radix_mem,
                 if ok { "da" } else { "NE!" });
        assert!(ok, "radix i std se razlikuju na N={}", n);
    }

    println!("\nr/sort, r/unst = ubrzanje (std vreme / radix vreme); > 1 znači radix brži.");
    println!("Napomena: std nema radix; 'radix strana' je naš radix256. Meri se sa -O.");
    if !huge {
        println!("1 milijarda je preskočena — dodaj --huge (traži ~16 GB RAM-a).");
    }
}
