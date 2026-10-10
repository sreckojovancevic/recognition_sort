// ============================================================================
// rang_broj.rs  —  broj sa dvostrukim identitetom: vrednost + rang
// ============================================================================
//
// Dva identiteta:
//   vrednost  — STA je (apstrakcija kolicine)
//   rang      — GDE stoji u nizu (pozicija)
//
// Pitanje: nosi li rang informaciju koju vrednost vec nema?
//
//   - UZASTOPNI brojevi:  rang = vrednost - koren  -> rang se RACUNA iz
//     vrednosti, dakle SUVISAN je (drugi identitet je ukras).
//
//   - PROIZVOLJNI podaci: rang NIJE u vrednosti. Broj 47 ne zna da je
//     treci po velicini — to zavisi od svih ostalih. Tu rang nosi PRAVI
//     visak informacije, i drugi identitet postaje stvaran.
//
// Isplata: kad svaki broj nosi svoj rang, rekonstrukcija poretka je
// smestanje po rangu — O(n), bez ijednog poredjenja izmedju elemenata.
// (Cena rangiranja proizvoljnih se plati jednom; posle je besplatno.)
//
// Run:  rustc rang_broj.rs && ./rang_broj
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangBroj {
    pub vrednost: i64,
    pub rang: usize,
}

/// Uzastopni: rang se RACUNA iz vrednosti (suvisan, izvodljiv).
pub fn iz_uzastopnih(vals: &[i64], koren: i64) -> Vec<RangBroj> {
    vals.iter()
        .map(|&v| RangBroj { vrednost: v, rang: (v - koren) as usize })
        .collect()
}

/// Proizvoljni: rang se CUVA. Mora se znati ceo skup (sortira se jednom),
/// jer rang nije funkcija pojedinacne vrednosti.
pub fn iz_proizvoljnih(vals: &[i64]) -> Vec<RangBroj> {
    let mut poredak: Vec<usize> = (0..vals.len()).collect();
    poredak.sort_by_key(|&i| vals[i]);        // jednokratna cena rangiranja
    let mut rang = vec![0usize; vals.len()];
    for (mesto, &i) in poredak.iter().enumerate() {
        rang[i] = mesto;
    }
    (0..vals.len())
        .map(|i| RangBroj { vrednost: vals[i], rang: rang[i] })
        .collect()
}

/// Rekonstrukcija poretka: svaki broj ode na svoj rang.
/// O(n), nijedno poredjenje izmedju elemenata.
pub fn rekonstruisi(brojevi: &[RangBroj]) -> Vec<i64> {
    let mut slots: Vec<Option<i64>> = vec![None; brojevi.len()];
    for rb in brojevi {
        slots[rb.rang] = Some(rb.vrednost);
    }
    slots.into_iter().flatten().collect()
}

fn main() {
    // --- Slucaj 1: uzastopni -> rang suvisan -----------------------
    let a = iz_uzastopnih(&[14, 10, 13, 11, 12], 10);
    let redundantan = a.iter().all(|rb| rb.rang == (rb.vrednost - 10) as usize);
    println!("SLUCAJ 1 (uzastopni [10..14]):");
    println!("  rang == vrednost - koren za sve? {}  -> rang SUVISAN", redundantan);
    println!("  rekonstruisano: {:?}", rekonstruisi(&a));
    assert!(redundantan);
    assert_eq!(rekonstruisi(&a), vec![10, 11, 12, 13, 14]);

    // --- Slucaj 2: proizvoljni -> rang nosi informaciju ------------
    let vals = [47, 3, 99, 8];
    let b = iz_proizvoljnih(&vals);
    let minv = vals.iter().copied().min().unwrap();
    let formula_vazi = b.iter().all(|rb| rb.rang == (rb.vrednost - minv) as usize);
    println!("\nSLUCAJ 2 (proizvoljni [47,3,99,8]):");
    for rb in &b { println!("  v={:3}  rang={}", rb.vrednost, rb.rang); }
    println!("  vazi 'rang = vrednost - min'? {}  -> NE: rang NIJE u vrednosti", formula_vazi);
    println!("  rekonstruisano (po rangu, bez poredjenja): {:?}", rekonstruisi(&b));
    assert!(!formula_vazi);
    assert_eq!(rekonstruisi(&b), vec![3, 8, 47, 99]);

    // --- Dokaz: ista vrednost, razlicit rang = pravi visak info ----
    let rang_u = |skup: &[i64], v: i64| -> usize {
        iz_proizvoljnih(skup).into_iter().find(|rb| rb.vrednost == v).unwrap().rang
    };
    let r1 = rang_u(&[47, 3], 47);     // 47 je veci -> rang 1
    let r2 = rang_u(&[47, 99], 47);    // 47 je manji -> rang 0
    println!("\nDOKAZ (rang nosi informaciju):");
    println!("  47 u [47,3]  -> rang {}", r1);
    println!("  47 u [47,99] -> rang {}", r2);
    println!("  ista vrednost, razlicit rang -> rang nije funkcija vrednosti");
    assert_eq!(r1, 1);
    assert_eq!(r2, 0);
    assert_ne!(r1, r2);

    println!("\nSvi testovi prosli: za uzastopne rang je suvisan (izvodljiv),");
    println!("za proizvoljne rang nosi pravi visak informacije (drugi identitet).");
}
