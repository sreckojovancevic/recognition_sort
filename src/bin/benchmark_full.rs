// ============================================================================
// benchmark_full.rs  —  vreme + STVARNA memorija + korektnost
// ============================================================================
//
// Meri, za svaki volumen: vreme radix256 vs std::sort, stvarno izmerenu
// vrsnu (peak) memoriju radix-a, i korektnost (poredjenje sa std::sort).
//
// Memorija se meri BROJECIM ALOKATOROM (cist std, bez eksternih crate-ova):
// svaka heap-alokacija/dealokacija se broji, pamti se peak. To je stvarno
// izmereno, ne procenjeno.
//
// Vrednosti su u32 (opseg do 100M staje), da memorija bude upola manja.
//
// UPOZORENJE NA MEMORIJU: 100M u32 -> peak oko ~1.6 GB. Ako ti je malo RAM-a,
// zakomentarisi velike volumene u `sizes`.
//
// Run (OBAVEZNO -O):  rustc -O benchmark_full.rs && ./benchmark_full
// ============================================================================

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::time::Instant;

// --- broječi alokator: prati trenutno i vrsno zauzece heap-a ---------------
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
fn mb(bytes: usize) -> f64 { bytes as f64 / (1024.0 * 1024.0) }

// --- radix baza 256 za u32 (bajt po bajt), bez poredjenja ------------------
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

fn main() {
    let sizes: [usize; 6] = [100, 10_000, 20_000_000, 50_000_000, 70_000_000, 100_000_000];

    println!("{:>12} | {:>10} | {:>10} | {:>7} | {:>9} | {:>9} | {}",
             "N", "radix", "std sort", "odnos", "radix mem", "ulaz mem", "korektno");
    println!("{}", "-".repeat(84));

    let mut state: u64 = 0x12345678ABCDEF01;
    let mut rnd = || { state ^= state << 13; state ^= state >> 7; state ^= state << 17; state };

    for &n in &sizes {
        let vals: Vec<u32> = (0..n).map(|_| (rnd() % 100_000_000) as u32).collect();
        let ulaz_mem = mb(n * std::mem::size_of::<u32>());

        // --- radix: meri vreme i vrsnu memoriju ---
        let base = cur();
        reset_peak();
        let t0 = Instant::now();
        let out = radix256(&vals);
        let t_radix = t0.elapsed();
        let radix_mem = mb(peak().saturating_sub(base));

        // --- std sort: vreme (na kloniranom nizu) ---
        let mut s = vals.clone();
        let t1 = Instant::now();
        s.sort();
        let t_std = t1.elapsed();

        // --- korektnost: poredjenje sa std::sort ---
        let korektno = out == s;

        let odnos = t_std.as_secs_f64() / t_radix.as_secs_f64();
        println!("{:>12} | {:>8.1?} | {:>8.1?} | {:>6.2}x | {:>7.1} MB | {:>7.1} MB | {}",
                 n, t_radix, t_std, odnos, radix_mem, ulaz_mem,
                 if korektno { "DA" } else { "NE !!!" });
        assert!(korektno, "radix i std sort se razlikuju na N={}", n);
    }

    println!("\nSvi prosli. odnos > 1 znaci radix brzi; < 1 znaci std sort brzi.");
    println!("radix mem = stvarno izmereno vrsno zauzece radix-a (broječi alokator).");
}
