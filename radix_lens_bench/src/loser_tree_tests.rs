#[cfg(test)]
mod tests {
    use super::super::loser_tree::LoserTree;
    use rand::prelude::*;

    struct MockRunReader {
        data: Vec<u32>,
        idx: usize,
    }

    impl MockRunReader {
        fn new(data: Vec<u32>) -> Self {
            Self { data, idx: 0 }
        }

        fn next_val(&mut self) -> Option<u32> {
            if self.idx < self.data.len() {
                let val = self.data[self.idx];
                self.idx += 1;
                Some(val)
            } else {
                None
            }
        }
    }

    fn run_loser_tree_merge(runs: Vec<Vec<u32>>) -> (Vec<u32>, u64, u64) {
        let actual_runs = runs.len();
        let total_expected_len: usize = runs.iter().map(|r| r.len()).sum();

        let mut readers: Vec<MockRunReader> = runs.into_iter().map(MockRunReader::new).collect();
        let mut loser_tree = LoserTree::new(actual_runs);

        let mut initial_vals = Vec::with_capacity(actual_runs);
        for (i, reader) in readers.iter_mut().enumerate() {
            if let Some(val) = reader.next_val() {
                initial_vals.push(Some((val, i)));
            } else {
                initial_vals.push(None);
            }
        }

        loser_tree.init(initial_vals);

        let mut output = Vec::with_capacity(total_expected_len);

        while let Some((min_val, r_idx)) = loser_tree.peek_winner() {
            output.push(min_val);
            let next_val_opt = readers[r_idx].next_val();
            loser_tree.replace_winner_and_adjust(next_val_opt);
        }

        (output, loser_tree.emitted_elements, loser_tree.comparison_count)
    }

    fn verify_result(runs: &[Vec<u32>], output: &[u32], emitted_count: u64) {
        let mut reference: Vec<u32> = runs.iter().flatten().copied().collect();
        reference.sort_unstable();

        // 1. Tačan broj elemenata
        assert_eq!(
            output.len(),
            reference.len(),
            "Neispravan broj elemenata u izlazu!"
        );
        assert_eq!(
            emitted_count as usize,
            reference.len(),
            "Broj emitovanih elemenata se ne poklapa sa dužinom izlaza!"
        );

        // 2. Sortiranost izlaza
        for window in output.windows(2) {
            assert!(
                window[0] <= window[1],
                "Izlazni niz nije sortiran! Pronađen par: {} > {}",
                window[0],
                window[1]
            );
        }

        // 3. Očuvanje multiskupa (potpuna ekvivalencija sa reference)
        assert_eq!(
            output,
            reference.as_slice(),
            "Izlazni niz se ne poklapa sa referentnim sortiranom nizom!"
        );
    }

    #[test]
    fn test_matrix_capacities() {
        let capacities = [1, 2, 3, 4, 5, 7, 8, 9, 12, 15, 16, 17];
        for &k in &capacities {
            let mut runs = Vec::new();
            for r in 0..k {
                let run_data: Vec<u32> = (0..100).map(|i| (i * k + r) as u32).collect();
                runs.push(run_data);
            }

            let (output, emitted, _) = run_loser_tree_merge(runs.clone());
            verify_result(&runs, &output, emitted);
        }
    }

    #[test]
    fn test_edge_case_all_empty() {
        let runs: Vec<Vec<u32>> = vec![vec![], vec![], vec![]];
        let (output, emitted, _) = run_loser_tree_merge(runs.clone());
        verify_result(&runs, &output, emitted);
    }

    #[test]
    fn test_edge_case_single_active_run() {
        let runs: Vec<Vec<u32>> = vec![vec![], vec![1, 3, 5, 7], vec![]];
        let (output, emitted, _) = run_loser_tree_merge(runs.clone());
        verify_result(&runs, &output, emitted);
    }

    #[test]
    fn test_edge_case_asymmetric_lengths() {
        let runs: Vec<Vec<u32>> = vec![
            vec![1; 1000],
            vec![2, 4, 6],
            vec![],
            (0..500).map(|x| x * 2).collect(),
        ];
        let (output, emitted, _) = run_loser_tree_merge(runs.clone());
        verify_result(&runs, &output, emitted);
    }

    #[test]
    fn test_edge_case_u32_max_as_regular_data() {
        // u32::MAX mora se tretirati kao regularan podatak ako je run aktivan
        let runs: Vec<Vec<u32>> = vec![
            vec![10, 20, u32::MAX],
            vec![5, u32::MAX],
            vec![100, 200],
        ];
        let (output, emitted, _) = run_loser_tree_merge(runs.clone());
        verify_result(&runs, &output, emitted);
    }

    #[test]
    fn test_edge_case_heavy_duplicates() {
        let runs: Vec<Vec<u32>> = vec![
            vec![42; 500],
            vec![42; 500],
            vec![42; 500],
        ];
        let (output, emitted, _) = run_loser_tree_merge(runs.clone());
        verify_result(&runs, &output, emitted);
    }

    #[test]
    fn test_deterministic_random_fuzzing() {
        let mut rng = StdRng::seed_from_u64(0xDEADBEEF);

        for _ in 0..100 {
            let num_runs = rng.gen_range(1..=20);
            let mut runs = Vec::new();

            for _ in 0..num_runs {
                let run_len = rng.gen_range(0..=200);
                let mut run_data: Vec<u32> = (0..run_len).map(|_| rng.gen_range(0..1000)).collect();
                run_data.sort_unstable();
                runs.push(run_data);
            }

            let (output, emitted, _) = run_loser_tree_merge(runs.clone());
            verify_result(&runs, &output, emitted);
        }
    }
}