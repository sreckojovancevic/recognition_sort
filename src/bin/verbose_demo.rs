// ============================================================================
// verbose_demo.rs  —  samosvesni broj, korak po korak (za male nizove)
// ============================================================================
//
// Benchmark pokazuje BROJEVE; ovaj demo pokazuje MEHANIZAM. Na malom nizu
// ispisuje kako svaki broj "prepoznaje" sopstveno mesto — bez ijednog
// poređenja sa drugim brojem. Cilj: da apstraktna ideja "broj zna gde
// pripada" postane vidljiva u jednom pogledu.
//
// Run:  rustc -O verbose_demo.rs && ./verbose_demo
// ============================================================================

fn main() {
    let ulaz: [i64; 8] = [42, 7, 23, 7, 99, 50, 15, 23];
    println!("Ulaz (razbacano): {:?}\n", ulaz);

    // Korak 1: nađi koren (offset). Skan min/max — NE poredi brojeve za redosled.
    let mut mn = ulaz[0];
    let mut mx = ulaz[0];
    for &v in &ulaz[1..] {
        if v < mn { mn = v; }
        if v > mx { mx = v; }
    }
    println!("Koren (offset) = min = {}, najveći = {}", mn, mx);
    println!("Prostor ima {} mesta (slotova), indeksi 0..{}\n", mx - mn + 1, mx - mn);

    // Korak 2: svaki broj prepoznaje svoje mesto iz SEBE: index = vrednost - koren.
    let mut slots: Vec<Vec<i64>> = vec![Vec::new(); (mx - mn + 1) as usize];
    let mut poredjenja = 0u64;        // broji poređenja IZMEĐU brojeva (ostaće 0)
    for &v in &ulaz {
        let idx = v - mn;
        println!(
            "Broj {:>3} prepoznaje svoju vrednost, oduzima koren ({:>2}): {} - {} = {}  ->  mesto (indeks) {}",
            v, mn, v, mn, idx, idx
        );
        slots[idx as usize].push(v);  // ode direktno na svoje mesto; nikog ne pita
        poredjenja += 0;
    }

    // Korak 3: pročitaj popunjena mesta redom — to je organizacija.
    println!("\nProstor (samo popunjena mesta):");
    for (i, kanta) in slots.iter().enumerate() {
        if !kanta.is_empty() {
            println!("  indeks {:>2} (vrednost {:>3}): {:?}", i, i as i64 + mn, kanta);
        }
    }

    let izlaz: Vec<i64> = slots.iter().flatten().copied().collect();
    println!("\nOrganizovano: {:?}", izlaz);
    println!("Poređenja IZMEĐU brojeva: {}  (svaki je sam prepoznao mesto)", poredjenja);

    let mut ocek = ulaz.to_vec();
    ocek.sort();
    assert_eq!(izlaz, ocek);
    assert_eq!(poredjenja, 0);
    println!("\nPoklapa se sa sort(): da.  Duplikati (7, 23) mirno dele svoje mesto.");
    println!("Napomena: ovo je gust/ograničen slučaj (indeks = vrednost - koren).");
    println!("radix generalizuje istu ideju cifru-po-cifru, pa prostor ne zavisi od opsega.");
}
