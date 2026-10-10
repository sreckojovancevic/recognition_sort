// ============================================================================
// sort_apsolutni.rs  —  rang je UVEK izracunat (apsolutna pozicija)
// ============================================================================
//
// Ispravka prethodnog: rang se NE "utvrdjuje". Rang broja je uvek
// njegova apsolutna pozicija:  rang = vrednost - koren.  Susedi su mu
// uvek vrednost ± 1. Vazi za SVE cele brojeve, ne samo uzastopne.
//
// Zato se i proizvoljni brojevi sortiraju BEZ IJEDNOG poredjenja:
// svaki ode na svoj slot (vrednost - min), pa se procitaju popunjeni.
//
// Pravi trosak nije VREME (poredjenja), nego PROSTOR: niz mora da
// pokrije ceo opseg min..max. Za [47,3,99,8] to je 97 slotova za 4
// broja; za [3, 1_000_000_000] bio bi milijarda. Vreme -> prostor.
// (Ovo je, pod poznatim imenom, counting sort / direktno adresiranje.)
//
// Run:  rustc sort_apsolutni.rs && ./sort_apsolutni
// ============================================================================

use std::cell::Cell;
thread_local!(static POR: Cell<u64> = Cell::new(0));

/// rang = vrednost - min, uvek izracunat. Nula poredjenja za redosled.
/// Vraca (sortirano, broj_slotova).
pub fn sort_apsolutni(vals: &[i64]) -> (Vec<i64>, usize) {
    POR.with(|c| c.set(0));
    let mut mn = vals[0];
    let mut mx = vals[0];
    for &v in &vals[1..] {
        if v < mn { mn = v; }   // skan za granice; ne poredi par-par za redosled
        if v > mx { mx = v; }
    }
    let prostor = (mx - mn + 1) as usize;
    let mut brojaci = vec![0u32; prostor];
    for &v in vals {
        brojaci[(v - mn) as usize] += 1;     // rang = v - min; bez poredjenja
    }
    let mut out = Vec::with_capacity(vals.len());
    for (i, &c) in brojaci.iter().enumerate() {
        for _ in 0..c { out.push(mn + i as i64); }
    }
    (out, prostor)
}

fn poredjenja() -> u64 { POR.with(|c| c.get()) }

fn main() {
    for data in [
        vec![47, 3, 99, 8, 3],
        vec![14, 10, 13, 11, 12],
        vec![5, 2, 9, 1],
    ] {
        let (r, prostor) = sort_apsolutni(&data);
        let mut ocek = data.clone(); ocek.sort();
        println!("{:?}", data);
        println!("  -> {:?}", r);
        println!("     poredjenja: {}   prostor (slotova): {}   poklapa sorted: {}",
                 poredjenja(), prostor, r == ocek);
        assert_eq!(r, ocek);
        assert_eq!(poredjenja(), 0);
    }
    println!("\nSvi prosli: proizvoljni brojevi sortirani sa 0 poredjenja.");
    println!("Rang je uvek izracunat (vrednost - min). Cena je PROSTOR, ne vreme.");
}
