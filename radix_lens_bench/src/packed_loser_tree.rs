//! Loser tree sa upakovanim ključem.
//!
//! Ključ = (vrednost as u64) << 32 | indeks_run-a.
//! Poređenje jednog u64 daje isti redosled kao (vrednost, pa indeks run-a)
//! u originalnom LoserTree-u. Iscrpljen run i dumi listovi = u64::MAX, što je
//! veće od svakog stvarnog ključa (indeks run-a je uvek < u32::MAX).
//!
//! Čvorovi čuvaju same ključeve (ne indekse igrača), pa nema dvostrukog
//! indeksiranja ni `Option` grananja: jedan korak naviše je min/max dva u64.

pub const EXHAUSTED: u64 = u64::MAX;

#[inline(always)]
pub fn pack(value: u32, run: usize) -> u64 {
    ((value as u64) << 32) | run as u64
}

#[inline(always)]
pub fn unpack(key: u64) -> (u32, usize) {
    ((key >> 32) as u32, (key & 0xFFFF_FFFF) as usize)
}

pub struct PackedLoserTree<const COUNT: bool> {
    losers: Vec<u64>, // losers[1..k] — ključevi gubitnika u unutrašnjim čvorovima
    winner: u64,
    k: usize,
    pub comparison_count: u64,
}

impl<const COUNT: bool> PackedLoserTree<COUNT> {
    /// `initial[i]` je prvi element run-a `i` (None = prazan run).
    pub fn new(initial: &[Option<u32>]) -> Self {
        assert!((initial.len() as u64) < u32::MAX as u64, "previše run-ova za 32-bitni indeks");
        let k = initial.len().max(1).next_power_of_two();
        let mut tree = Self { losers: vec![EXHAUSTED; k], winner: EXHAUSTED, k, comparison_count: 0 };
        if initial.is_empty() {
            return tree;
        }

        // Turnir odozdo nagore: win[k+i] su listovi, win[t] pobednik podstabla t
        let mut win = vec![EXHAUSTED; 2 * k];
        for (i, v) in initial.iter().enumerate() {
            if let Some(v) = v {
                win[k + i] = pack(*v, i);
            }
        }
        for t in (1..k).rev() {
            let (a, b) = (win[2 * t], win[2 * t + 1]);
            if COUNT {
                tree.comparison_count += 1;
            }
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            tree.losers[t] = hi;
            win[t] = lo;
        }
        tree.winner = win[1]; // za k=1 ovo je sam list 0
        tree
    }

    /// Trenutni minimum: (vrednost, run) ili None kad su svi run-ovi iscrpljeni.
    #[inline(always)]
    pub fn peek(&self) -> Option<(u32, usize)> {
        if self.winner == EXHAUSTED { None } else { Some(unpack(self.winner)) }
    }

    /// Zamenjuje pobednika sledećom vrednošću iz istog run-a (None = run iscrpljen).
    #[inline(always)]
    pub fn replace_winner(&mut self, next: Option<u32>) {
        let (_, run) = unpack(self.winner);
        let mut cur = match next {
            Some(v) => pack(v, run),
            None => EXHAUSTED,
        };
        let mut t = (run + self.k) >> 1;
        while t > 0 {
            let l = self.losers[t];
            if COUNT {
                self.comparison_count += 1;
            }
            // Manji ključ nastavlja naviše, veći ostaje kao gubitnik
            let (lo, hi) = if l < cur { (l, cur) } else { (cur, l) };
            self.losers[t] = hi;
            cur = lo;
            t >>= 1;
        }
        self.winner = cur;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;

    fn merge(runs: &[Vec<u32>]) -> Vec<u32> {
        let mut pos = vec![0usize; runs.len()];
        let initial: Vec<Option<u32>> = runs.iter().map(|r| r.first().copied()).collect();
        let mut t = PackedLoserTree::<true>::new(&initial);
        let mut out = Vec::new();
        while let Some((v, r)) = t.peek() {
            out.push(v);
            pos[r] += 1;
            t.replace_winner(runs[r].get(pos[r]).copied());
        }
        out
    }

    fn reference(runs: &[Vec<u32>]) -> Vec<u32> {
        let mut all: Vec<u32> = runs.iter().flatten().copied().collect();
        all.sort_unstable();
        all
    }

    #[test]
    fn pack_preserves_order() {
        assert!(pack(1, 5) < pack(2, 0));
        assert!(pack(7, 1) < pack(7, 2));
        assert!(pack(u32::MAX, 1000) < EXHAUSTED);
        assert_eq!(unpack(pack(123, 45)), (123, 45));
    }

    #[test]
    fn random_runs_all_counts() {
        let mut rng = StdRng::seed_from_u64(1);
        for n_runs in 1..=20 {
            let runs: Vec<Vec<u32>> = (0..n_runs)
                .map(|_| {
                    let len = rng.gen_range(0..300);
                    let mut r: Vec<u32> = (0..len).map(|_| rng.gen_range(0..50)).collect();
                    r.sort_unstable();
                    r
                })
                .collect();
            assert_eq!(merge(&runs), reference(&runs), "n_runs={}", n_runs);
        }
    }

    #[test]
    fn edge_cases() {
        assert_eq!(merge(&[]), Vec::<u32>::new());
        assert_eq!(merge(&[vec![], vec![], vec![]]), Vec::<u32>::new());
        assert_eq!(merge(&[vec![1, 2, 3]]), vec![1, 2, 3]);
        let maxes = vec![vec![u32::MAX, u32::MAX], vec![0, u32::MAX], vec![]];
        assert_eq!(merge(&maxes), reference(&maxes));
    }

    #[test]
    fn stable_by_run_on_ties() {
        // Jednake vrednosti moraju izlaziti po rastućem indeksu run-a
        let runs = vec![vec![5, 5], vec![5], vec![5, 5, 5]];
        let mut pos = vec![0usize; runs.len()];
        let initial: Vec<Option<u32>> = runs.iter().map(|r| r.first().copied()).collect();
        let mut t = PackedLoserTree::<false>::new(&initial);
        let mut order = Vec::new();
        while let Some((_, r)) = t.peek() {
            order.push(r);
            pos[r] += 1;
            t.replace_winner(runs[r].get(pos[r]).copied());
        }
        assert_eq!(order, vec![0, 0, 1, 2, 2, 2]);
    }

    #[test]
    fn comparisons_are_log2_k_per_element() {
        let runs: Vec<Vec<u32>> = (0..12).map(|i| (0..100).map(|j| j * 12 + i).collect()).collect();
        let initial: Vec<Option<u32>> = runs.iter().map(|r| r.first().copied()).collect();
        let mut t = PackedLoserTree::<true>::new(&initial);
        assert_eq!(t.comparison_count, 15); // k-1 za init, k=16
        let mut pos = vec![0usize; 12];
        let mut n = 0u64;
        while let Some((_, r)) = t.peek() {
            n += 1;
            pos[r] += 1;
            t.replace_winner(runs[r].get(pos[r]).copied());
        }
        assert_eq!(t.comparison_count, 15 + n * 4);
    }
}
