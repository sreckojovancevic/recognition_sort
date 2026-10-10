// ============================================================================
// pozicioni_broj.rs  —  racionalni pozicioni ključ (ubacivanje bez presortiravanja)
// ============================================================================
//
// U duhu KONCEPT.md: ovo je USKLADIŠTEN dodatni kontekst. Za retke/proizvoljne
// elemente relacija (gde stoji) ne može se izvesti iz vrednosti — pa se nosi
// kao pozicioni ključ. Time element dobija "gde stoji" kao poseban identitet,
// a da ne menja svoju vrednost.
//
// Ključ je string cifara, tretiran kao decimalni razlomak 0.<cifre>.
// Leksikografsko poređenje stringova == poređenje razlomaka. Između bilo koja
// dva ključa uvek postoji treći (gustina racionalnih), pa je ubacivanje
// LOKALNO: izmedju(a, b) daje ključ strogo između, bez pomeranja ostalih.
//
// Šta ovo daje što radix/counting NE daju: održavanje poretka pod ubacivanjem.
// Cena: ponovljeno ubacivanje u isti razmak produžava ključ (nema preciznog
// zida kao f64 — ključ raste u dužini umesto da pukne).
//
// Run:  rustc -O pozicioni_broj.rs && ./pozicioni_broj
// ============================================================================

fn digit(s: &[u8], i: usize) -> i32 {
    if i < s.len() { (s[i] - b'0') as i32 } else { 0 }
}

/// Ključ strogo između a i b (oba cifarski string; "" = donja granica 0 /
/// gornja granica 1). Pretpostavlja a < b. Radi cifru-po-cifru, bez integera.
pub fn izmedju(a: &str, b: &str) -> String {
    let a = a.as_bytes();
    let b = b.as_bytes();
    let b_upper = b.is_empty();
    let mut res: Vec<u8> = Vec::new();
    let mut i = 0usize;
    loop {
        let da = digit(a, i);
        let db = if b_upper { 10 } else { digit(b, i) };
        if da == db {
            res.push(b'0' + da as u8);   // zajednički prefiks
            i += 1;
            continue;
        }
        if db - da >= 2 {
            res.push(b'0' + ((da + db) / 2) as u8);   // sredina stane ovde
            return String::from_utf8(res).unwrap();
        }
        // db - da == 1: uzmi a-cifru, pa idi dublje u rep broja a ka gornjoj granici
        res.push(b'0' + da as u8);
        i += 1;
        loop {
            let d2 = digit(a, i);
            if d2 <= 8 {
                res.push(b'0' + ((d2 + 10) / 2) as u8);
                return String::from_utf8(res).unwrap();
            }
            res.push(b'9');     // d2 == 9: zadrži 9, dublje
            i += 1;
        }
    }
}

/// Element: vrednost (šta je) + pozicioni ključ (gde stoji). Dva identiteta.
#[derive(Debug, Clone)]
pub struct PozicioniBroj {
    pub vrednost: String,
    pub kljuc: String,
}

/// Uređena lista koja se održava ubacivanjem, bez presortiravanja.
pub struct Poredak {
    pub stavke: Vec<PozicioniBroj>,   // drži se sortirano po ključu
}

impl Poredak {
    pub fn nova() -> Self { Poredak { stavke: Vec::new() } }

    /// Ubaci na logičku poziciju `idx` (između idx-1 i idx), lokalno.
    pub fn ubaci(&mut self, idx: usize, vrednost: &str) {
        let lo = if idx > 0 { self.stavke[idx - 1].kljuc.clone() } else { String::new() };
        let hi = if idx < self.stavke.len() { self.stavke[idx].kljuc.clone() } else { String::new() };
        let kljuc = izmedju(&lo, &hi);
        self.stavke.insert(idx, PozicioniBroj { vrednost: vrednost.to_string(), kljuc });
    }

    pub fn vrednosti(&self) -> Vec<&str> {
        self.stavke.iter().map(|s| s.vrednost.as_str()).collect()
    }
    pub fn kljucevi(&self) -> Vec<&str> {
        self.stavke.iter().map(|s| s.kljuc.as_str()).collect()
    }
}

fn pad(s: &str, n: usize) -> String { format!("{:0<width$}", s, width = n) }
fn kljuc_lt(x: &str, y: &str) -> bool {
    let n = x.len().max(y.len());
    pad(x, n) < pad(y, n)
}

fn main() {
    // Počni sa tri elementa
    let mut p = Poredak::nova();
    p.ubaci(0, "A");
    p.ubaci(1, "B");
    p.ubaci(2, "C");
    println!("početak:          {:?}   ključevi {:?}", p.vrednosti(), p.kljucevi());

    // Ubaci X između A i B — niko drugi ne menja ključ
    let kljucevi_pre: Vec<String> = p.kljucevi().iter().map(|s| s.to_string()).collect();
    p.ubaci(1, "X");
    println!("X između A,B:     {:?}   ključevi {:?}", p.vrednosti(), p.kljucevi());
    // A i B i C su zadržali svoje ključeve (lokalnost)
    assert_eq!(p.stavke[0].kljuc, kljucevi_pre[0]);
    assert_eq!(p.stavke[2].kljuc, kljucevi_pre[1]);
    assert_eq!(p.stavke[3].kljuc, kljucevi_pre[2]);

    // Ubaci Y između X i B
    p.ubaci(2, "Y");
    println!("Y između X,B:     {:?}   ključevi {:?}", p.vrednosti(), p.kljucevi());
    assert_eq!(p.vrednosti(), vec!["A", "X", "Y", "B", "C"]);

    // Ključevi su strogo rastući (poredak održan)
    for w in p.kljucevi().windows(2) {
        assert!(kljuc_lt(w[0], w[1]), "ključevi nisu rastući");
    }

    // Rast ključa: mnogo ubacivanja u isti razmak -> ključ se produžava
    let mut a = String::from("2");
    let b = String::from("5");
    let start = a.len();
    for _ in 0..60 { a = izmedju(&a, &b); }
    println!("\n60 ubacivanja u isti razmak: dužina ključa {} -> {}", start, a.len());
    println!("  (ključ raste u dužini, ne puca — nema preciznog zida kao f64)");

    // Stres: mnogo nasumičnih ubacivanja, poredak ostaje tačan
    let mut state: u64 = 0xA5A5A5A5A5A5A5A5;
    let mut rnd = || { state ^= state << 13; state ^= state >> 7; state ^= state << 17; state };
    let mut q = Poredak::nova();
    q.ubaci(0, "0");
    for i in 1..2000 {
        let idx = (rnd() as usize) % (q.stavke.len() + 1);
        q.ubaci(idx, &i.to_string());
    }
    let mut ok = true;
    for w in q.kljucevi().windows(2) { if !kljuc_lt(w[0], w[1]) { ok = false; break; } }
    println!("\n2000 nasumičnih ubacivanja: ključevi strogo rastu? {}", ok);
    assert!(ok);

    println!("\nSve prošlo: ubacivanje je lokalno (ostali ključevi netaknuti),");
    println!("poredak se održava bez presortiravanja, ključ raste umesto da pukne.");
}
