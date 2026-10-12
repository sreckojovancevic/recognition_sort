//! Paralelni LSD radix sort za u32: 4 prolaza po 8 bita.
//!
//! Svaki prolaz: (1) histogram po delu niza, paralelno; (2) prefiksne sume tako da
//! deo c dobija svoj opseg u svakom bucket-u, posle delova 0..c; (3) paralelno
//! raspoređivanje u pomoćni bafer. Redosled unutar bucket-a čuva redosled delova i
//! redosled unutar dela, pa je svaki prolaz stabilan — uslov ispravnosti LSD-a.
//! Posle 4 prolaza (paran broj) rezultat je ponovo u ulaznom nizu.

use rayon::prelude::*;

const RADIX_BITS: u32 = 8;
const BUCKETS: usize = 1 << RADIX_BITS;
const PASSES: u32 = 32 / RADIX_BITS;

/// Pokazivač koji se sme deliti između niti: svaki deo piše isključivo
/// u svoje, unapred izračunate i međusobno disjunktne pozicije.
#[derive(Clone, Copy)]
struct SendPtr(*mut u32);
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

/// Sortira `data` u mestu. `scratch` je pomoćni bafer koji pozivalac čuva između
/// poziva (da se ne alocira 32 MiB po bloku); po potrebi se proširi.
pub fn radix_sort_u32(data: &mut [u32], scratch: &mut Vec<u32>) {
    let n = data.len();
    if n < 2 {
        return;
    }
    if scratch.len() < n {
        scratch.resize(n, 0);
    }
    let scratch = &mut scratch[..n];

    let threads = rayon::current_num_threads().max(1);
    // Deo ne manji od 64K elemenata, da histogrami ne budu skuplji od posla
    let chunk = ((n + threads - 1) / threads).max(1 << 16);

    let mut src_is_data = true;
    for pass in 0..PASSES {
        let shift = pass * RADIX_BITS;
        let (src, dst): (&[u32], &mut [u32]) = if src_is_data {
            (&*data, &mut *scratch)
        } else {
            (&*scratch, &mut *data)
        };

        // 1. Histogram po delu
        let hists: Vec<[usize; BUCKETS]> = src
            .par_chunks(chunk)
            .map(|c| {
                let mut h = [0usize; BUCKETS];
                for &v in c {
                    h[((v >> shift) as usize) & (BUCKETS - 1)] += 1;
                }
                h
            })
            .collect();

        // Ako su sve vrednosti u istom bucket-u, prolaz ne menja redosled — preskoči
        let mut totals = [0usize; BUCKETS];
        for h in &hists {
            for b in 0..BUCKETS {
                totals[b] += h[b];
            }
        }
        if totals.iter().any(|&t| t == n) {
            continue;
        }

        // 2. Početne pozicije: bucket po bucket, unutar bucket-a deo po deo
        let mut offsets: Vec<[usize; BUCKETS]> = vec![[0usize; BUCKETS]; hists.len()];
        let mut running = 0usize;
        for b in 0..BUCKETS {
            for (c, h) in hists.iter().enumerate() {
                offsets[c][b] = running;
                running += h[b];
            }
        }
        debug_assert_eq!(running, n);

        // 3. Raspoređivanje
        let out = SendPtr(dst.as_mut_ptr());
        src.par_chunks(chunk)
            .zip(offsets.into_par_iter())
            .for_each(move |(c, mut off)| {
                let out = out;
                for &v in c {
                    let b = ((v >> shift) as usize) & (BUCKETS - 1);
                    // SAFETY: opsezi [offsets[c][b], +hists[c][b]) su disjunktni za sve (c, b)
                    // i ukupno pokrivaju 0..n; svaka pozicija se upisuje tačno jednom.
                    unsafe { *out.0.add(off[b]) = v; }
                    off[b] += 1;
                }
            });

        src_is_data = !src_is_data;
    }

    // Ako je zbog preskočenih prolaza rezultat ostao u pomoćnom baferu, vrati ga
    if !src_is_data {
        data.copy_from_slice(scratch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;

    fn check(mut v: Vec<u32>) {
        let mut expected = v.clone();
        expected.sort_unstable();
        let mut scratch = Vec::new();
        radix_sort_u32(&mut v, &mut scratch);
        assert_eq!(v, expected);
    }

    #[test]
    fn random_sizes() {
        let mut rng = StdRng::seed_from_u64(5);
        for &n in &[0usize, 1, 2, 3, 100, 65_535, 65_536, 65_537, 300_000, 1_000_003] {
            check((0..n).map(|_| rng.gen()).collect());
        }
    }

    #[test]
    fn duplicates_and_extremes() {
        let mut rng = StdRng::seed_from_u64(6);
        check((0..500_000).map(|_| (rng.gen::<u32>() % 100) * 1000).collect());
        check(vec![u32::MAX; 200_000]);
        check(vec![0; 200_000]);
        check((0..300_000u32).rev().collect());
        check((0..300_000u32).collect());
        // Razlika samo u najvišem bajtu — preskaču se prolazi 0–2
        check((0..300_000u32).map(|i| (i % 256) << 24).collect());
    }

    #[test]
    fn scratch_is_reused() {
        let mut rng = StdRng::seed_from_u64(7);
        let mut scratch = Vec::new();
        for _ in 0..3 {
            let mut v: Vec<u32> = (0..200_000).map(|_| rng.gen()).collect();
            let mut e = v.clone();
            e.sort_unstable();
            radix_sort_u32(&mut v, &mut scratch);
            assert_eq!(v, e);
        }
        assert_eq!(scratch.len(), 200_000);
    }
}
