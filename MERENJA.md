# Merenja — nekomparativno sortiranje u recognition okviru

> **Transparentnost.** Ovde su *sve* mere kako su stvarno dobijene — i
> debug (bez `-O`) i optimizovano (`rustc -O`) — sa objašnjenjem zašto se
> razlikuju. Ništa nije ulepšano. Brojevi su sa autorove mašine (Windows),
> pa su mašinski-specifični; bitni su odnosi i redovi veličine, ne apsolut.

## Šta je ovo (i šta nije)

Ovo je istraživanje i **rekonstrukcija** nekomparativnog sortiranja unutar
Recognition Organization okvira — celobrojni blizanac string engine-a. Isti
princip kao MSD prepoznavanje stringova: **prepoznaj, smesti, bez poređenja.**

**Ovo nisu novi algoritmi.** Ono do čega se došlo poznato je kao:
- *direktno adresiranje / counting sort* (pozicija = vrednost − min),
- *radix sort* (prepoznavanje cifru/bajt po cifru).

Vrednost nije nov algoritam nego razumevanje i objedinjavanje: da se brojevi
i stringovi u ovom okviru organizuju istim mehanizmom, sve dokazano kodom
koji se sam testira (`assert`), i sve pošteno izmereno.

## Fajlovi

| Fajl | Šta radi |
|---|---|
| `samosvesni_broj.rs` | broj zna svoje mesto; pozicija = vrednost − koren; O(n), 0 poređenja |
| `rang_broj.rs` | dvostruki identitet (vrednost + rang); granica suvišno/informacija |
| `sort_apsolutni.rs` | proizvoljni celi, 0 poređenja; cena je prostor |
| `rang_radix_1m.rs` | radix baza 10, 1M brojeva |
| `rang_radix256_1m.rs` | radix baza 256 (bajt po bajt), 1M brojeva |

## Korektnost (uvek proverena protiv `sort()` / `sorted()`)

- `sort_apsolutni`: `[47,3,99,8,3]`, `[14,10,13,11,12]`, `[5,2,9,1]` — svi se
  poklapaju sa `sorted()`, **0 poređenja** između vrednosti.
- radix baza 10, **1.000.000** brojeva (opseg do 100M): poklapa se sa
  `sorted()`, **0 poređenja**, 8 prolaza.
- radix baza 256, 1.000.000 brojeva: poklapa se, **0 poređenja**, 4 prolaza.

"0 poređenja" = nijednom se dve vrednosti ne porede da bi se odredio
redosled; svaki broj ode na mesto izvedeno iz sebe.

## Brzina — debug naspram `-O` (OVO JE KLJUČNO)

1.000.000 brojeva, opseg do 100.000.000.

| Varijanta | Kompajl | radix | `std::sort()` | odnos |
|---|---|---|---|---|
| baza 10  | debug (bez `-O`) | 249.5 ms | 543.8 ms | 2.18× |
| baza 256 | debug (bez `-O`) | 71.1 ms  | 549.9 ms | **7.73×** |
| baza 256 | **`-O`**        | **15.1 ms** | **23.3 ms** | **1.54×** |

**Zašto debug laže.** U debug modu `std::sort` je nesrazmerno spor (provere
granica, bez inline-ovanja), pa izgleda mnogo gori nego što jeste — odatle
varljivih 7.73×. Sa `rustc -O` i radix i `std::sort` ubrzaju, ali `std::sort`
ubrza *mnogo više* (549 ms → 23 ms). **Pravi, pošteni broj je ~1.5×.**

Pouka o merenju: **uvek meriti sa `-O`; debug brojevi obmanjuju.**

### 10.000.000 brojeva (sve `-O`) — uključuje i poraz

| Varijanta | prolaza | radix | `std::sort()` | odnos |
|---|---|---|---|---|
| baza 10  | 8 | 473.5 ms | 294.4 ms | **radix GUBI (0.62×)** |
| baza 256 | 4 | 142.2 ms | 291.6 ms | radix pobeđuje 2.05× |

**Broj prolaza je presudan.** Isti podaci, isti n — menja se samo baza.
Baza 10 (8 prolaza) *gubi* od `std::sort` jer memorijski saobraćaj 8 prolaza
nadjača O(n·log n). Baza 256 (4 prolaza) *pobeđuje*. Dakle radix nije uvek
brži — prednost zavisi od broja prolaza (baze i opsega).

**Skaliranje.** Za bazu 256, prednost raste sa n: 1.54× na 1M → 2.05× na 10M.
Radix je linearan, `std::sort` je n·log n — pa kad su prolazi malobrojni,
radix se odvaja kako n raste.

## Ponašanje kroz razmeru (nije pobeda — ukaz o ponašanju)

Benchmark (`benchmark_full.rs`) meri vreme, **stvarno izmerenu** vrsnu
memoriju (broječi alokator u čistom std-u) i korektnost (poređenje sa
`std::sort`). Vrednosti u32, opseg do 100M. Radix baza 256. Sve `-O`.

| N | radix | std sort | odnos | radix mem (mereno) | ulaz mem | korektno |
|---|---|---|---|---|---|---|
| 100 | 5.0 µs | 1.8 µs | 0.36× (std brži) | ~0 | ~0 | DA |
| 10.000 | 101 µs | 132 µs | 1.30× | 0.1 MB | 0.0 MB | DA |
| 20.000.000 | 295 ms | 502 ms | 1.70× | 228.9 MB | 76.3 MB | DA |
| 50.000.000 | 680 ms | 1.3 s | 1.91× | 572.2 MB | 190.7 MB | DA |
| 70.000.000 | 748 ms | 1.9 s | 2.48× | 801.1 MB | 267.0 MB | DA |
| 100.000.000 | 1.1 s | 2.7 s | 2.47× | 1144.4 MB | 381.5 MB | DA |

**Čitanje ponašanja (ne rang-lista):**

- **Prelomna tačka.** Na N=100 radix *gubi* (0.36×) — režija prolaza/bafera
  guta sve kad nema šta da se sortira. Do ~10.000 se izjednačava i prestiže.
  Postoji veličina ispod koje je comparison sort bolji; to je podatak, ne poraz.
- **Rast pa plato.** Prednost raste (1.30→2.48) kako n raste (O(n) vs O(n·log n)),
  pa se oko 70–100M **zaustavlja na ~2.5×** — skoro sigurno zasićenje
  memorijske propusnosti (oba algoritma udaraju u brzinu RAM-a).
- **Cena brzine = memorija.** Radix peak ≈ **3× ulaz** (tek + bafer + izlaz),
  raste linearno (100M ulaza 381 MB → radix 1144 MB). `std::sort` je in-place
  (~1×). Brže na razmeri, ali po ceni ~3× memorije. Kompromis, izmeren.

## Pošten zaključak

- Smeštanje po poziciji/rangu je **O(n), bez poređenja** — stvarno i uvek.
- Za cele brojeve iz ograničenog opsega, radix je realno **~1.5× brži** od
  optimizovanog `std::sort`, bez poređenja. Tačno i očekivano po teoriji
  (radix O(n·k) sa malim k, naspram O(n·log n)); memorijski saobraćaj svede
  prednost na ~1.5×.
- Prednost **raste** sa brojem elemenata, a **smanjuje se** ako opseg traži
  više prolaza (cifara/bajtova).

## Granice (iskreno)

- **Specijalizovano, ne univerzalno.** Radix/counting rade na onome što se
  razlaže na cifre/bajtove iz ograničenog broja koraka (celi brojevi,
  fiksni ključevi). `std::sort` radi na *bilo čemu* uporedivom (stringovi
  proizvoljne dužine, floati, custom poređenja). Nije „radix je bolji od
  sorta" — nego „za cele brojeve, nekomparativni sort pobeđuje".
- **Prostorni zid direktnog adresiranja.** `sort_apsolutni` traži niz
  veličine opsega: za `[47,3,99,8]` to je 97 slotova za 4 broja; za
  `[3, 1e9]` bila bi milijarda. Radix upravo to rešava (prostor = baza + n,
  nezavisno od opsega).

## Reprodukcija

```
rustc -O rang_radix256_1m.rs && ./rang_radix256_1m
rustc -O rang_radix_1m.rs     && ./rang_radix_1m
rustc sort_apsolutni.rs       && ./sort_apsolutni
rustc samosvesni_broj.rs      && ./samosvesni_broj
rustc rang_broj.rs            && ./rang_broj
```
