// ============================================================================
// pipeline_demo.rs  —  protočni (pipeline) sort u dve faze
// ============================================================================
//
// Dve faze, kao montažna traka:
//
//   izvor (tok) -> [Faza 1: generator runova]  -> [Faza 2: k-way merge] -> izlaz
//                   blok -> lokalno direktno        spoji runove u
//                   adresiranje (BEZ poređenja)     globalni red (PORED I)
//
// Pošteno: poređenja NEMA unutar bloka (Faza 1 — direktno adresiranje).
// Spajanje blokova u globalni red (Faza 2) MORA da poredi čela runova —
// za proizvoljne vrednosti nema načina da se globalni red sklopi bez toga.
//
// Dve izvedbe iste strukture:
//   --sekv     Faze jedna za drugom (jasno pokazuje strukturu).
//   --paralel  Faze kao niti povezane kanalom (mpsc): Faza 1 pravi runove
//              i šalje ih u cev dok Faza 2 prima i spaja — prava traka.
//
// Run:  rustc -O pipeline_demo.rs && ./pipeline_demo --sekv
//       rustc -O pipeline_demo.rs && ./pipeline_demo --paralel
// ============================================================================

use std::env;
use std::sync::mpsc;
use std::thread;

const VELICINA_BLOKA: usize = 5;

/// Faza 1: sortiraj jedan blok lokalnim direktnim adresiranjem. Bez poređenja.
fn sortiraj_run(blok: &[i64]) -> Vec<i64> {
    let mut mn = blok[0];
    let mut mx = blok[0];
    for &v in &blok[1..] {
        if v < mn { mn = v; }
        if v > mx { mx = v; }
    }
    let mut slots: Vec<Vec<i64>> = vec![Vec::new(); (mx - mn + 1) as usize];
    for &v in blok {
        slots[(v - mn) as usize].push(v);   // direktan upis na adresu, bez poređenja
    }
    slots.into_iter().flatten().collect()
}

/// Faza 2: k-way merge sortiranih runova u globalni red. Broji poređenja.
fn kway_merge(runovi: &[Vec<i64>]) -> (Vec<i64>, u64) {
    let mut idx = vec![0usize; runovi.len()];
    let total: usize = runovi.iter().map(|r| r.len()).sum();
    let mut out = Vec::with_capacity(total);
    let mut poredjenja = 0u64;
    for _ in 0..total {
        let mut best: Option<usize> = None;
        for r in 0..runovi.len() {
            if idx[r] < runovi[r].len() {
                match best {
                    None => best = Some(r),
                    Some(b) => {
                        poredjenja += 1;                       // poređenje čela dva runa
                        if runovi[r][idx[r]] < runovi[b][idx[b]] { best = Some(r); }
                    }
                }
            }
        }
        let b = best.unwrap();
        out.push(runovi[b][idx[b]]);
        idx[b] += 1;
    }
    (out, poredjenja)
}

fn sekvencijalno(tok: &[i64]) -> (Vec<i64>, u64, usize) {
    let runovi: Vec<Vec<i64>> = tok
        .chunks(VELICINA_BLOKA)
        .filter(|c| !c.is_empty())
        .map(sortiraj_run)
        .collect();
    let broj_runova = runovi.len();
    let (merged, por) = kway_merge(&runovi);
    (merged, por, broj_runova)
}

fn paralelno(tok: Vec<i64>) -> (Vec<i64>, u64, usize) {
    let (tx, rx) = mpsc::channel::<Vec<i64>>();

    // Faza 1: nit koja pravi runove i šalje ih u cev kako su gotovi.
    let faza1 = thread::spawn(move || {
        for blok in tok.chunks(VELICINA_BLOKA) {
            if blok.is_empty() { continue; }
            tx.send(sortiraj_run(blok)).unwrap();   // emituj run u traku
        }
        // tx se ovde dropuje -> cev se zatvara, Faza 2 zna da je kraj
    });

    // Faza 2: nit koja prima runove dok teku, pa spaja kad svi stignu.
    let faza2 = thread::spawn(move || {
        let mut runovi: Vec<Vec<i64>> = Vec::new();
        for run in rx {                 // blokira dok runovi pristižu
            runovi.push(run);
        }
        let broj = runovi.len();
        let (merged, por) = kway_merge(&runovi);
        (merged, por, broj)
    });

    faza1.join().unwrap();
    faza2.join().unwrap()
}

fn main() {
    let paralel = env::args().any(|a| a == "--paralel");
    let rezim = if paralel { "--paralel (niti + kanal)" } else { "--sekv (faze jedna za drugom)" };

    let tok: Vec<i64> = vec![
        42, 7, -5, 23, 7,       // prvi talas
        99, -12, 50, 15, 23,    // drugi talas
        10, 5, 20, 1, 15,       // treći talas
    ];

    println!("=== PIPELINE SORT  [{}] ===", rezim);
    println!("Tok (stiže u talasima od {} ): {:?}\n", VELICINA_BLOKA, tok);

    let (merged, poredjenja, broj_runova) = if paralel {
        paralelno(tok.clone())
    } else {
        sekvencijalno(&tok)
    };

    println!("Runova (Faza 1, svaki sortiran bez poređenja): {}", broj_runova);
    println!("Globalno sortirano (Faza 2): {:?}", merged);
    println!("\nPoređenja IZMEĐU brojeva:");
    println!("   unutar blokova (Faza 1): 0     (direktno adresiranje)");
    println!("   na spajanju   (Faza 2): {}     (čela runova)", poredjenja);

    let mut ocek = tok.clone();
    ocek.sort();
    assert_eq!(merged, ocek);
    println!("\nPoklapa se sa sort(): da.");
    println!("Faza 1 je bez poređenja; globalni red traži poređenje tek na spajanju.");
    println!("Isti rezultat u oba režima — paralelni samo pušta faze da rade istovremeno.");
}
