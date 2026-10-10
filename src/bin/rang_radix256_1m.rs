// ============================================================================
// rang_radix256_1m.rs  —  radix baza 256 (bajt po bajt), 1,000,000 brojeva
// ============================================================================
//
// Isto kao rang_radix_1m, ali baza = 256: prepoznaje se BAJT po bajt umesto
// decimalna cifra. Za opseg do 100M to je 4 prolaza umesto 8 (baza 10).
// Ovo je bukvalno isti mehanizam kao MSD/bajt prepoznavanje stringova.
//
// Nula poredjenja; prostor = 256 brojaca + bafer N.
//
// Run:  rustc -O rang_radix256_1m.rs && ./rang_radix256_1m
// ============================================================================

use std::time::Instant;

/// LSD radix baza 256. Pomeri na nenegativne (koren = min), pa bajt po bajt.
/// Vraca (sortirano, broj_prolaza). Nula poredjenja izmedju vrednosti.
pub fn radix256(vals: &[i64]) -> (Vec<i64>, u32) {
    if vals.is_empty() { return (vec![], 0); }

    let mut mn = vals[0];
    let mut mx = vals[0];
    for &v in &vals[1..] {
        if v < mn { mn = v; }
        if v > mx { mx = v; }
    }
    let mut tek: Vec<u64> = vals.iter().map(|&v| (v - mn) as u64).collect();
    let max_rad = (mx - mn) as u64;

    let mut bafer = vec![0u64; tek.len()];
    let mut pomak: u32 = 0;
    let mut prolaza = 0u32;
    while (max_rad >> pomak) > 0 {
        let mut count = [0usize; 256];
        for &v in &tek {
            count[((v >> pomak) & 0xFF) as usize] += 1;
        }
        let mut suma = 0usize;
        for c in count.iter_mut() {
            let t = *c; *c = suma; suma += t;        // prefiksne sume
        }
        for &v in &tek {
            let d = ((v >> pomak) & 0xFF) as usize;
            bafer[count[d]] = v;                      // stabilno, bez poredjenja
            count[d] += 1;
        }
        std::mem::swap(&mut tek, &mut bafer);
        pomak += 8;
        prolaza += 1;
    }
    (tek.into_iter().map(|v| v as i64 + mn).collect(), prolaza)
}

fn main() {
    let mut state: u64 = 0xDEADBEEFCAFEBABE;
    let mut rnd = || { state ^= state << 13; state ^= state >> 7; state ^= state << 17; state };

    let n = 1_000_000;
    let vals: Vec<i64> = (0..n).map(|_| (rnd() % 100_000_000) as i64).collect();

    let t0 = Instant::now();
    let (res, prolaza) = radix256(&vals);
    let t_radix = t0.elapsed();

    let mut ocek = vals.clone();
    let t1 = Instant::now();
    ocek.sort();
    let t_std = t1.elapsed();

    println!("N = {}, opseg do 100,000,000, baza 256", n);
    println!("  poklapa se sa sort(): {}", res == ocek);
    println!("  prolaza (bajtova): {}   (naspram 8 u bazi 10)", prolaza);
    println!("  vreme radix256:    {:?}", t_radix);
    println!("  vreme std sort():  {:?}", t_std);

    assert_eq!(res, ocek);
    println!("\nProslo: 1,000,000 brojeva, bajt-po-bajt, 0 poredjenja.");
}
