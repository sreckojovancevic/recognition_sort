# My Radix LENS — izveštaj o optimizaciji

Oct 12, 2026 · @Srecko

> Autoritativni izveštaj o optimizaciji suite-a. U ovom repou kod živi u
> `radix_lens_bench/src/` (u izveštaju se pominje originalna lokacija
> `E:\samo\suit\src`). Sirovi rezultati (CSV/log) su u `radix_lens_bench/results/`.

## Sažetak

Eksterno sortiranje 100 miliona `u32` vrednosti ubrzano je sa **2821 ms na 720 ms** ukupnog vremena bez verifikacije, oko **3,9×**, uz istu proveru ispravnosti na bit-identičnim ulaznim podacima.

Najveći dobici došli su iz tri izmene: pipeline pripreme (generisanje, sortiranje i upis se preklapaju), loser tree sa upakovanim `u64` ključem (merge ~2× brži na istom broju poređenja) i paralelni merge u 8 niti sa zasebnim izlaznim fajlom po niti (merge još ~4,5× brži). Radix sort, dva writer-a i 6 rayon niti dodali su manje, ali merljive dobitke u pripremi.

Pipeline je sada ograničen zajedničkim resursima mašine (jezgra i memorija), a ne jednom fazom. Ubrzanje pojedinačne faze zato više ne skraćuje ukupno vreme, što je potvrđeno sa tri neuspela pokušaja (radix bez dodatnog writer-a, 4 writer-a, brži generator).

## Konfiguracija i metodologija

Sva merenja su na istom računaru (Intel i9, 32 GB RAM, Windows), cilj `C:\temp_sort_pipe1` na NVMe disku, osim namenskog poređenja diskova.

| Parametar | Vrednost |
| --- | --- |
| Elementi | 100.000.000 × `u32` (12 run-ova) |
| Blok | 32 MiB |
| Bafer čitanja / upisa | 2048 KiB / 4096 KiB |
| Seme | `--seed 42`, otisak ulaza `5401f572beb93e6a` |
| Ponavljanja | 15 po konfiguraciji (3 za dijagnostiku) |

Pravila koja su važila kroz ceo rad:

- **Ispravnost:** svako ponavljanje proverava broj elemenata, sortiranost (i preko granica delova izlaza) i multiset otisak (zbir heševa svih vrednosti) ulaza naspram izlaza.
- **Jedna izmena po eksperimentu**, sa predviđanjem zapisanim pre merenja, tako da rezultat može da obori hipotezu.
- **Poređenje medijana i raspona**, nikad pojedinačnih vrednosti; varijante koje se porede puštane su jedna za drugom, u istim uslovima.
- **Glavna metrika:** ukupno vreme bez verifikacije; za izmene u pripremi gleda se zidno vreme pripreme, za merge vreme `Merge core`.
- Brojanje poređenja (`--count-ops`) i mikrobenchmark (`--gen-microbench`) su posebni režimi i ne mešaju se sa vremenskim merenjima.

## Ispravljene greške u kodu

Pre optimizacije ispravljene su četiri greške; prve dve su značile da raniji LoserTree rezultati benchmarka nisu bili ispravni.

| Greška | Posledica | Ispravka |
| --- | --- | --- |
| `LoserTree::adjust` nikad nije upisivao igrača u prazan čvor (sentinel) | Stablo nije sortiralo (izlaz `[]` ili delimičan) | Na praznom čvoru igrač se parkira i staje |
| `main.rs` je imao sopstvenu, pokvarenu kopiju `LoserTree` | Benchmark nije koristio ispravljeni modul | Kopija uklonjena, koristi se modul |
| `BufferedRunReader` je pretpostavljao da `read()` vraća broj bajtova deljiv sa 4 | Izgubljeni bajtovi, pogrešna vrednost pri kratkom čitanju, tiha greška kao kraj fajla | Nepotpuna vrednost se čuva u `carry`; `Interrupted` se ponavlja; ostale greške padaju sa porukom |
| Paralelni merge je pisao u jedan unapred proširen izlazni fajl | Ogromna varijacija, i kvarenje pripreme sledećeg ponavljanja | Svaka nit piše svoj `.partN` fajl; provera ih čita redom |

Uz to je dodato 29 testova (čitač, generator, oba loser tree-a, paralelni merge, radix sort), a provera ispravnosti je proširena multiset otiskom.

## Tok optimizacije

[grafikon: Medijane ukupnog vremena bez verifikacije, 15 ponavljanja, seed 42, C: NVMe, 12. 10. 2026 (run_compare.ps1 + ponovljena f6) — vidi results/ CSV-ove]

Packed loser tree i paralelni merge zajedno su skratili merge sa ~1584 ms na ~230 ms; pipeline, radix sort, dva writer-a i 6 rayon niti skratili su pripremu sa ~1200 ms na ~460 ms. Sve faze su izmerene istim kodom u jednom danu; refaktor merge koda u funkcije (11. 10.) nije menjao algoritam, ali je pomerio referencu za ~11% i ugrađen je u sve faze, pa se ne vidi kao zaseban korak.

## Odbačene ideje i oborene hipoteze

Devet izmena i hipoteza nije dalo merljiv dobitak ili je oboreno merenjem; sve su ostale kao opcije ili zapisane, ne u podrazumevanoj konfiguraciji.

| Ideja ili hipoteza | Rezultat | Zašto |
| --- | --- | --- |
| Pipeline dubina 1 ili 2 | Isto vreme kao dubina 0, 32–64 MiB više memorije | Faze su ujednačene, bafer nema šta da upije; isto i na SSD i HDD |
| Upis bez Windows keša (`fsync`) kao režim rada | Samo za merenje diskova | Bez `fsync` upis meri keš, sa njim disk |
| Radix sort bez dodatnog writer-a | Sort −24%, priprema bez promene | Usko grlo se pomerilo na writer |
| Skupo oslobađanje blokova (`NtFreeVirtualMemory`) | Oboreno: 1–2 ms po bloku | Profil je kernel deo upisa pogrešno pripisao oslobađanju |
| Pool blokova | Nije urađen | Ni oslobađanje ni nova memorija (~12% generatora) ne opravdavaju ga |
| 4 writer niti | Bez dobitka, više memorije | Writer-i već čekaju sa 2 niti |
| Brži generator (Xoshiro256++) | Izolovano ~50 ms brži, u pipeline-u bez efekta (dva puta ponovljeno) | Generator u pipeline-u ograničava konkurencija, ne računanje |
| Merge čita run fajlove sa diska | Oboreno | Merge isti na NVMe, SSD i HDD: čita iz keša (32 GB RAM) |
| Pad takta kao uzrok sporog `std` sorta | Delimično podržano, nije dokazano | Vraća se na ~481 ms, ali ne dosledno; bez očitavanja takta |

## Dijagnostika okruženja

Okruženje utiče na apsolutne brojeve, ali ne menja zaključke koji su dobijeni poređenjem varijanti jedne za drugom.

**Diskovi** (OptimizedHeap, pipeline dubine 0, ranija verzija programa). Upis run-ova po ponavljanju, ms:

| Disk | Bez `fsync` | Sa `fsync` | Merge core |
| --- | --- | --- | --- |
| C: NVMe | 412 | 951 | 1789 |
| D: SATA SSD | 615 | 1112 | 1805 |
| E: HDD | 1494 | 3982 | 1804 |

- Bez `fsync` merenje upisa većinom meri Windows keš; za poređenje diskova merodavni su samo brojevi sa `fsync`.
- Merge je isti na sva tri diska, jer sa 32 GB RAM-a run fajlovi (~400 MB) ostaju u kešu. HDD sa ~100 MB/s ne bi mogao da ih pročita za 1,8 s.

**Pozadinski procesi.** F-Secure (WithSecure) je aktivan, Microsoft Defender je pasivan. Tokom benchmarka disk C: je skoro neaktivan (red čekanja 0,01) i nijedan drugi proces ne piše značajno. Jednom je upis po bloku skočio na ~101 ms i već u sledećem pokretanju vratio se na ~36 ms, pa je zabeležen kao prolazno odstupanje.

**Promenljivost tokom dana.** Iste konfiguracije daju 5–10% različite apsolutne vrednosti u različitim delovima dana; zato su sve odluke donete na parovima pokretanja jedno za drugim.

## Otvorena pitanja i ograničenja

- **Bimodalni `std` sort:** isti kod daje ~481 ili ~590 ms, uz generator koji se pomera zajedno sa njim. Uzrok (takt, ograničenje snage ili konkurencija za memoriju) nije utvrđen; zahteva očitavanje efektivnog takta (HWiNFO) tokom brzog i sporog pokretanja.
- **Zastoji upisa posle mnogo uzastopnih pokretanja:** `write_all` po bloku tri puta je skočio sa ~44 ms na ~92–101 ms medijane ili pojedinačno do 684 ms, uvek posle više pokretanja sa desetinama GB upisa. Hipoteza: Windows usporava nove upise dok prazni keš na disk; nije potvrđena.
- **Mehanizam kod jednog proširenog izlaznog fajla:** NTFS dopuna nulama je u skladu sa rezultatom, ali nije direktno izmerena.
- **Ograničenje pipeline-a:** test sa `--threads 6/8` ukazuje na otimanje niti za jezgra; da li je i9 sa 8 jezgara i hyperthreading-om, i koliki je udeo memorije, nije potvrđeno.
- **Merge iz keša:** svi rezultati merge-a važe za podatke koji staju u RAM. Za ulaz veći od slobodne memorije merge bi čitao sa diska i ovi brojevi ne važe.
- **Skaliranje na velike skupove:** broj run-ova raste sa N, a sa njim memorija za bafere, otvoreni fajlovi i dubina stabla (na 10 milijardi elemenata ~2,4–4,7 GiB samo za bafere). Zaštite (provera pre početka, budžet bafera, višeprolazni merge) još nisu ugrađene.
- **Jedna mašina, jedna raspodela:** merenja su na jednom računaru i većinom na uniformno slučajnim podacima.

## Poređenje sa sortovima zasnovanim na poređenju

Na istih 100M `u32` vrednosti, u memoriji, Radix LENS radix je u jednoj niti **2,29× brži od `sort_unstable`** i **3,36× od `sort`**; sa 8 niti prednost nad `par_sort_unstable` pada na **1,26–1,59×**.

| Metoda | Niti | Vreme, medijana (ms) | Dodatna memorija (MiB) | Poređenja |
| --- | --- | --- | --- | --- |
| Radix LENS radix | 1 | 788 | 381 | ne |
| `sort_unstable` | 1 | 1802 | 0 | da |
| `sort` (stabilan) | 1 | 2645 | 191 | da |
| Radix LENS radix | 8 | 374 | 382 | ne |
| `par_sort_unstable` | 8 | 596 | 0 | da |
| `par_sort` (stabilan) | 8 | 985 | 383 | da |

Izvor: `--compare-std`, 5 ponavljanja, seed 42, otisak `5401f572beb93e6a`, 12. 10. 2026; sve metode PROLAZ.

- **Nalaz iz `recognition_sort` važi i naspram jačeg protivnika:** studija je radix poredila samo sa stabilnim `std::sort` (2,47×); u jednoj niti radix je brži i od `sort_unstable`.
- **Radix slabije skalira na niti:** sa 8 niti je 2,1× brži nego sa jednom, a `sort_unstable` 3,0×. Verovatno više zavisi od memorije nego od jezgara (zaključak iz merenja, ne direktna mera).
- **Cena radix-a:** pomoćni bafer veličine ulaza (~381 MiB), dok `sort_unstable` radi u mestu.
- **Šum:** `par_sort_unstable` je imao 468 ms u prvom i 596 ms u drugom prolazu, zato je prednost na 8 niti raspon, ne jedan broj.
- **Eksterno naspram memorije:** preporučena eksterna konfiguracija (720 ms, ~146 MiB, sa upisom i čitanjem fajlova) brža je od `sort_unstable` u jednoj niti u memoriji, a samo 1,2–1,5× sporija od `par_sort_unstable` koji drži ceo niz u RAM-u.
- **Poređenja po elementu:** Radix LENS ima 0 u formiranju run-ova i izmereno 4,0 u merge-u; klasičan eksterni sort (sort poređenjem + heap merge) najmanje ~26,6 (teorijski minimum za blok + izmereni heap). Manje poređenja samo po sebi nije brže: stari LoserTree je imao istih 4,0 i bio je najsporiji.

## Preporučena konfiguracija

Preporuka je PackedLoserTree sa 8 merge niti, radix sort blokova, 2 writer niti, 6 rayon niti i pipeline dubine 0: priprema ~460 ms, merge ~230 ms, ukupno ~720 ms, Peak RSS ~146 MiB (12. 10. 2026).

```
cargo run --release -- --scenario external --target-dir "C:\temp_sort_pipe1" --elements 100000000 --block-size-mb 32 --read-buffer-kb 2048 --write-buffer-kb 4096 --threads 6 --repetitions 15 --pipeline --pipeline-depth 0 --seed 42 --strategy packed-loser-tree --merge-threads 8 --block-sort radix --writers 2
```

Ceo niz faza i poređenje sa standardnim sortovima ponavljaju se jednom komandom: `powershell -ExecutionPolicy Bypass -File .\run_compare.ps1` (log i zbirni CSV u `logs\`). Podrazumevane vrednosti programa su ostale konzervativne (`--block-sort std`, `--writers 1`, `--rng chacha`, sekvencijalni merge bez `--merge-threads`), tako da stara merenja ostaju ponovljiva. Kod je u `radix_lens_bench/src` (original: `E:\samo\suit\src`), sa rezervnim kopijama `main.rs.back12`–`back30` (nisu u repou).

## Zaključak

Radix LENS je u merenjima brži od `sort` i `sort_unstable` na velikim nizovima `u32` (2,29–3,36× u jednoj niti, 1,26–1,59× naspram `par_sort_unstable` sa 8 niti), po cenu pomoćne memorije veličine ulaza i ograničenja na celobrojne ključeve. Radix LENS je hibrid: run-ove sortira bez poređenja (LSD radix), a spaja ih poređenjem (loser tree sa upakovanim ključem), u memoriji ili eksterno.

**Prednosti — prikazane u merenjima** (100M `u32`, uniformno slučajni podaci, ista mašina, isti ulaz):

- **Jedna nit:** 2,29× brži od `sort_unstable` (788 prema 1802 ms) i 3,36× od `sort` (788 prema 2645 ms); nalaz iz `recognition_sort` važi i naspram jačeg protivnika.
- **8 niti:** 1,26–1,59× brži od `par_sort_unstable`, 2,6× od `par_sort`.
- **Eksterni režim:** 720 ms sa ~146 MiB RAM-a, uz upis i čitanje fajlova — brže od `sort_unstable` u jednoj niti u memoriji, a samo 1,2–1,5× sporije od `par_sort_unstable` koji drži ceo niz u RAM-u.
- **Ispravnost i ponovljivost:** svaki rezultat proveren (sortiranost, broj elemenata, otisak sadržaja), isti ulaz za isto seme.

**Mane i ograničenja — prikazane u merenjima:**

- **Memorija:** pomoćni bafer veličine ulaza (381 MiB za 100M); `sort_unstable` radi u mestu (0 MiB), `sort` traži pola ulaza (191 MiB).
- **Slabije skalira na niti:** radix sa 8 niti ubrzava 2,1×, `sort_unstable` 3,0×, pa prednost sa 2,29× pada na 1,26–1,59×.
- **Mali nizovi:** u `recognition_sort` je `std::sort` brži na 100 elemenata (radix ~2,8× sporiji), radix vodi tek od ~10.000; fiksni trošak histograma i prolaza isplati se tek na većim nizovima.
- **Okruženje u eksternom režimu:** zavisi od Windows keša, diska i pozadinskih procesa; izmereni su povremeni zastoji upisa i pomeranja od 5–10% tokom dana.

**Ograničenja koja nisu merena, ali proizilaze iz načina rada:**

- **Samo ključevi fiksne širine** (sada `u32`); `sort` i `sort_unstable` rade sa bilo kojim tipom koji se poredi, sa sopstvenim pravilom poređenja, stringovima i zapisima.
- **Delimično sortirani podaci:** `sort_unstable` ima ubrzanje za skoro sortirane nizove, radix ga nema; mereni su samo uniformno slučajni podaci.
- **Veliki skupovi u eksternom režimu:** memorija za bafere i otvoreni fajlovi rastu sa brojem run-ova, zaštite još nisu ugrađene, a merge brojevi važe samo dok podaci staju u RAM.

**Primenjivost:**

| Situacija | Bolji izbor |
| --- | --- |
| Veliki nizovi celobrojnih ključeva (stotine hiljada i više): ID-jevi, vremenske oznake, indeksi; naročito kad podaci ne staju udobno u memoriju | Radix LENS |
| Mali nizovi, ključevi koji nisu celi brojevi ili su složeni, ograničena memorija pri sortiranju u memoriji, skoro sortirani podaci (verovatno, nije mereno) | `sort_unstable` |
| Potreban stabilan redosled zapisa sa istim ključem, pomoćna memorija od pola ulaza nije problem | `sort` |

---

*Provenance: koncept i pravac — Srećko Jovančević; kod suite-a — pretežno Gemini, uz prepravke/fiksove Claude (Anthropic); dizajn testova (šta meriti / koji scenariji) — Claude i ChatGPT.*
