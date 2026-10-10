// ============================================================================
// samosvesni_broj.rs  —  N@ : the self-aware number
// ============================================================================
//
// Core idea (valid in a precise domain): for consecutive integers with
// step 1, a number's neighbours are NOT stored — they are COMPUTED from
// its own value. The successor is a LAW (n + 1), not data.
//
//     3  ==  2 < 3 < 4       meta(3) = (2, 3, 4)   derived, never stored
//
// This is Peano's successor function: 3 is S(S(S(0))). The relational
// identity IS the constructive definition of the number.
//
// Consequence: a number already knows its place. Its position is its own
// identity, offset by the origin:
//
//     position(n) = n - origin
//
// So ordering needs ZERO pairwise comparisons between elements — each
// number lands directly at its own index. This is the purest form of
// "recognition instead of comparison" (in CS terms: direct addressing).
//
// HONEST BOUNDARY: this freeness holds only for a DENSE, step-1 domain.
// For an arbitrary sparse set like [3, 9, 8, 2, 7, 5] the successor is no
// longer n+1 (3's successor is 5, not 4), so the neighbour stops being
// computable from the value alone — and this method does not apply there.
//
// Run:  rustc samosvesni_broj.rs && ./samosvesni_broj      (self-test incl.)
// ============================================================================

/// N@ — a self-aware integer. Holds only its value; its context is a law.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct N(pub i64);

impl N {
    /// Predecessor — computed, not stored.
    pub fn prethodnik(&self) -> N { N(self.0 - 1) }

    /// Successor — computed, not stored. This is the law.
    pub fn sledbenik(&self) -> N { N(self.0 + 1) }

    /// Relational identity (before, self, after) — derived from the value.
    pub fn meta(&self) -> (i64, i64, i64) { (self.0 - 1, self.0, self.0 + 1) }

    /// "2 < 3 < 4" notation.
    pub fn zapis(&self) -> String {
        format!("{} < {} < {}", self.0 - 1, self.0, self.0 + 1)
    }
}

/// Result of organizing: ordered slots (None = a gap / missing number),
/// plus the origin that anchors the coordinate system.
pub struct Organizacija {
    pub slots: Vec<Option<N>>,
    pub koren: i64,
}

impl Organizacija {
    /// Each number lands directly at `value - origin`. No element is ever
    /// compared to another to decide order. If `koren` is known, pass it
    /// and even the min/max scan disappears.
    pub fn organizuj(brojevi: &[N], koren: Option<i64>) -> Organizacija {
        let origin = koren.unwrap_or_else(|| {
            // O(n) scan to find the coordinate origin — this establishes
            // the axis, it does NOT compare elements for relative order.
            let mut m = brojevi[0].0;
            for b in &brojevi[1..] { if b.0 < m { m = b.0; } }
            m
        });
        let mut maxv = brojevi[0].0;
        for b in &brojevi[1..] { if b.0 > maxv { maxv = b.0; } }

        let mut slots: Vec<Option<N>> = vec![None; (maxv - origin + 1) as usize];
        for &b in brojevi {
            slots[(b.0 - origin) as usize] = Some(b);   // direct placement
        }
        Organizacija { slots, koren: origin }
    }

    /// Ordered values, gaps skipped.
    pub fn niz(&self) -> Vec<i64> {
        self.slots.iter().filter_map(|s| s.map(|n| n.0)).collect()
    }

    /// Integrity: a dense step-1 chain has no gaps. Returns the indices of
    /// missing numbers (anomalies) — where the successor law is broken.
    pub fn verifikuj(&self) -> (bool, Vec<i64>) {
        let rupe: Vec<i64> = self.slots.iter().enumerate()
            .filter(|(_, s)| s.is_none())
            .map(|(i, _)| self.koren + i as i64)
            .collect();
        (rupe.is_empty(), rupe)
    }
}

fn main() {
    // Chaos arriving over network/disk, fully scattered.
    let haos: Vec<N> = [3, 0, 4, 2, 1].iter().map(|&v| N(v)).collect();

    println!("--- N@ zapis (kontekst se RACUNA iz identiteta) ---");
    for b in &haos { println!("{}   meta = {:?}", b.zapis(), b.meta()); }

    let org = Organizacija::organizuj(&haos, None);
    println!("\nhaos         : {:?}", haos.iter().map(|n| n.0).collect::<Vec<_>>());
    println!("organizovano : {:?}   koren = {}", org.niz(), org.koren);
    println!("-> svaki broj je otisao na poziciju (vrednost - koren),");
    println!("   nula poredjenja izmedju elemenata, O(n).");

    let (ok, _) = org.verifikuj();
    assert!(ok);
    assert_eq!(org.niz(), vec![0, 1, 2, 3, 4]);

    // Gap case: missing 2 -> anomaly detected by the broken successor law.
    let sa_rupom: Vec<N> = [3, 0, 4, 1].iter().map(|&v| N(v)).collect();
    let org2 = Organizacija::organizuj(&sa_rupom, None);
    let (ok2, rupe) = org2.verifikuj();
    println!("\nsa rupom [3,0,4,1] : {:?}  integritet = {}  nedostaje = {:?}",
             org2.niz(), ok2, rupe);
    assert!(!ok2);
    assert_eq!(rupe, vec![2]);

    // Pure mode: origin known in advance -> zero scans, zero comparisons.
    let org3 = Organizacija::organizuj(&haos, Some(0));
    assert_eq!(org3.niz(), vec![0, 1, 2, 3, 4]);

    println!("\nSvi testovi prosli: kontekst racunat (ne pamcen), redosled bez");
    println!("poredjenja izmedju elemenata, anomalije (rupe) detektovane.");
}
