// ============================================================================
// rang_radix_1m.rs  —  radix na 1,000,000 brojeva, sa merenjem vremena
// ============================================================================
//
// Isti mehanizam kao rang_radix.rs, ali na milion brojeva iz ogromnog
// opsega. Nula poredjenja; prostor nezavisan od opsega; meri se vreme.
//
// Run (obavezno -O za realno vreme):
//     rustc -O rang_radix_1m.rs && ./rang_radix_1m
// ============================================================================

use std::cell::Cell;
use std::time::Instant;

thread_local!(static POR: Cell<u64> = Cell::new(0));

pub fn radix_sort(vals: &[i64]) -> (Vec<i64>, u32) {
    POR.with(|c| c.set(0));
    if vals.is_empty() { return (vec![], 0); }

    let mut mn = vals[0];
    let mut mx = vals[0];
    for &v in &vals[1..] {
        if v < mn { mn = v; }
        if v > mx { mx = v; }
    }
    let baza: u64 = 10;
    let mut tek: Vec<u64> = vals.iter().map(|&v| (v - mn) as u64).collect();
    let max_rad = (mx - mn) as u64;

    // Stabilni counting po jednoj cifri, bez alokacije Vec-po-kofici
    // (brze: jedan bafer + prefiksne sume).
    let mut bafer = vec![0u64; tek.len()];
    let mut mesto: u64 = 1;
    let mut prolaza = 0u32;
    while max_rad / mesto > 0 {
        let mut count = [0usize; 10];
        for &v in &tek {
            count[((v / mesto) % baza) as usize] += 1;
        }
        let mut suma = 0usize;
        for c in count.iter_mut() {
            let t = *c; *c = suma; suma += t;      // prefiksne sume -> pocetni indeksi
        }
        for &v in &tek {
            let d = ((v / mesto) % baza) as usize;
            bafer[count[d]] = v;                    // smesti stabilno; bez poredjenja
            count[d] += 1;
        }
        std::mem::swap(&mut tek, &mut bafer);
        mesto *= baza;
        prolaza += 1;
    }
    let out: Vec<i64> = tek.into_iter().map(|v| v as i64 + mn).collect();
    (out, prolaza)
}

fn poredjenja() -> u64 { POR.with(|c| c.get()) }

fn main() {
    let mut state: u64 = 0xDEADBEEFCAFEBABE;
    let mut rnd = || { state ^= state << 13; state ^= state >> 7; state ^= state << 17; state };

    let n = 1_000_000;
    let vals: Vec<i64> = (0..n).map(|_| (rnd() % 100_000_000) as i64).collect();

    let t0 = Instant::now();
    let (res, prolaza) = radix_sort(&vals);
    let t_radix = t0.elapsed();

    let mut ocek = vals.clone();
    let t1 = Instant::now();
    ocek.sort();                          // poredjenje: standardni sort
    let t_std = t1.elapsed();

    let mn = *vals.iter().min().unwrap();
    let mx = *vals.iter().max().unwrap();

    println!("N = {}, opseg do 100,000,000", n);
    println!("  poklapa se sa sort(): {}", res == ocek);
    println!("  poredjenja izmedju vrednosti (radix): {}", poredjenja());
    println!("  prolaza (cifara): {}", prolaza);
    println!("  vreme radix:       {:?}", t_radix);
    println!("  vreme std sort():  {:?}", t_std);
    println!("  prostor radix: 10 brojaca + bafer N");
    println!("  direktno adresiranje bi trazilo: {} slotova", mx - mn + 1);

    assert_eq!(res, ocek);
    assert_eq!(poredjenja(), 0);
    println!("\nProslo: 1,000,000 brojeva sortirano sa 0 poredjenja, prostor mali.");
}
