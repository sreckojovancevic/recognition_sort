use clap::{Parser, ValueEnum};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::cell::Cell;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::sync_channel;
use std::thread;
use std::time::{Duration, Instant};
mod loser_tree;
mod packed_loser_tree;
mod radix_sort;
use packed_loser_tree::PackedLoserTree;
use loser_tree::LoserTree;
#[cfg(test)]
mod loser_tree_tests;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Serialize, Deserialize, Debug)]
enum Scenario {
    Ram,
    Streaming,
    External,
    Parallel,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Serialize, Deserialize, Debug)]
enum Distribution {
    Random,
    Sorted,
    Reverse,
    Duplicates,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Serialize, Deserialize, Debug)]
enum RngKind {
    /// rand::StdRng (ChaCha12) — dosadašnji generator, kriptografski
    Chacha,
    /// Xoshiro256++ — brz nekriptografski generator (drugi podaci za isto seme)
    Xoshiro,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Serialize, Deserialize, Debug)]
enum BlockSort {
    /// rayon par_sort_unstable (poređenja)
    Std,
    /// paralelni LSD radix sort, 4 × 8 bita
    Radix,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Serialize, Deserialize, Debug)]
enum MergeStrategy {
    StandardHeap,
    OptimizedHeap,
    LoserTree,
    PackedLoserTree,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "My Radix LENS v1.3.0 (Packed Loser Tree Engine)")]
struct Args {
    #[arg(short, long, value_enum, default_value_t = Scenario::External)]
    scenario: Scenario,

    #[arg(short, long, default_value_t = 100_000_000)]
    elements: usize,

    #[arg(short, long, default_value_t = 32)]
    block_size_mb: usize,

    #[arg(long, default_value_t = 512)]
    read_buffer_kb: usize,

    #[arg(long, default_value_t = 1024)]
    write_buffer_kb: usize,

    #[arg(short, long, default_value = ".")]
    target_dir: String,

    #[arg(short, long, default_value_t = 8)]
    threads: usize,

    #[arg(short, long, default_value_t = 10)]
    repetitions: usize,

    #[arg(short, long, value_enum, default_value_t = Distribution::Random)]
    distribution: Distribution,

    #[arg(long, default_value_t = false)]
    pipeline: bool,

    #[arg(long, default_value_t = 1)]
    pipeline_depth: usize,

    /// Pokreni samo jednu merge strategiju (bez ovog parametra: sve tri)
    #[arg(long, value_enum)]
    strategy: Option<MergeStrategy>,

    /// Posle svakog run fajla i izlaznog fajla pozovi sync_all() (stvarni upis na disk)
    #[arg(long, default_value_t = false)]
    fsync: bool,

    /// Seme generatora: isto seme + isti parametri = bit-identični ulazni podaci
    #[arg(long)]
    seed: Option<u64>,

    /// Broji poređenja ključeva u merge fazi (utiče na vreme — ne porediti sa merenjima bez njega)
    #[arg(long, default_value_t = false)]
    count_ops: bool,

    /// Paralelni merge sa N niti (važi za packed-loser-tree). N=1 = isti paralelni algoritam
    /// sa jednom particijom (kontrola). Bez opcije: postojeći sekvencijalni PackedLoserTree.
    #[arg(long)]
    merge_threads: Option<usize>,

    /// Algoritam sortiranja bloka u pripremi
    #[arg(long, value_enum, default_value_t = BlockSort::Std)]
    block_sort: BlockSort,

    /// Generator slučajnih brojeva za ulazne podatke
    #[arg(long, value_enum, default_value_t = RngKind::Chacha)]
    rng: RngKind,

    /// Samo mikrobenchmark generatora (van pipeline-a): nov naspram ponovo korišćenog bafera
    #[arg(long, default_value_t = false)]
    gen_microbench: bool,

    /// Poređenje sa standardnim sortovima u memoriji (sort, sort_unstable, par_sort, par_sort_unstable, radix)
    #[arg(long, default_value_t = false)]
    compare_std: bool,

    /// Oznaka konfiguracije za zbirni red SUMMARY (za run_compare.ps1)
    #[arg(long)]
    label: Option<String>,

    /// Pauza (sekunde) između metoda u --compare-std, da se mašina smiri
    #[arg(long, default_value_t = 0)]
    pause_secs: u64,

    /// Broj writer niti u pipeline-u (svaka piše svoje run fajlove; +1 živi blok po dodatnoj niti)
    #[arg(long, default_value_t = 1)]
    writers: usize,
}

#[derive(Serialize, Deserialize, Debug)]
struct BenchmarkReport {
    version: String,
    scenario: String,
    target_dir: String,
    elements: usize,
    block_size_mb: usize,
    read_buffer_kb: usize,
    write_buffer_kb: usize,
    threads: usize,
    repetitions: usize,
    distribution: String,
    strategy: String,
    total_runs: usize,
    median_gen_duration_ms: u128,
    median_sort_duration_ms: u128,
    median_io_write_duration_ms: u128,
    median_prep_wall_ms: u128,
    median_total_excl_verify_ms: u128,
    max_inflight_blocks: usize,
    median_merge_init_duration_ms: u128,
    median_merge_core_duration_ms: u128,
    median_final_write_ms: u128,
    median_verify_duration_ms: u128,
    median_total_duration_ms: u128,
    median_peak_rss_mb: f64,
    is_correct: bool,
    notes: String,
}

thread_local! {
    /// Brojač poređenja ključeva u merge fazi (koristi se samo uz --count-ops)
    static CMP_COUNT: Cell<u64> = const { Cell::new(0) };
}

/// Element heap-a. COUNT=true broji svako poređenje; COUNT=false je bez ikakvog troška.
struct HeapEl<const COUNT: bool> {
    value: u32,
    run_index: usize,
}

impl<const COUNT: bool> PartialEq for HeapEl<COUNT> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && self.run_index == other.run_index
    }
}
impl<const COUNT: bool> Eq for HeapEl<COUNT> {}
impl<const COUNT: bool> PartialOrd for HeapEl<COUNT> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<const COUNT: bool> Ord for HeapEl<COUNT> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        if COUNT {
            CMP_COUNT.with(|c| c.set(c.get() + 1));
        }
        // Obrnuto (min-heap); tie-break po run-u, isto kao LoserTree
        other.value.cmp(&self.value)
            .then_with(|| other.run_index.cmp(&self.run_index))
    }
}

// -------------------------------------------------------------------------
// IZLAZNI BAFER ZA MERGE
// -------------------------------------------------------------------------
struct OutSink {
    batch: Vec<u32>,
    cap: usize,
    writer: BufWriter<File>,
    writes: u64,
}

impl OutSink {
    fn new(writer: BufWriter<File>, cap: usize) -> Self {
        Self { batch: Vec::with_capacity(cap), cap, writer, writes: 0 }
    }

    #[inline]
    fn push(&mut self, v: u32) {
        self.batch.push(v);
        if self.batch.len() >= self.cap {
            self.flush_batch();
        }
    }

    #[inline(never)]
    fn flush_batch(&mut self) {
        if self.batch.is_empty() {
            return;
        }
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(self.batch.as_ptr() as *const u8, self.batch.len() * std::mem::size_of::<u32>())
        };
        self.writer.write_all(bytes).unwrap();
        self.batch.clear();
        self.writes += 1;
    }
}

#[derive(Default, Clone, Copy)]
struct WriterStats {
    total: Duration,  // ceo životni vek writer petlje
    wait: Duration,   // čekanje na sledeći sortiran blok (recv)
    create: Duration, // File::create + BufWriter
    write: Duration,  // write_all
    flush: Duration,  // flush (+ sync_all uz --fsync)
    free: Duration,   // drop(block) — oslobađanje 32 MiB
    close: Duration,  // drop(writer) — CloseHandle
    alloc: Duration,  // zbir alokacija blokova u generatoru (druga nit)
}

struct MergeResult {
    init: Duration,
    core: Duration,
    emitted: u64,
    refills: u64,
    comparisons: u64,
    /// Najveća particija / prosečna particija (1.0 = savršeno ravnomerno; samo paralelni merge)
    max_part_ratio: f64,
    /// Batch upisi obavljeni unutar merge funkcije (paralelni merge ima sopstvene izlaze)
    inner_batch_writes: u64,
    /// Paralelni merge: izlaz po delovima (redom = globalni redosled). Prazno = jedan izlazni fajl.
    output_parts: Vec<PathBuf>,
    /// Paralelni merge: da li svaki deo ima tačno očekivan broj bajtova
    parts_complete: bool,
}

fn open_readers(run_files: &[PathBuf], read_buffer_bytes: usize) -> Vec<BufferedRunReader> {
    run_files.iter().map(|p| BufferedRunReader::new(File::open(p).unwrap(), read_buffer_bytes)).collect()
}

fn merge_standard_heap<const C: bool>(run_files: &[PathBuf], rb: usize, sink: &mut OutSink) -> MergeResult {
    CMP_COUNT.with(|c| c.set(0));
    let t_init = Instant::now();
    let mut readers = open_readers(run_files, rb);
    let mut heap: BinaryHeap<HeapEl<C>> = BinaryHeap::with_capacity(readers.len());
    for (i, reader) in readers.iter_mut().enumerate() {
        if let Some(val) = reader.next_val() {
            heap.push(HeapEl { value: val, run_index: i });
        }
    }
    let init = t_init.elapsed();

    let t_core = Instant::now();
    let mut emitted = 0u64;
    while let Some(HeapEl { value, run_index }) = heap.pop() {
        emitted += 1;
        sink.push(value);
        if let Some(val) = readers[run_index].next_val() {
            heap.push(HeapEl { value: val, run_index });
        }
    }
    let core = t_core.elapsed();
    MergeResult {
        init, core, emitted,
        refills: readers.iter().map(|r| r.refill_count).sum(),
        comparisons: CMP_COUNT.with(|c| c.get()),
        max_part_ratio: 1.0,
        inner_batch_writes: 0,
        output_parts: Vec::new(),
        parts_complete: true,
    }
}

fn merge_optimized_heap<const C: bool>(run_files: &[PathBuf], rb: usize, sink: &mut OutSink) -> MergeResult {
    CMP_COUNT.with(|c| c.set(0));
    let t_init = Instant::now();
    let mut readers = open_readers(run_files, rb);
    let mut heap: BinaryHeap<HeapEl<C>> = BinaryHeap::with_capacity(readers.len());
    for (i, reader) in readers.iter_mut().enumerate() {
        if let Some(val) = reader.next_val() {
            heap.push(HeapEl { value: val, run_index: i });
        }
    }
    let init = t_init.elapsed();

    let t_core = Instant::now();
    let mut emitted = 0u64;
    while let Some(mut root) = heap.peek_mut() {
        emitted += 1;
        let r_idx = root.run_index;
        sink.push(root.value);
        if let Some(val) = readers[r_idx].next_val() {
            *root = HeapEl { value: val, run_index: r_idx };
        } else {
            std::collections::binary_heap::PeekMut::pop(root);
        }
    }
    let core = t_core.elapsed();
    MergeResult {
        init, core, emitted,
        refills: readers.iter().map(|r| r.refill_count).sum(),
        comparisons: CMP_COUNT.with(|c| c.get()),
        max_part_ratio: 1.0,
        inner_batch_writes: 0,
        output_parts: Vec::new(),
        parts_complete: true,
    }
}

/// LoserTree uvek broji poređenja (u32 inkrement u adjust, kao i u svim ranijim merenjima).
fn merge_loser_tree(run_files: &[PathBuf], rb: usize, sink: &mut OutSink) -> MergeResult {
    let t_init = Instant::now();
    let mut readers = open_readers(run_files, rb);
    let mut loser_tree = LoserTree::new(readers.len());
    let initial_vals: Vec<Option<(u32, usize)>> =
        readers.iter_mut().enumerate().map(|(i, r)| r.next_val().map(|v| (v, i))).collect();
    loser_tree.init(initial_vals);
    let init = t_init.elapsed();

    let t_core = Instant::now();
    let mut emitted = 0u64;
    while let Some((min_val, r_idx)) = loser_tree.peek_winner() {
        emitted += 1;
        sink.push(min_val);
        let next_val_opt = readers[r_idx].next_val();
        loser_tree.replace_winner_and_adjust(next_val_opt);
    }
    let core = t_core.elapsed();
    MergeResult {
        init, core, emitted,
        refills: readers.iter().map(|r| r.refill_count).sum(),
        comparisons: loser_tree.comparison_count,
        max_part_ratio: 1.0,
        inner_batch_writes: 0,
        output_parts: Vec::new(),
        parts_complete: true,
    }
}

// -------------------------------------------------------------------------
// BUFFERED RUN READER
// -------------------------------------------------------------------------
/// Čita u32 vrednosti (native endian) iz toka bajtova.
/// Ne pretpostavlja da `read()` vraća broj bajtova deljiv sa 4: nepotpuna
/// vrednost na kraju jednog čitanja čuva se u `carry` i dopunjuje sledećim.
struct BufferedRunReader<R: Read = BufReader<File>> {
    reader: R,
    buffer: Vec<u32>,
    buffer_idx: usize,
    buffer_len: usize,
    carry: [u8; 4],
    carry_len: usize,
    refill_count: u64,
}

impl BufferedRunReader<BufReader<File>> {
    fn new(file: File, capacity_bytes: usize) -> Self {
        Self::from_reader(BufReader::with_capacity(capacity_bytes, file), capacity_bytes)
    }
}

impl<R: Read> BufferedRunReader<R> {
    fn from_reader(reader: R, capacity_bytes: usize) -> Self {
        let u32_capacity = std::cmp::max(capacity_bytes / std::mem::size_of::<u32>(), 1024);
        Self {
            reader,
            buffer: vec![0; u32_capacity],
            buffer_idx: 0,
            buffer_len: 0,
            carry: [0; 4],
            carry_len: 0,
            refill_count: 0,
        }
    }

    #[inline]
    fn next_val(&mut self) -> Option<u32> {
        if self.buffer_idx >= self.buffer_len && !self.refill() {
            return None;
        }
        let val = self.buffer[self.buffer_idx];
        self.buffer_idx += 1;
        Some(val)
    }

    /// Puni bafer sa bar jednom celom vrednošću. false = čist kraj toka.
    /// Panic na grešku čitanja ili na kraj toka usred vrednosti (oštećen run).
    #[inline(never)]
    fn refill(&mut self) -> bool {
        const W: usize = std::mem::size_of::<u32>();
        self.refill_count += 1;
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(self.buffer.as_mut_ptr() as *mut u8, self.buffer.len() * W)
        };

        // Nepotpuna vrednost iz prethodnog čitanja ide na početak
        bytes[..self.carry_len].copy_from_slice(&self.carry[..self.carry_len]);
        let mut filled = self.carry_len;
        self.carry_len = 0;

        // Čitaj dok nemamo bar jednu celu vrednost ili do kraja toka
        while filled < W {
            match self.reader.read(&mut bytes[filled..]) {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => panic!("Greška pri čitanju run fajla: {}", e),
            }
        }

        let whole = filled / W;
        let rem = filled % W;
        if whole == 0 {
            if rem != 0 {
                panic!("Run fajl se završava usred vrednosti ({} viška bajtova)", rem);
            }
            self.buffer_len = 0;
            self.buffer_idx = 0;
            return false;
        }

        // Ostatak (0–3 bajta) čuvamo za sledeće punjenje
        self.carry[..rem].copy_from_slice(&bytes[whole * W..filled]);
        self.carry_len = rem;
        self.buffer_len = whole;
        self.buffer_idx = 0;
        true
    }
}

#[cfg(test)]
mod buffered_reader_tests {
    use super::BufferedRunReader;
    use std::io::Read;

    /// Read koji vraća nasumično kratke komade (1–7 bajtova), nezavisno od poravnanja
    struct ChunkyReader { data: Vec<u8>, pos: usize, step: usize }
    impl Read for ChunkyReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.pos >= self.data.len() { return Ok(0); }
            self.step = self.step % 7 + 1;
            let n = self.step.min(buf.len()).min(self.data.len() - self.pos);
            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    fn bytes_of(v: &[u32]) -> Vec<u8> { v.iter().flat_map(|x| x.to_ne_bytes()).collect() }

    #[test]
    fn partial_reads_preserve_all_values() {
        let vals: Vec<u32> = (0..10_000u32).map(|i| i.wrapping_mul(2654435761)).collect();
        for cap in [4usize, 16, 4096] {
            let mut r = BufferedRunReader::from_reader(ChunkyReader { data: bytes_of(&vals), pos: 0, step: 0 }, cap);
            let mut out = Vec::new();
            while let Some(v) = r.next_val() { out.push(v); }
            assert_eq!(out, vals, "cap={}", cap);
        }
    }

    /// Read koji svako drugo pozivanje vraća Interrupted, a inače 3 bajta
    struct InterruptingReader { data: Vec<u8>, pos: usize, calls: usize }
    impl Read for InterruptingReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.calls += 1;
            if self.calls % 2 == 1 {
                return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "test"));
            }
            let n = 3usize.min(buf.len()).min(self.data.len() - self.pos);
            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    #[test]
    fn interrupted_is_retried() {
        let vals: Vec<u32> = (0..1000u32).map(|i| i ^ 0xA5A5_5A5A).collect();
        let mut r = BufferedRunReader::from_reader(InterruptingReader { data: bytes_of(&vals), pos: 0, calls: 0 }, 4096);
        let mut out = Vec::new();
        while let Some(v) = r.next_val() { out.push(v); }
        assert_eq!(out, vals);
    }

    struct FailingReader;
    impl Read for FailingReader {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(std::io::ErrorKind::Other, "disk"))
        }
    }

    #[test]
    #[should_panic(expected = "Greška pri čitanju")]
    fn other_errors_panic() {
        let mut r = BufferedRunReader::from_reader(FailingReader, 4096);
        r.next_val();
    }

    #[test]
    fn empty_stream_returns_none() {
        let mut r = BufferedRunReader::from_reader(ChunkyReader { data: vec![], pos: 0, step: 0 }, 4096);
        assert_eq!(r.next_val(), None);
        assert_eq!(r.next_val(), None);
    }

    #[test]
    #[should_panic(expected = "usred vrednosti")]
    fn truncated_value_panics() {
        let mut data = bytes_of(&[1, 2, 3]);
        data.pop();
        let mut r = BufferedRunReader::from_reader(ChunkyReader { data, pos: 0, step: 0 }, 4096);
        while r.next_val().is_some() {}
    }
}

/// Isti algoritam kao LoserTree; jedina razlika je reprezentacija ključa (videti packed_loser_tree.rs).
fn merge_packed_loser_tree<const C: bool>(run_files: &[PathBuf], rb: usize, sink: &mut OutSink) -> MergeResult {
    let t_init = Instant::now();
    let mut readers = open_readers(run_files, rb);
    let initial: Vec<Option<u32>> = readers.iter_mut().map(|r| r.next_val()).collect();
    let mut tree = PackedLoserTree::<C>::new(&initial);
    let init = t_init.elapsed();

    let t_core = Instant::now();
    let mut emitted = 0u64;
    while let Some((min_val, r_idx)) = tree.peek() {
        emitted += 1;
        sink.push(min_val);
        let next = readers[r_idx].next_val();
        tree.replace_winner(next);
    }
    let core = t_core.elapsed();
    MergeResult {
        init, core, emitted,
        refills: readers.iter().map(|r| r.refill_count).sum(),
        comparisons: tree.comparison_count,
        max_part_ratio: 1.0,
        inner_batch_writes: 0,
        output_parts: Vec::new(),
        parts_complete: true,
    }
}

// -------------------------------------------------------------------------
// PARALELNI MERGE PO OPSEZIMA KLJUČEVA
// -------------------------------------------------------------------------
#[cfg(windows)]
fn read_u32_at(f: &File, idx: u64) -> u32 {
    use std::os::windows::fs::FileExt;
    let mut b = [0u8; 4];
    let mut got = 0;
    while got < 4 {
        let n = f.seek_read(&mut b[got..], idx * 4 + got as u64).unwrap();
        assert!(n > 0, "neočekivan kraj run fajla");
        got += n;
    }
    u32::from_ne_bytes(b)
}

#[cfg(unix)]
fn read_u32_at(f: &File, idx: u64) -> u32 {
    use std::os::unix::fs::FileExt;
    let mut b = [0u8; 4];
    f.read_exact_at(&mut b, idx * 4).unwrap();
    u32::from_ne_bytes(b)
}

/// Prvi indeks i u sortiranom run-u dužine n za koji važi a[i] >= v.
fn lower_bound_file(f: &File, n: u64, v: u32) -> u64 {
    let (mut lo, mut hi) = (0u64, n);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if read_u32_at(f, mid) < v { lo = mid + 1; } else { hi = mid; }
    }
    lo
}

/// Paralelni merge: izlaz se deli po vrednostima na `threads` opsega.
/// Svaka particija j se upisuje u svoj fajl `<out_path>.part{j}`; delovi redom čine
/// globalno sortiran izlaz (nema konkurentnog upisa u jedan unapred proširen fajl).
/// Granice se biraju iz uzoraka svih run-ova; za svaki run i granicu binarnom pretragom
/// se nalazi pozicija. Sve jednake vrednosti padaju u istu particiju, pa je redosled
/// (vrednost, run) isti kao kod sekvencijalnog merge-a. Svaka nit koristi PackedLoserTree
/// i upisuje svoj opseg direktno na njegovo mesto u izlaznom fajlu.
fn merge_parallel_packed<const C: bool>(
    run_files: &[PathBuf],
    read_buffer_bytes: usize,
    write_buffer_bytes: usize,
    out_path: &Path,
    threads: usize,
) -> MergeResult {
    const SAMPLES_PER_RUN: u64 = 256;
    let t = threads.max(1);
    let t_init = Instant::now();

    let files: Vec<File> = run_files.iter().map(|p| File::open(p).unwrap()).collect();
    let lens: Vec<u64> = files.iter().map(|f| {
        let bytes = f.metadata().unwrap().len();
        assert!(bytes % 4 == 0, "run fajl nije poravnat na u32");
        bytes / 4
    }).collect();
    let total: u64 = lens.iter().sum();

    // Uzorci → granice (t-1 vrednosti)
    let mut samples: Vec<u32> = Vec::new();
    for (f, &n) in files.iter().zip(&lens) {
        if n == 0 { continue; }
        let k = SAMPLES_PER_RUN.min(n);
        for i in 0..k {
            samples.push(read_u32_at(f, i * n / k));
        }
    }
    samples.sort_unstable();
    let splitters: Vec<u32> = if samples.is_empty() {
        Vec::new()
    } else {
        (1..t).map(|j| samples[j * samples.len() / t]).collect()
    };
    let parts = splitters.len() + 1;

    // bounds[r][j] = početak particije j u run-u r; bounds[r][parts] = kraj
    let bounds: Vec<Vec<u64>> = files.iter().zip(&lens).map(|(f, &n)| {
        let mut b = Vec::with_capacity(parts + 1);
        b.push(0);
        for &v in &splitters {
            b.push(lower_bound_file(f, n, v));
        }
        b.push(n);
        b
    }).collect();
    drop(files);

    let part_len: Vec<u64> = (0..parts)
        .map(|j| bounds.iter().map(|b| b[j + 1] - b[j]).sum())
        .collect();
    debug_assert_eq!(part_len.iter().sum::<u64>(), total);
    let avg = total as f64 / parts as f64;
    let max_part_ratio = if avg > 0.0 { *part_len.iter().max().unwrap() as f64 / avg } else { 1.0 };

    let part_paths: Vec<PathBuf> = (0..parts)
        .map(|j| {
            let mut name = out_path.as_os_str().to_owned();
            name.push(format!(".part{}", j));
            PathBuf::from(name)
        })
        .collect();

    // Memorija bafera se deli na niti, da ukupni RSS ostane uporediv
    let rb = (read_buffer_bytes / t).max(64 * 1024);
    let wb = (write_buffer_bytes / t).max(256 * 1024);
    let init = t_init.elapsed();

    let t_core = Instant::now();
    let results: Vec<(u64, u64, u64, u64)> = thread::scope(|sc| {
        let handles: Vec<_> = (0..parts).map(|j| {
            let bounds = &bounds;
            let part_path = &part_paths[j];
            sc.spawn(move || {
                let mut readers: Vec<BufferedRunReader<std::io::Take<File>>> = run_files.iter().enumerate().map(|(r, p)| {
                    let mut f = File::open(p).unwrap();
                    f.seek(SeekFrom::Start(bounds[r][j] * 4)).unwrap();
                    BufferedRunReader::from_reader(f.take((bounds[r][j + 1] - bounds[r][j]) * 4), rb)
                }).collect();

                let of = File::create(part_path).unwrap();
                let mut sink = OutSink::new(BufWriter::with_capacity(wb, of), wb / 4);

                let initial: Vec<Option<u32>> = readers.iter_mut().map(|r| r.next_val()).collect();
                let mut tree = PackedLoserTree::<C>::new(&initial);
                let mut emitted = 0u64;
                while let Some((v, r)) = tree.peek() {
                    emitted += 1;
                    sink.push(v);
                    let next = readers[r].next_val();
                    tree.replace_winner(next);
                }
                sink.flush_batch();
                sink.writer.flush().unwrap();
                (emitted, readers.iter().map(|r| r.refill_count).sum(), tree.comparison_count, sink.writes)
            })
        }).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let core = t_core.elapsed();

    // Kontrola: svaki deo mora imati tačno part_len[j] vrednosti
    let parts_complete = part_paths.iter().zip(&part_len).all(|(p, &n)| {
        std::fs::metadata(p).map(|m| m.len() == n * 4).unwrap_or(false)
    });

    MergeResult {
        init,
        core,
        emitted: results.iter().map(|r| r.0).sum(),
        refills: results.iter().map(|r| r.1).sum(),
        comparisons: results.iter().map(|r| r.2).sum(),
        max_part_ratio,
        inner_batch_writes: results.iter().map(|r| r.3).sum(),
        output_parts: part_paths,
        parts_complete,
    }
}

#[cfg(test)]
mod parallel_merge_tests {
    use super::*;

    fn write_run(path: &Path, data: &[u32]) {
        let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_ne_bytes()).collect();
        std::fs::write(path, bytes).unwrap();
    }

    fn read_all(path: &Path) -> Vec<u32> {
        std::fs::read(path).unwrap().chunks_exact(4).map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]])).collect()
    }

    fn check(runs: Vec<Vec<u32>>, threads: usize, tag: &str) {
        let dir = std::env::temp_dir().join(format!("rl_pm_{}_{}_{}", std::process::id(), tag, threads));
        std::fs::create_dir_all(&dir).unwrap();
        let paths: Vec<PathBuf> = runs.iter().enumerate().map(|(i, r)| {
            let p = dir.join(format!("run_{}.bin", i));
            write_run(&p, r);
            p
        }).collect();
        let out = dir.join("out.bin");
        File::create(&out).unwrap();
        let mr = merge_parallel_packed::<false>(&paths, 4096, 4096, &out, threads);
        let mut expected: Vec<u32> = runs.iter().flatten().copied().collect();
        expected.sort_unstable();
        assert!(mr.parts_complete, "{} t={}", tag, threads);
        assert!(!mr.output_parts.is_empty());
        assert_eq!(mr.emitted as usize, expected.len(), "{} t={}", tag, threads);
        let got: Vec<u32> = mr.output_parts.iter().flat_map(|p| read_all(p)).collect();
        assert_eq!(got, expected, "{} t={}", tag, threads);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn sorted_runs(rng: &mut StdRng, n_runs: usize, max_len: usize, modulo: u32) -> Vec<Vec<u32>> {
        (0..n_runs).map(|_| {
            let len = rng.gen_range(0..max_len);
            let mut r: Vec<u32> = (0..len).map(|_| rng.gen::<u32>() % modulo).collect();
            r.sort_unstable();
            r
        }).collect()
    }

    #[test]
    fn matches_sequential_random() {
        let mut rng = StdRng::seed_from_u64(11);
        for &t in &[1usize, 2, 3, 8] {
            check(sorted_runs(&mut rng, 12, 5000, u32::MAX), t, "rand");
        }
    }

    #[test]
    fn heavy_duplicates_and_edges() {
        let mut rng = StdRng::seed_from_u64(12);
        for &t in &[2usize, 8] {
            check(sorted_runs(&mut rng, 7, 3000, 5), t, "dup");
            check(vec![vec![], vec![], vec![]], t, "empty");
            check(vec![vec![u32::MAX; 100], vec![0, u32::MAX]], t, "max");
            check(vec![vec![42; 1000]], t, "one");
        }
    }
}

#[inline]
fn sort_block(block: &mut Vec<u32>, how: BlockSort, scratch: &mut Vec<u32>) {
    match how {
        BlockSort::Std => block.par_sort_unstable(),
        BlockSort::Radix => radix_sort::radix_sort_u32(block, scratch),
    }
}

fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Seme bloka zavisi samo od glavnog semena i početnog indeksa bloka (ne od niti ni redosleda).
fn block_seed(seed: u64, start_idx: usize) -> u64 {
    splitmix64(seed ^ splitmix64(start_idx as u64))
}

fn fill_block<R: Rng>(rng: &mut R, start_idx: usize, count: usize, total_n: usize, dist: Distribution) -> Vec<u32> {
    let data = Vec::with_capacity(count);
    fill_block_into(rng, data, start_idx, count, total_n, dist)
}

/// Popunjava već alociran (prazan) Vec. Odvojeno od alokacije da bi se alokacija merila zasebno.
fn fill_block_into<R: Rng>(rng: &mut R, mut data: Vec<u32>, start_idx: usize, count: usize, total_n: usize, dist: Distribution) -> Vec<u32> {
    match dist {
        Distribution::Random => { for _ in 0..count { data.push(rng.gen()); } }
        Distribution::Sorted => { for i in 0..count { data.push((start_idx + i) as u32); } }
        Distribution::Reverse => {
            for i in 0..count {
                let global_idx = start_idx + i;
                let val = if total_n > global_idx { total_n - 1 - global_idx } else { 0 };
                data.push(val as u32);
            }
        }
        Distribution::Duplicates => { for _ in 0..count { data.push((rng.gen::<u32>() % 100) * 1000); } }
    }
    data
}

/// Xoshiro256++ (Blackman & Vigna). Stanje se puni iz splitmix64, kako preporučuju autori.
struct Xoshiro256pp {
    s: [u64; 4],
}

impl Xoshiro256pp {
    fn from_seed_u64(seed: u64) -> Self {
        let mut x = seed;
        let mut next = || {
            x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
            splitmix64(x)
        };
        let s = [next(), next(), next(), next()];
        Self { s }
    }
}

impl rand::RngCore for Xoshiro256pp {
    #[inline]
    fn next_u64(&mut self) -> u64 {
        let s = &mut self.s;
        let result = s[0].wrapping_add(s[3]).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }
    #[inline]
    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32 // gornji bitovi su najkvalitetniji
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let v = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&v[..chunk.len()]);
        }
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

/// Isto kao generate_block_data, uz izmereno trajanje same alokacije (Vec::with_capacity).
/// Page fault-ovi pri prvom upisu nisu deo ovog vremena — oni padaju u popunjavanje.
fn generate_block_data_timed(start_idx: usize, count: usize, total_n: usize, dist: Distribution, seed: Option<u64>, rng: RngKind) -> (Vec<u32>, Duration) {
    let t = Instant::now();
    let data: Vec<u32> = Vec::with_capacity(count);
    let alloc = t.elapsed();
    let v = match (rng, seed) {
        (RngKind::Chacha, Some(s)) => fill_block_into(&mut StdRng::seed_from_u64(block_seed(s, start_idx)), data, start_idx, count, total_n, dist),
        (RngKind::Chacha, None) => fill_block_into(&mut rand::thread_rng(), data, start_idx, count, total_n, dist),
        (RngKind::Xoshiro, Some(s)) => fill_block_into(&mut Xoshiro256pp::from_seed_u64(block_seed(s, start_idx)), data, start_idx, count, total_n, dist),
        (RngKind::Xoshiro, None) => fill_block_into(&mut Xoshiro256pp::from_seed_u64(rand::random()), data, start_idx, count, total_n, dist),
    };
    (v, alloc)
}

fn generate_block_data(start_idx: usize, count: usize, total_n: usize, dist: Distribution, seed: Option<u64>, rng: RngKind) -> Vec<u32> {
    match (rng, seed) {
        (RngKind::Chacha, Some(s)) => fill_block(&mut StdRng::seed_from_u64(block_seed(s, start_idx)), start_idx, count, total_n, dist),
        (RngKind::Chacha, None) => fill_block(&mut rand::thread_rng(), start_idx, count, total_n, dist),
        (RngKind::Xoshiro, Some(s)) => fill_block(&mut Xoshiro256pp::from_seed_u64(block_seed(s, start_idx)), start_idx, count, total_n, dist),
        (RngKind::Xoshiro, None) => fill_block(&mut Xoshiro256pp::from_seed_u64(rand::random()), start_idx, count, total_n, dist),
    }
}

#[cfg(test)]
mod generator_tests {
    use super::*;

    #[test]
    fn same_seed_same_block() {
        for dist in [Distribution::Random, Distribution::Duplicates, Distribution::Sorted, Distribution::Reverse] {
            let a = generate_block_data(1000, 5000, 100_000, dist, Some(42), RngKind::Chacha);
            let b = generate_block_data(1000, 5000, 100_000, dist, Some(42), RngKind::Chacha);
            assert_eq!(a, b);
        }
    }

    #[test]
    fn different_block_or_seed_differs() {
        let a = generate_block_data(0, 5000, 100_000, Distribution::Random, Some(42), RngKind::Chacha);
        let b = generate_block_data(5000, 5000, 100_000, Distribution::Random, Some(42), RngKind::Chacha);
        let c = generate_block_data(0, 5000, 100_000, Distribution::Random, Some(43), RngKind::Chacha);
        assert_ne!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn block_independent_of_generation_order() {
        // Blok generisan "kasnije" mora biti isti kao kad se generiše prvi
        let _ = generate_block_data(0, 5000, 100_000, Distribution::Random, Some(7), RngKind::Chacha);
        let late = generate_block_data(10_000, 5000, 100_000, Distribution::Random, Some(7), RngKind::Chacha);
        let alone = generate_block_data(10_000, 5000, 100_000, Distribution::Random, Some(7), RngKind::Chacha);
        assert_eq!(late, alone);
    }

    #[test]
    fn xoshiro_deterministic_and_distinct() {
        let a = generate_block_data(1000, 50_000, 1_000_000, Distribution::Random, Some(42), RngKind::Xoshiro);
        let b = generate_block_data(1000, 50_000, 1_000_000, Distribution::Random, Some(42), RngKind::Xoshiro);
        let c = generate_block_data(51_000, 50_000, 1_000_000, Distribution::Random, Some(42), RngKind::Xoshiro);
        let d = generate_block_data(1000, 50_000, 1_000_000, Distribution::Random, Some(42), RngKind::Chacha);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d, "xoshiro i chacha daju različite podatke za isto seme");
        let (t, _) = generate_block_data_timed(1000, 50_000, 1_000_000, Distribution::Random, Some(42), RngKind::Xoshiro);
        assert_eq!(a, t);
    }

    #[test]
    fn xoshiro_reference_vector() {
        // Referentni izlaz Xoshiro256++ za stanje [1, 2, 3, 4] (iz originalne C implementacije)
        use rand::RngCore;
        let mut r = Xoshiro256pp { s: [1, 2, 3, 4] };
        let expected: [u64; 4] = [41943041, 58720359, 3588806011781223, 3591011842654386];
        for e in expected {
            assert_eq!(r.next_u64(), e);
        }
    }

    #[test]
    fn xoshiro_roughly_uniform() {
        let v = generate_block_data(0, 1_000_000, 1_000_000, Distribution::Random, Some(7), RngKind::Xoshiro);
        let mut buckets = [0u32; 16];
        for &x in &v { buckets[(x >> 28) as usize] += 1; }
        for &b in &buckets {
            assert!((b as f64 - 62_500.0).abs() < 1_500.0, "neravnomerno: {:?}", buckets);
        }
    }

    #[test]
    fn heap_counter_only_when_enabled() {
        CMP_COUNT.with(|c| c.set(0));
        let a = HeapEl::<false> { value: 1, run_index: 0 };
        let b = HeapEl::<false> { value: 2, run_index: 1 };
        let _ = a.cmp(&b);
        assert_eq!(CMP_COUNT.with(|c| c.get()), 0);
        let a = HeapEl::<true> { value: 1, run_index: 0 };
        let b = HeapEl::<true> { value: 2, run_index: 1 };
        let _ = a.cmp(&b);
        let _ = b.cmp(&a);
        assert_eq!(CMP_COUNT.with(|c| c.get()), 2);
    }
}

#[cfg(target_os = "windows")]
fn get_process_rss_mb() -> f64 {
    use std::mem::MaybeUninit;
    #[link(name = "psapi")]
    extern "system" {
        fn GetProcessMemoryInfo(hProcess: *mut std::ffi::c_void, psmemcounters: *mut PROCESS_MEMORY_COUNTERS, cb: u32) -> i32;
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
    }
    #[repr(C)]
    #[allow(non_snake_case)]
    struct PROCESS_MEMORY_COUNTERS {
        cb: u32, PageFaultCount: u32, PeakWorkingSetSize: usize, WorkingSetSize: usize,
        QuotaPeakPagedPoolUsage: usize, QuotaPagedPoolUsage: usize, QuotaPeakNonPagedPoolUsage: usize,
        QuotaNonPagedPoolUsage: usize, PagefileUsage: usize, PeakPagefileUsage: usize,
    }
    let mut counters = MaybeUninit::<PROCESS_MEMORY_COUNTERS>::uninit();
    unsafe {
        let ptr = counters.as_mut_ptr();
        (*ptr).cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(GetCurrentProcess(), ptr as *mut _, (*ptr).cb) != 0 {
            return (counters.assume_init().WorkingSetSize as f64) / (1024.0 * 1024.0);
        }
    }
    0.0
}

#[cfg(not(target_os = "windows"))]
fn get_process_rss_mb() -> f64 {
    if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
        let parts: Vec<&str> = statm.split_whitespace().collect();
        if parts.len() > 1 {
            if let Ok(pages) = parts[1].parse::<f64>() {
                return (pages * 4.0) / 1024.0;
            }
        }
    }
    0.0
}

/// Pozadinska nit koja uzorkuje RSS svakih 5 ms i pamti maksimum.
struct RssSampler {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<thread::JoinHandle<f64>>,
}

impl RssSampler {
    fn start() -> Self {
        use std::sync::atomic::{AtomicBool, Ordering as AO};
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let s2 = stop.clone();
        let handle = thread::spawn(move || {
            let mut peak = get_process_rss_mb();
            while !s2.load(AO::Relaxed) {
                peak = peak.max(get_process_rss_mb());
                thread::sleep(Duration::from_millis(5));
            }
            peak.max(get_process_rss_mb())
        });
        Self { stop, handle: Some(handle) }
    }

    fn finish(mut self) -> f64 {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        self.handle.take().unwrap().join().unwrap()
    }
}

/// Multiset otisak: zbir (wrapping) heševa svih vrednosti. Ne zavisi od redosleda,
/// pa je isti za ulaz (run fajlovi) i izlaz ako su sve vrednosti očuvane.
#[inline]
fn fp_mix(v: u32) -> u64 {
    let mut z = (v as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn range_u128(v: &[u128]) -> String {
    format!("{}–{}", v.iter().min().unwrap_or(&0), v.iter().max().unwrap_or(&0))
}

fn range_f64(v: &[f64]) -> String {
    let mn = v.iter().cloned().fold(f64::INFINITY, f64::min);
    let mx = v.iter().cloned().fold(0.0, f64::max);
    format!("{:.1}–{:.1}", mn, mx)
}

/// Statistika pojedinačnih operacija (npr. oslobađanje jednog bloka), u mikrosekundama.
fn op_stats(v: &mut Vec<u128>) -> String {
    if v.is_empty() {
        return "n/a".to_string();
    }
    v.sort_unstable();
    let med = v[v.len() / 2];
    let max = *v.last().unwrap();
    format!("n={} medijana {:.2} ms, max {:.2} ms", v.len(), med as f64 / 1000.0, max as f64 / 1000.0)
}

/// Mikrobenchmark generatora, jedna nit, bez pipeline-a. Razdvaja trošak nove memorije
/// (page fault-ovi + čišćenje nulama pri prvom upisu) od računanja slučajnih brojeva.
fn run_gen_microbench(args: &Args) {
    let epb = (args.block_size_mb * 1024 * 1024) / std::mem::size_of::<u32>();
    let blocks = (args.elements + epb - 1) / epb;
    let seed = args.seed.unwrap_or(42);
    let reps = args.repetitions.max(1);
    println!("[*] Mikrobenchmark generatora: {} blokova × {} vrednosti, {} ponavljanja, seed {}", blocks, epb, reps, seed);

    // Varijante: (naziv, ponovo korišćen bafer?, generator: Some(rng) ili None = samo brojač)
    let variants: [(&str, bool, Option<RngKind>); 6] = [
        ("1 nov bafer      + ChaCha  ", false, Some(RngKind::Chacha)),
        ("2 ponovo korišćen + ChaCha  ", true, Some(RngKind::Chacha)),
        ("3 nov bafer      + Xoshiro ", false, Some(RngKind::Xoshiro)),
        ("4 ponovo korišćen + Xoshiro ", true, Some(RngKind::Xoshiro)),
        ("5 nov bafer      + brojač  ", false, None),
        ("6 ponovo korišćen + brojač  ", true, None),
    ];
    let mut per_block: Vec<Vec<u128>> = vec![Vec::new(); variants.len()]; // µs po bloku
    let mut checksum = 0u64;
    let mut reuse: Vec<u32> = Vec::with_capacity(epb);
    // Jednom dodirni ponovo korišćeni bafer, da njegove stranice budu dodeljene
    reuse.resize(epb, 0);

    // Varijante se smenjuju unutar svakog ponavljanja, da sve dele iste uslove na mašini
    for _ in 0..reps {
        for (vi, &(_, reused, gen)) in variants.iter().enumerate() {
            for b in 0..blocks {
                let start = b * epb;
                let count = epb.min(args.elements - start);
                let t = Instant::now();
                let data: Vec<u32> = if reused {
                    let mut buf = std::mem::take(&mut reuse);
                    buf.clear();
                    buf
                } else {
                    Vec::with_capacity(count)
                };
                let filled = match gen {
                    Some(RngKind::Chacha) => fill_block_into(&mut StdRng::seed_from_u64(block_seed(seed, start)), data, start, count, args.elements, args.distribution),
                    Some(RngKind::Xoshiro) => fill_block_into(&mut Xoshiro256pp::from_seed_u64(block_seed(seed, start)), data, start, count, args.elements, args.distribution),
                    None => {
                        let mut d = data;
                        for i in 0..count { d.push((start + i) as u32); }
                        d
                    }
                };
                let el = t.elapsed();
                checksum = checksum.wrapping_add(filled[count / 2] as u64);
                if reused { reuse = filled; } else { drop(filled); }
                per_block[vi].push(el.as_micros());
            }
        }
    }

    println!("| Varijanta | Medijana po bloku | Min | Max | Po ponavljanju ({} blokova) |", blocks);
    println!("| :--- | :--- | :--- | :--- | :--- |");
    let mut meds = Vec::new();
    for (vi, (name, _, _)) in variants.iter().enumerate() {
        let v = &mut per_block[vi];
        v.sort_unstable();
        let med = v[v.len() / 2] as f64 / 1000.0;
        meds.push(med);
        println!(
            "| {} | {:.2} ms | {:.2} ms | {:.2} ms | ~{:.0} ms |",
            name.trim_end(), med, v[0] as f64 / 1000.0, *v.last().unwrap() as f64 / 1000.0, med * blocks as f64
        );
    }
    println!("Trošak nove memorije (ChaCha):  1 − 2 = {:.2} ms po bloku", meds[0] - meds[1]);
    println!("ChaCha − Xoshiro (bez memorije): 2 − 4 = {:.2} ms po bloku", meds[1] - meds[3]);
    println!("Čist prvi upis (brojač):         5 − 6 = {:.2} ms po bloku", meds[4] - meds[5]);
    println!("(kontrolni zbir: {:016x})", checksum);
}

/// Poređenje sa standardnim sortovima: ceo ulaz (isti generator i seme kao pipeline)
/// sortira se u memoriji. Meri se vreme sortiranja, dodatna memorija i ispravnost.
fn run_compare_std(args: &Args) {
    let n = args.elements;
    let epb = (args.block_size_mb * 1024 * 1024) / std::mem::size_of::<u32>();
    let reps = args.repetitions.max(1);
    println!("[*] Poređenje sa standardnim sortovima u memoriji: {} elemenata, {} ponavljanja, {} rayon niti, seed {:?}, RNG {:?}",
        n, reps, args.threads, args.seed, args.rng);
    println!("    Napomena: sort u memoriji drži ceo niz ({} MiB) i ne radi I/O; eksterno sortiranje radi sa ~150 MiB i upisuje/čita fajlove.",
        n * 4 / (1024 * 1024));

    // Isti podaci kao u pipeline-u: blok po blok, isto seme po bloku
    let t_gen = Instant::now();
    let mut input: Vec<u32> = Vec::with_capacity(n);
    let mut start = 0;
    while start < n {
        let count = epb.min(n - start);
        input.extend(generate_block_data(start, count, n, args.distribution, args.seed, args.rng));
        start += count;
    }
    let in_fp = input.iter().fold(0u64, |a, &v| a.wrapping_add(fp_mix(v)));
    println!("    Ulaz generisan za {} ms, otisak {:016x}", t_gen.elapsed().as_millis(), in_fp);

    let methods: [(&str, &str); 6] = [
        ("radix-lens-radix", "Radix LENS LSD radix (N niti, bez poređenja)"),
        ("radix-lens-radix-1nit", "Radix LENS LSD radix (1 nit, bez poređenja)"),
        ("std-sort", "slice::sort (stabilan, 1 nit, poređenje)"),
        ("std-sort-unstable", "slice::sort_unstable (1 nit, poređenje)"),
        ("rayon-par-sort", "rayon par_sort (stabilan, N niti, poređenje)"),
        ("rayon-par-sort-unstable", "rayon par_sort_unstable (N niti, poređenje)"),
    ];
    let one_thread = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    println!("| Metoda | Opis | Medijana | Min | Max | Dodatna memorija (peak) | Ispravno? |");
    println!("| :--- | :--- | :--- | :--- | :--- | :--- | :--- |");
    for (mi, (key, desc)) in methods.iter().enumerate() {
        let (key, desc) = (*key, *desc);
        if mi > 0 && args.pause_secs > 0 {
            eprintln!("    … pauza {} s pre metode {}", args.pause_secs, key);
            thread::sleep(Duration::from_secs(args.pause_secs));
        }
        let mut times: Vec<u128> = Vec::new();
        let mut extra: Vec<f64> = Vec::new();
        let mut ok = true;
        for _ in 0..reps {
            let mut data = input.clone();
            let base_rss = get_process_rss_mb();
            let sampler = RssSampler::start();
            let t = Instant::now();
            match key {
                "std-sort" => data.sort(),
                "std-sort-unstable" => data.sort_unstable(),
                "rayon-par-sort" => data.par_sort(),
                "rayon-par-sort-unstable" => data.par_sort_unstable(),
                "radix-lens-radix-1nit" => {
                    // Isti radix, ali u rayon pool-u sa jednom niti (uslovi kao u recognition_sort)
                    let mut scratch: Vec<u32> = Vec::new();
                    one_thread.install(|| radix_sort::radix_sort_u32(&mut data, &mut scratch));
                }
                _ => {
                    // Pomoćni bafer se alocira unutar merenja, kao i interni bafer stabilnog sorta
                    let mut scratch: Vec<u32> = Vec::new();
                    radix_sort::radix_sort_u32(&mut data, &mut scratch);
                }
            }
            let el = t.elapsed();
            let peak = sampler.finish();
            times.push(el.as_millis());
            extra.push((peak - base_rss).max(0.0));
            let sorted = data.windows(2).all(|w| w[0] <= w[1]);
            let fp = data.iter().fold(0u64, |a, &v| a.wrapping_add(fp_mix(v)));
            if !sorted || fp != in_fp || data.len() != n { ok = false; }
        }
        let med = median_u128(times.clone());
        let mn = *times.iter().min().unwrap();
        let mx = *times.iter().max().unwrap();
        let ex = median_f64(extra);
        println!("| {} | {} | {} ms | {} | {} | {:.0} MiB | {} |", key, desc, med, mn, mx, ex, if ok { "PROLAZ" } else { "PAD" });
        println!("SUMMARY,{},{},in-memory,{},{},{},,,{:.0},{}",
            args.label.clone().unwrap_or_else(|| "mem".to_string()), key, med, mn, mx, ex, if ok { "PROLAZ" } else { "PAD" });
    }
}

fn median_u128(mut v: Vec<u128>) -> u128 {
    v.sort_unstable();
    let mid = v.len() / 2;
    if v.len() % 2 == 0 && mid > 0 { (v[mid - 1] + v[mid]) / 2 } else { v[mid] }
}

fn median_f64(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = v.len() / 2;
    if v.len() % 2 == 0 && mid > 0 { (v[mid - 1] + v[mid]) / 2.0 } else { v[mid] }
}

fn main() {
    let args = Args::parse();

    rayon::ThreadPoolBuilder::new().num_threads(args.threads).build_global().unwrap();

    if args.gen_microbench {
        run_gen_microbench(&args);
        return;
    }
    if args.compare_std {
        run_compare_std(&args);
        return;
    }

    let target_path = Path::new(&args.target_dir);
    if !target_path.exists() {
        std::fs::create_dir_all(target_path).unwrap();
    }

    println!("[*] Pokretanje My Radix LENS v1.3.0 (Packed Loser Tree Engine)...");
    println!("    Scenario: {:?}", args.scenario);
    println!("    Ciljni disk/direktorijum: {}", args.target_dir);
    println!("    N (Elementi): {}", args.elements);
    println!("    Veličina bloka: {} MiB", args.block_size_mb);
    println!("    Bafer čitanja run-ova: {} KiB", args.read_buffer_kb);
    println!("    Izlazni bafer pisanja: {} KiB", args.write_buffer_kb);
    println!("    Niti (Threads): {}", args.threads);
    println!("    Ponavljanja: {}", args.repetitions);
    println!("    Pipeline režim: {}, Dubina: {}", args.pipeline, args.pipeline_depth);
    println!("    fsync: {}", args.fsync);
    println!("    Seed: {}", args.seed.map(|v| v.to_string()).unwrap_or_else(|| "nasumično".to_string()));
    println!("    Brojanje poređenja: {}", args.count_ops);
    println!("    Sortiranje bloka: {:?}", args.block_sort);
    println!("    RNG: {:?}", args.rng);
    println!("    Writer niti: {}", args.writers.max(1));
    println!("    Merge niti: {}", args.merge_threads.map(|n| format!("{} (paralelni algoritam)", n)).unwrap_or_else(|| "sekvencijalno".to_string()));
    println!("    Strategija: {}", args.strategy.map(|st| format!("{:?}", st)).unwrap_or_else(|| "sve".to_string()));

    let read_buffer_bytes = args.read_buffer_kb * 1024;
    let write_buffer_bytes = args.write_buffer_kb * 1024;
    let elements_per_block = (args.block_size_mb * 1024 * 1024) / std::mem::size_of::<u32>();

    let strategies: Vec<MergeStrategy> = match args.strategy {
        Some(st) => vec![st],
        None => vec![
            MergeStrategy::StandardHeap,
            MergeStrategy::OptimizedHeap,
            MergeStrategy::LoserTree,
            MergeStrategy::PackedLoserTree,
        ],
    };

    for strategy in strategies {
        println!("\n-------------------------------------------------------------");
        println!("  TESTIRANJE STRATEGIJE: {:?}", strategy);
        println!("-------------------------------------------------------------");

        let mut gen_durations = Vec::with_capacity(args.repetitions);
        let mut sort_durations = Vec::with_capacity(args.repetitions);
        let mut io_write_durations = Vec::with_capacity(args.repetitions);
        let mut prep_wall_durations = Vec::with_capacity(args.repetitions);
        let mut writer_iters: Vec<WriterStats> = Vec::new();
        let mut free_ops: Vec<u128> = Vec::new();
        let mut write_ops: Vec<u128> = Vec::new();
        let mut alloc_ops: Vec<u128> = Vec::new();
        let mut total_excl_verify_durations = Vec::with_capacity(args.repetitions);
        let mut max_inflight_overall = 0usize;
        let mut merge_init_durations = Vec::with_capacity(args.repetitions);
        let mut merge_core_durations = Vec::with_capacity(args.repetitions);
        let mut verify_durations = Vec::with_capacity(args.repetitions);
        let mut total_durations = Vec::with_capacity(args.repetitions);
        let mut peak_rss_values = Vec::with_capacity(args.repetitions);
        
        let mut last_run_count = 0;
        let mut cumulative_refills = 0u64;
        let mut cumulative_batch_writes = 0u64;
        let mut cumulative_emitted = 0u64;
        let mut cumulative_cmps = 0u64;
        let mut worst_part_ratio = 1.0f64;
        let mut final_write_durations = Vec::with_capacity(args.repetitions);
        let mut first_in_fp: Option<u64> = None;
        let mut input_stable = true;
        let mut all_correct = true;

        for iter in 0..args.repetitions {
            let iter_start = Instant::now();
            let rss_sampler = RssSampler::start();
            let mut iter_peak_rss = get_process_rss_mb();

            let mut gen_duration = Duration::ZERO;
            let mut sort_duration = Duration::ZERO;
            let mut io_write_duration = Duration::ZERO;
            let mut is_correct = true;

            let mut run_files: Vec<PathBuf> = Vec::new();
            let t_prep = Instant::now();
            let mut ws_it = WriterStats::default();

            if !args.pipeline {
                // --- SEKVENCIJALNI REŽIM ---
                let mut seq_scratch: Vec<u32> = Vec::new();
                max_inflight_overall = max_inflight_overall.max(1);
                let mut generated = 0;
                let mut block_idx = 0;

                while generated < args.elements {
                    let chunk_size = std::cmp::min(elements_per_block, args.elements - generated);
                    
                    let t_gen = Instant::now();
                    let mut block = generate_block_data(generated, chunk_size, args.elements, args.distribution, args.seed, args.rng);
                    gen_duration += t_gen.elapsed();

                    let t_sort = Instant::now();
                    sort_block(&mut block, args.block_sort, &mut seq_scratch);
                    sort_duration += t_sort.elapsed();
                    iter_peak_rss = iter_peak_rss.max(get_process_rss_mb());

                    let t_io = Instant::now();
                    let file_name = format!("strat_{:?}_run_{}_{}.bin", strategy, iter, block_idx);
                    let file_path = target_path.join(file_name);
                    let file = File::create(&file_path).unwrap();
                    let mut writer = BufWriter::with_capacity(write_buffer_bytes, file);

                    let bytes: &[u8] = unsafe {
                        std::slice::from_raw_parts(block.as_ptr() as *const u8, block.len() * std::mem::size_of::<u32>())
                    };
                    writer.write_all(bytes).unwrap();
                    writer.flush().unwrap();
                    if args.fsync {
                        writer.get_ref().sync_all().unwrap();
                    }
                    io_write_duration += t_io.elapsed();

                    run_files.push(file_path);
                    generated += chunk_size;
                    block_idx += 1;
                }
            } else {
                // --- PIPELINE REŽIM ---
                let depth = args.pipeline_depth;
                let (tx_gen, rx_gen) = sync_channel::<(usize, usize, Vec<u32>)>(depth);
                let (tx_sort, rx_sort) = sync_channel::<(usize, Vec<u32>)>(depth);

                let total_elements = args.elements;
                let dist = args.distribution;
                let seed = args.seed;
                let rng_kind = args.rng;
                let epb = elements_per_block;

                use std::sync::atomic::{AtomicUsize, Ordering as AO};
                use std::sync::Arc;
                let live = Arc::new(AtomicUsize::new(0));
                let peak_live = Arc::new(AtomicUsize::new(0));
                let (live_g, peak_g) = (live.clone(), peak_live.clone());

                // 1. Generator nit — meri sopstveno vreme generisanja
                let gen_handle = thread::spawn(move || {
                    let mut busy = Duration::ZERO;
                    let mut allocs: Vec<u128> = Vec::new(); // µs po bloku
                    let mut generated = 0;
                    let mut block_idx = 0;
                    while generated < total_elements {
                        let chunk_size = std::cmp::min(epb, total_elements - generated);
                        // Blok se broji kao živ od trenutka alokacije
                        let now_live = live_g.fetch_add(1, AO::SeqCst) + 1;
                        peak_g.fetch_max(now_live, AO::SeqCst);
                        let t = Instant::now();
                        let (block, alloc) = generate_block_data_timed(generated, chunk_size, total_elements, dist, seed, rng_kind);
                        busy += t.elapsed();
                        allocs.push(alloc.as_micros());
                        if tx_gen.send((block_idx, generated, block)).is_err() {
                            break;
                        }
                        generated += chunk_size;
                        block_idx += 1;
                    }
                    (busy, allocs)
                });

                // 2. Sorter nit — meri sopstveno vreme sortiranja
                let block_sort = args.block_sort;
                let sort_handle = thread::spawn(move || {
                    let mut busy = Duration::ZERO;
                    // Pomoćni bafer radix sorta se čuva između blokova (bez nove alokacije po bloku)
                    let mut scratch: Vec<u32> = Vec::new();
                    while let Ok((block_idx, _generated, mut block)) = rx_gen.recv() {
                        let t = Instant::now();
                        sort_block(&mut block, block_sort, &mut scratch);
                        busy += t.elapsed();
                        if tx_sort.send((block_idx, block)).is_err() {
                            break;
                        }
                    }
                    busy
                });

                // 3. Writer (glavni tok) — upisuje svaki blok odmah po prijemu
                // 3. Writer niti — svaka uzima sledeći sortiran blok i piše ga u SVOJ run fajl
                //    (nezavisni fajlovi, bez deljenog File položaja). Tajmeri su lokalni po niti.
                let writers = args.writers.max(1);
                let rx_sort = std::sync::Mutex::new(rx_sort);
                let fsync = args.fsync;
                let writer_results: Vec<(WriterStats, Vec<(usize, PathBuf)>, Vec<u128>, Vec<u128>, Duration)> =
                    thread::scope(|sc| {
                        let handles: Vec<_> = (0..writers).map(|_| {
                            let rx_sort = &rx_sort;
                            let live = &live;
                            sc.spawn(move || {
                                let mut ws = WriterStats::default();
                                let mut files: Vec<(usize, PathBuf)> = Vec::new();
                                let mut frees: Vec<u128> = Vec::new();
                                let mut writes: Vec<u128> = Vec::new();
                                let mut io = Duration::ZERO;
                                let t_writer = Instant::now();
                                loop {
                                    let t_wait = Instant::now();
                                    let msg = rx_sort.lock().unwrap().recv();
                                    ws.wait += t_wait.elapsed();
                                    let Ok((block_idx, block)) = msg else { break };

                                    let t_io = Instant::now();
                                    let file_name = format!("strat_{:?}_run_{}_{}.bin", strategy, iter, block_idx);
                                    let file_path = target_path.join(file_name);
                                    let file = File::create(&file_path).unwrap();
                                    let mut writer = BufWriter::with_capacity(write_buffer_bytes, file);
                                    let t_w = Instant::now();
                                    ws.create += t_w - t_io;

                                    let bytes: &[u8] = unsafe {
                                        std::slice::from_raw_parts(block.as_ptr() as *const u8, block.len() * std::mem::size_of::<u32>())
                                    };
                                    writer.write_all(bytes).unwrap();
                                    let t_f = Instant::now();
                                    ws.write += t_f - t_w;
                                    writes.push((t_f - t_w).as_micros());
                                    writer.flush().unwrap();
                                    if fsync {
                                        writer.get_ref().sync_all().unwrap();
                                    }
                                    let t_end = Instant::now();
                                    ws.flush += t_end - t_f;
                                    io += t_end - t_io;

                                    files.push((block_idx, file_path));
                                    let t_free = Instant::now();
                                    drop(block);
                                    let free = t_free.elapsed();
                                    ws.free += free;
                                    frees.push(free.as_micros());
                                    live.fetch_sub(1, AO::SeqCst);
                                    let t_close = Instant::now();
                                    drop(writer);
                                    ws.close += t_close.elapsed();
                                }
                                ws.total = t_writer.elapsed();
                                (ws, files, frees, writes, io)
                            })
                        }).collect();
                        handles.into_iter().map(|h| h.join().unwrap()).collect()
                    });

                // Zbir preko writer niti (ukupno = najduža nit; ostalo = zbir zauzetosti)
                let mut indexed_files: Vec<(usize, PathBuf)> = Vec::new();
                for (ws, files, frees, writes, io) in writer_results {
                    ws_it.total = ws_it.total.max(ws.total);
                    ws_it.wait += ws.wait;
                    ws_it.create += ws.create;
                    ws_it.write += ws.write;
                    ws_it.flush += ws.flush;
                    ws_it.free += ws.free;
                    ws_it.close += ws.close;
                    indexed_files.extend(files);
                    free_ops.extend(frees);
                    write_ops.extend(writes);
                    io_write_duration += io;
                }
                max_inflight_overall = max_inflight_overall.max(peak_live.load(AO::SeqCst));

                let (gen_busy, allocs) = gen_handle.join().unwrap();
                gen_duration = gen_busy;
                alloc_ops.extend(allocs.iter().copied());
                ws_it.alloc = Duration::from_micros(allocs.iter().sum::<u128>() as u64);
                sort_duration = sort_handle.join().unwrap();

                // FIFO kanali već čuvaju redosled; sortiranje je samo osigurač
                indexed_files.sort_by_key(|f| f.0);
                run_files = indexed_files.into_iter().map(|f| f.1).collect();
            }

            let prep_wall = t_prep.elapsed();
            last_run_count = run_files.len();
            iter_peak_rss = iter_peak_rss.max(get_process_rss_mb());

            let final_output_name = format!("strat_{:?}_final_{}.bin", strategy, iter);
            let final_output_path = target_path.join(final_output_name);
            let final_file = File::create(&final_output_path).unwrap();
            let final_writer = BufWriter::with_capacity(write_buffer_bytes, final_file);
            let batch_capacity = std::cmp::max(write_buffer_bytes / std::mem::size_of::<u32>(), 4096);
            let mut sink = OutSink::new(final_writer, batch_capacity);

            // Merge core = samo spajanje (uključuje predaju punih batch-eva OS-u, bez završnog flush-a)
            let mr = match (strategy, args.count_ops) {
                (MergeStrategy::StandardHeap, false) => merge_standard_heap::<false>(&run_files, read_buffer_bytes, &mut sink),
                (MergeStrategy::StandardHeap, true) => merge_standard_heap::<true>(&run_files, read_buffer_bytes, &mut sink),
                (MergeStrategy::OptimizedHeap, false) => merge_optimized_heap::<false>(&run_files, read_buffer_bytes, &mut sink),
                (MergeStrategy::OptimizedHeap, true) => merge_optimized_heap::<true>(&run_files, read_buffer_bytes, &mut sink),
                (MergeStrategy::LoserTree, _) => merge_loser_tree(&run_files, read_buffer_bytes, &mut sink),
                (MergeStrategy::PackedLoserTree, false) if args.merge_threads.is_some() =>
                    merge_parallel_packed::<false>(&run_files, read_buffer_bytes, write_buffer_bytes, &final_output_path, args.merge_threads.unwrap()),
                (MergeStrategy::PackedLoserTree, true) if args.merge_threads.is_some() =>
                    merge_parallel_packed::<true>(&run_files, read_buffer_bytes, write_buffer_bytes, &final_output_path, args.merge_threads.unwrap()),
                (MergeStrategy::PackedLoserTree, false) => merge_packed_loser_tree::<false>(&run_files, read_buffer_bytes, &mut sink),
                (MergeStrategy::PackedLoserTree, true) => merge_packed_loser_tree::<true>(&run_files, read_buffer_bytes, &mut sink),
            };
            let merge_init_ms = mr.init.as_millis();
            let merge_core_ms = mr.core.as_millis();

            // Finalni upis = poslednji batch + flush + (opciono) sync_all
            let t_final = Instant::now();
            sink.flush_batch();
            let mut final_writer = sink.writer;
            final_writer.flush().unwrap();
            if args.fsync {
                final_writer.get_ref().sync_all().unwrap();
                for part in &mr.output_parts {
                    std::fs::OpenOptions::new().write(true).open(part).unwrap().sync_all().unwrap();
                }
            }
            drop(final_writer);
            let final_write = t_final.elapsed();

            cumulative_batch_writes += sink.writes + mr.inner_batch_writes;
            cumulative_refills += mr.refills;
            cumulative_emitted += mr.emitted;
            cumulative_cmps += mr.comparisons;
            worst_part_ratio = worst_part_ratio.max(mr.max_part_ratio);
            iter_peak_rss = iter_peak_rss.max(get_process_rss_mb());

            let total_excl_verify = iter_start.elapsed();
            let t_verify = Instant::now();
            let output_files: Vec<PathBuf> = if mr.output_parts.is_empty() {
                vec![final_output_path.clone()]
            } else {
                mr.output_parts.clone()
            };
            if !mr.parts_complete { is_correct = false; }
            let mut prev_val: Option<u32> = None;
            let mut count = 0;
            let mut out_fp: u64 = 0;

            // Ulazni otisak iz run fajlova (van merenja pripreme i merge-a)
            let mut in_fp: u64 = 0;
            let mut in_count = 0usize;
            for file_path in &run_files {
                let mut r = BufferedRunReader::new(File::open(file_path).unwrap(), read_buffer_bytes);
                while let Some(v) = r.next_val() {
                    in_fp = in_fp.wrapping_add(fp_mix(v));
                    in_count += 1;
                }
            }

            // Delovi se čitaju redom kao jedan logički izlaz (prev_val prelazi granice delova)
            'verify: for out_file in &output_files {
                let mut verify_reader = BufferedRunReader::new(File::open(out_file).unwrap(), read_buffer_bytes);
                while let Some(val) = verify_reader.next_val() {
                    count += 1;
                    if let Some(p) = prev_val {
                        if p > val { is_correct = false; break 'verify; }
                    }
                    prev_val = Some(val);
                    out_fp = out_fp.wrapping_add(fp_mix(val));
                }
            }
            match first_in_fp {
                None => first_in_fp = Some(in_fp),
                Some(f) => if f != in_fp {
                    input_stable = false;
                    if args.seed.is_some() { is_correct = false; }
                },
            }
            if count != args.elements || in_count != args.elements || in_fp != out_fp {
                is_correct = false;
            }
            let verify_ms = t_verify.elapsed().as_millis();

            for file_path in &run_files { let _ = std::fs::remove_file(file_path); }
            let _ = std::fs::remove_file(&final_output_path);
            for part in &mr.output_parts { let _ = std::fs::remove_file(part); }

            if !is_correct { all_correct = false; }

            let total_ms = iter_start.elapsed().as_millis();
            iter_peak_rss = iter_peak_rss.max(rss_sampler.finish());
            gen_durations.push(gen_duration.as_millis());
            sort_durations.push(sort_duration.as_millis());
            io_write_durations.push(io_write_duration.as_millis());
            prep_wall_durations.push(prep_wall.as_millis());
            writer_iters.push(ws_it);
            total_excl_verify_durations.push(total_excl_verify.as_millis());
            merge_init_durations.push(merge_init_ms);
            merge_core_durations.push(merge_core_ms);
            final_write_durations.push(final_write.as_millis());
            verify_durations.push(verify_ms);
            total_durations.push(total_ms);
            peak_rss_values.push(iter_peak_rss);
        }

        let prep_range = range_u128(&prep_wall_durations);
        let core_range = range_u128(&merge_core_durations);
        let final_range = range_u128(&final_write_durations);
        let total_range = range_u128(&total_excl_verify_durations);
        let rss_range = range_f64(&peak_rss_values);
        let depth_bound = if args.pipeline { 2 + 2 * args.pipeline_depth + args.writers.max(1) } else { 1 };

        let report = BenchmarkReport {
            version: "My Radix LENS v1.3.0 (Packed Loser Tree)".to_string(),
            scenario: format!("{:?}", args.scenario),
            target_dir: args.target_dir.clone(),
            elements: args.elements,
            block_size_mb: args.block_size_mb,
            read_buffer_kb: args.read_buffer_kb,
            write_buffer_kb: args.write_buffer_kb,
            threads: args.threads,
            repetitions: args.repetitions,
            distribution: format!("{:?}", args.distribution),
            strategy: format!("{:?}", strategy),
            total_runs: last_run_count,
            median_gen_duration_ms: median_u128(gen_durations),
            median_sort_duration_ms: median_u128(sort_durations),
            median_io_write_duration_ms: median_u128(io_write_durations),
            median_prep_wall_ms: median_u128(prep_wall_durations),
            median_total_excl_verify_ms: median_u128(total_excl_verify_durations),
            max_inflight_blocks: max_inflight_overall,
            median_merge_init_duration_ms: median_u128(merge_init_durations),
            median_merge_core_duration_ms: median_u128(merge_core_durations),
            median_final_write_ms: median_u128(final_write_durations),
            median_verify_duration_ms: median_u128(verify_durations),
            median_total_duration_ms: median_u128(total_durations),
            median_peak_rss_mb: median_f64(peak_rss_values),
            is_correct: all_correct,
            notes: format!(
                "Strategy: {:?}, Runs: {}, Emitovano: {}, Poređenja: {}, Refills: {}, BatchWrites: {}",
                strategy, last_run_count, cumulative_emitted,
                if args.count_ops { cumulative_cmps.to_string() } else { "n/a (bez --count-ops)".to_string() },
                cumulative_refills, cumulative_batch_writes
            ),
        };

        println!("\n--- REZULTAT ZA STRATEGIJU: {:?} ---", strategy);
        println!("| Strategija | Gen Vreme | Sort Vreme | I/O Upis | Priprema (zid) | Core Vreme | Finalni upis | Ukupno Vreme | Peak RSS | Ispravno? |");
        println!("| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |");
        println!(
            "| {:?} | {} ms | {} ms | {} ms | {} ms | {} ms | {} ms | {} ms | {:.2} MiB | {} |",
            strategy,
            report.median_gen_duration_ms,
            report.median_sort_duration_ms,
            report.median_io_write_duration_ms,
            report.median_prep_wall_ms,
            report.median_merge_core_duration_ms,
            report.median_final_write_ms,
            report.median_total_duration_ms,
            report.median_peak_rss_mb,
            if report.is_correct { "PROLAZ" } else { "PAD" }
        );
        println!(
            "Raspon (min–max, {} pon.): Priprema {} ms | Merge core {} ms | Finalni upis {} ms | Ukupno bez verif. {} ms (medijana {}) | Peak RSS {} MiB",
            args.repetitions, prep_range, core_range, final_range, total_range, report.median_total_excl_verify_ms, rss_range
        );
        println!(
            "Živi blokovi: max {} (teorijska granica {}{}) → ~{} MiB podataka u letu",
            report.max_inflight_blocks, depth_bound, if args.pipeline { " = 2 + 2·depth + writers" } else { ", sekvencijalno" }, report.max_inflight_blocks * args.block_size_mb
        );
        if args.count_ops && cumulative_emitted > 0 {
            println!(
                "Poređenja po elementu: {:.3} (brojanje uključeno — vremena nisu uporediva sa merenjima bez --count-ops)",
                cumulative_cmps as f64 / cumulative_emitted as f64
            );
        }
        if let (Some(mt), MergeStrategy::PackedLoserTree) = (args.merge_threads, strategy) {
            println!(
                "Paralelni merge: {} niti, najveća particija = {:.3}× proseka (najgori slučaj kroz ponavljanja)",
                mt, worst_part_ratio
            );
        }
        println!(
            "Ulazni otisak: {:016x}{}",
            first_in_fp.unwrap_or(0),
            match args.seed {
                Some(_) => if input_stable { " (seed: identičan kroz sva ponavljanja)" } else { " (seed: RAZLIKUJE SE — greška!)" },
                None => " (bez --seed: ulaz se menja između ponavljanja)",
            }
        );
        if args.pipeline && !writer_iters.is_empty() {
            let med = |f: fn(&WriterStats) -> Duration| -> u128 {
                median_u128(writer_iters.iter().map(|w| f(w).as_millis()).collect())
            };
            println!(
                "Writer ({} niti; medijana po ponavljanju, ms; čekanje/zauzet = zbir preko niti): ukupno {} | čekanje {} | zauzet {} = create {} + write {} + flush {} + free {} + close {}",
                args.writers.max(1), med(|w| w.total), med(|w| w.wait),
                median_u128(writer_iters.iter().map(|w| (w.create + w.write + w.flush + w.free + w.close).as_millis()).collect()),
                med(|w| w.create), med(|w| w.write), med(|w| w.flush), med(|w| w.free), med(|w| w.close)
            );
            println!("write_all po bloku: {}", op_stats(&mut write_ops));
            println!("BlockFree po bloku (writer): {}", op_stats(&mut free_ops));
            println!(
                "BlockAlloc po bloku (generator, samo Vec::with_capacity; page fault-ovi su u Gen vremenu): {} | zbir/pon. medijana {} ms",
                op_stats(&mut alloc_ops), med(|w| w.alloc)
            );
        }
        println!("Detaljne beleške: {}", report.notes);
        // Mašinski čitljiv zbirni red: label,strategija,vrsta,ukupno_med,min,max,priprema_med,merge_med,peak_rss_med,ispravno
        println!(
            "SUMMARY,{},{:?},external,{},{},{},{},{},{:.0},{}",
            args.label.clone().unwrap_or_else(|| "bez-oznake".to_string()), strategy,
            report.median_total_excl_verify_ms,
            total_range.split('–').next().unwrap_or(""), total_range.split('–').nth(1).unwrap_or(""),
            report.median_prep_wall_ms, report.median_merge_core_duration_ms, report.median_peak_rss_mb,
            if report.is_correct { "PROLAZ" } else { "PAD" }
        );
    }
}
