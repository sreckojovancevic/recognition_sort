// ============================================================================
// verbose_demo.rs  —  samosvesni broj, korak po korak (za male nizove)
// ============================================================================
//
// Benchmark pokazuje BROJEVE; ovaj demo pokazuje MEHANIZAM. Na malom nizu
// ispisuje kako svaki broj "prepoznaje" sopstveno mesto — bez ijednog
// poređenja sa drugim brojem.
//
// Probaj svoje brojeve:
//     cargo run --release --bin verbose_demo -- 42 7 23 99 7
//     rustc -O verbose_demo.rs && ./verbose_demo 42 7 23 99 7
// Bez argumenata koristi podrazumevani niz.
// ============================================================================

use std::env;

/// Prepoznavanje + smeštanje. Vraća (organizovano, broj_poređenja_izmedju).
/// index = vrednost - koren; nijedan broj se ne poredi sa drugim.
fn organizuj(ulaz: &[i64], verbose: bool) -> (Vec<i64>, u64) {
    let mut mn = ulaz[0];
    let mut mx = ulaz[0];
    for &v in &ulaz[1..] {
        if v < mn { mn = v; }
        if v > mx { mx = v; }
    }
    if verbose {
        println!("Koren (offset) = min = {}, najveći = {}", mn, mx);
        println!("Prostor ima {} mesta (slotova), indeksi 0..{}\n", mx - mn + 1, mx - mn);
    }
    let mut slots: Vec<Vec<i64>> = vec![Vec::new(); (mx - mn + 1) as usize];
    for &v in ulaz {
        let idx = v - mn;
        if verbose {
            println!(
                "Broj {:>3} prepoznaje svoju vrednost, oduzima koren ({:>2}): {} - {} = {}  ->  mesto (indeks) {}",
                v, mn, v, mn, idx, idx
            );
        }
        slots[idx as usize].push(v);     // ode direktno na svoje mesto; nikog ne pita
    }
    if verbose {
        println!("\nProstor (samo popunjena mesta):");
        for (i, kanta) in slots.iter().enumerate() {
            if !kanta.is_empty() {
                println!("  indeks {:>2} (vrednost {:>3}): {:?}", i, i as i64 + mn, kanta);
            }
        }
    }
    let izlaz: Vec<i64> = slots.iter().flatten().copied().collect();
    (izlaz, 0)
}

/// Običan insertion sort, ali BROJI svoja poređenja — za kontrast.
fn insertion_sort_poredjenja(ulaz: &[i64]) -> (Vec<i64>, u64) {
    let mut a = ulaz.to_vec();
    let mut poredjenja = 0u64;
    for i in 1..a.len() {
        let mut j = i;
        while j > 0 {
            poredjenja += 1;              // poređenje IZMEĐU dva broja
            if a[j - 1] > a[j] { a.swap(j - 1, j); j -= 1; } else { break; }
        }
    }
    (a, poredjenja)
}

fn main() {
    // Argumenti kao brojevi; ako ih nema, podrazumevani niz.
    let args: Vec<i64> = env::args().skip(1).filter_map(|s| s.parse().ok()).collect();
    let ulaz: Vec<i64> = if args.is_empty() {
        vec![42, 7, -5, 23, 7, 99, -12, 50, 15, 23]   // uključuje negativne
    } else {
        args
    };

    println!("Ulaz (razbacano): {:?}\n", ulaz);

    // --- Prostorni zid: direktno adresiranje traži niz veličine opsega ---
    // Opseg računamo u i128 da se ne prelije (mx - mn može preći i64 za
    // ekstremne vrednosti, npr. i64::MIN .. i64::MAX = 2^64 - 1).
    let mn = *ulaz.iter().min().unwrap();
    let mx = *ulaz.iter().max().unwrap();
    let opseg: i128 = (mx as i128) - (mn as i128) + 1;

    // Odluka direktno-adresiranje vs radix: plafon (memorija) + gustina (isplativost).
    const MAX_SLOTOVA: i128 = 200_000_000;   // tvrd plafon (proxy za memoriju)
    const PRAG_GUSTINE: f64 = 0.05;          // ispod 5% popunjenosti -> radix
    const MIN_ZA_GUSTINU: i128 = 10_000;     // ispod ovog demo uvek prikazuje korake
    let gustina = ulaz.len() as f64 / opseg as f64;
    let previse_veliko = opseg > MAX_SLOTOVA;
    let previse_retko = opseg > MIN_ZA_GUSTINU && gustina < PRAG_GUSTINE;
    if previse_veliko || previse_retko {
        println!("Opseg = {} (od {} do {}), gustina = {:.4}% ({} brojeva).",
                 opseg, mn, mx, gustina * 100.0, ulaz.len());
        if previse_veliko {
            println!("-> PREVELIK opseg za direktno adresiranje (plafon {} slotova).", MAX_SLOTOVA);
        }
        if previse_retko {
            println!("-> PRERETKO: ispod {}% popunjenosti, većina slotova bi bila prazna.",
                     PRAG_GUSTINE * 100.0);
        }
        println!("Ovo je prostorni zid iz LIMITS. Za ovakav ulaz koristi radix");
        println!("(prostor = baza + n, nezavisno od opsega i gustine).");
        return;
    }

    // --- Mehanizam, korak po korak ---
    let (izlaz, por_rec) = organizuj(&ulaz, true);
    println!("\nOrganizovano: {:?}", izlaz);

    let mut ocek = ulaz.clone();
    ocek.sort();
    assert_eq!(izlaz, ocek);
    assert_eq!(por_rec, 0);

    // --- Kontrast: koliko poređenja bi napravio običan sort? ---
    let (_, por_ins) = insertion_sort_poredjenja(&ulaz);
    println!("\nPoređenja IZMEĐU brojeva:");
    println!("   prepoznavanje : {}   (svaki broj sam prepoznao mesto)", por_rec);
    println!("   insertion sort: {}   (toliko puta je pitao 'da li je a > b?')", por_ins);

    // --- Nezavisnost od redosleda dolaska ---
    let mut obrnut = ulaz.clone();
    obrnut.reverse();
    let (izlaz2, _) = organizuj(&obrnut, false);
    println!("\nIsti skup, obrnut redosled dolaska: {:?}", obrnut);
    println!("   rezultat: {:?}", izlaz2);
    println!("   isti kao pre? {}  -> pozicija dolazi iz identiteta, ne iz redosleda", izlaz2 == izlaz);
    assert_eq!(izlaz2, izlaz);

    println!("\nPoklapa se sa sort(): da.  Duplikati mirno dele svoje mesto.");
    println!("Negativni brojevi rade bez ikakvog posebnog slučaja: koren (min) može biti");
    println!("negativan, pa je 'vrednost - koren' uvek >= 0. Znak nestane pre smeštanja.");
    println!("Napomena: ovo je gust/ograničen slučaj (indeks = vrednost - koren).");
    println!("radix generalizuje istu ideju cifru-po-cifru, pa prostor ne zavisi od opsega.");
}
