use std::fmt;

pub struct LoserTreePlayer {
    pub value: Option<u32>, // None = iscrpljen run / dumi, Some(v) = aktivan podatak
    pub run_index: usize,   // Uvek jednak indeksu slota
}

pub struct LoserTree {
    pub losers: Vec<usize>,            // losers[0] je pobednik, losers[1..k-1] su gubitnici
    pub players: Vec<LoserTreePlayer>, // Igrači veličine K (Power of 2)
    pub k: usize,                      // Snaga stabla (Power of 2)
    pub actual_runs: usize,            // Stvarni broj aktivnih run-ova na početku
    pub comparison_count: u64,
    pub emitted_elements: u64,
}

impl LoserTree {
    pub fn new(actual_runs: usize) -> Self {
        let k = actual_runs.max(1).next_power_of_two();
        Self {
            losers: vec![k; k],
            players: Vec::with_capacity(k),
            k,
            actual_runs,
            comparison_count: 0,
            emitted_elements: 0,
        }
    }

    /// `initial_values[i]` je prvi element run-a `i` (ili None ako je run prazan).
    /// Drugi član torke (r_idx) se ignoriše — indeks run-a je uvek pozicija u vektoru.
    pub fn init(&mut self, initial_values: Vec<Option<(u32, usize)>>) {
        assert_eq!(
            initial_values.len(),
            self.actual_runs,
            "init: broj početnih vrednosti mora biti jednak actual_runs"
        );

        self.players.clear();
        self.comparison_count = 0;
        self.emitted_elements = 0;

        if self.actual_runs == 0 {
            return;
        }

        // Stvarni run-ovi
        for (i, val_opt) in initial_values.into_iter().enumerate() {
            self.players.push(LoserTreePlayer {
                value: val_opt.map(|(v, _)| v),
                run_index: i,
            });
        }

        // Dopuni dumi neaktivnim igračima do K (Power of 2)
        for i in self.actual_runs..self.k {
            self.players.push(LoserTreePlayer {
                value: None,
                run_index: i,
            });
        }

        // Svi čvorovi prazni (sentinel = k)
        self.losers.iter_mut().for_each(|l| *l = self.k);

        // Gradimo stablo: svaki igrač se penje dok ne naiđe na prazan čvor.
        // Do korena stiže samo poslednji, i on je pravi pobednik.
        for i in 0..self.k {
            self.adjust(i);
        }
    }

    /// true ako igrač `a` pobeđuje igrača `b` (manja vrednost; None gubi; tie-break po run-u).
    #[inline]
    fn beats(&self, a: usize, b: usize) -> bool {
        let (pa, pb) = (&self.players[a], &self.players[b]);
        match (pa.value, pb.value) {
            (Some(va), Some(vb)) => va < vb || (va == vb && pa.run_index < pb.run_index),
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => pa.run_index < pb.run_index,
        }
    }

    #[inline]
    pub fn adjust(&mut self, mut s: usize) {
        let mut t = (s + self.k) / 2;
        while t > 0 {
            let current_loser = self.losers[t];

            // Prazan čvor (samo tokom init-a): parkiraj igrača i stani
            if current_loser == self.k {
                self.losers[t] = s;
                return;
            }

            self.comparison_count += 1;
            if !self.beats(s, current_loser) {
                // Novi pobednik nastavlja naviše, s ostaje kao gubitnik
                self.losers[t] = s;
                s = current_loser;
            }
            t /= 2;
        }
        self.losers[0] = s;
    }

    #[inline]
    pub fn peek_winner(&self) -> Option<(u32, usize)> {
        if self.actual_runs == 0 {
            return None;
        }
        let p = &self.players[self.losers[0]];
        p.value.map(|v| (v, p.run_index))
    }

    #[inline]
    pub fn replace_winner_and_adjust(&mut self, new_val: Option<u32>) {
        if self.actual_runs == 0 {
            return;
        }
        let winner_idx = self.losers[0];
        if self.players[winner_idx].value.is_some() {
            self.emitted_elements += 1;
        }
        self.players[winner_idx].value = new_val;
        self.adjust(winner_idx);
    }
}

impl fmt::Debug for LoserTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== LoserTree Debug State ===")?;
        writeln!(f, "K (Capacity): {}, Actual Runs: {}", self.k, self.actual_runs)?;
        writeln!(f, "Emitted Elements: {}, Comparisons: {}", self.emitted_elements, self.comparison_count)?;
        writeln!(f, "Winner (losers[0]): {}", self.losers[0])?;
        writeln!(f, "Losers Array: {:?}", self.losers)?;
        writeln!(f, "Players:")?;
        for (idx, p) in self.players.iter().enumerate() {
            writeln!(f, "  [{}] RunIdx: {}, Value: {:?}", idx, p.run_index, p.value)?;
        }
        Ok(())
    }
}
