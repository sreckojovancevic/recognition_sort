# Ideje — zabeleške za kasnije

## Racionalni pozicioni ključ (ubacivanje bez presortiravanja)

**Ideja.** Umesto celobrojnog ranga (pozicija = vrednost − koren), svakom
elementu dodeliti **racionalni pozicioni ključ**. Pošto su racionalni
brojevi *gusti* (između bilo koja dva uvek postoji treći), uvek možeš da
ubaciš nov element *između* dva postojeća računajući sredinu njihovih
ključeva — bez pomeranja i bez ponovnog rangiranja ostalih.

**Šta daje što radix/counting ne daju.** Radix i direktno adresiranje
sortiraju brz̀o i bez poređenja, ali svako ubacivanje traži ponovni prolaz.
Pozicioni ključ daje **održavanje poretka**: ubacivanje je lokalno i jeftino.

```
imaš:    A(ključ 2.0)   B(ključ 3.0)
ubaci X između:  ključ(X) = (2.0 + 3.0) / 2 = 2.5
rezultat: A(2.0)  X(2.5)  B(3.0)   — niko se nije pomerio
```

**Gde se stvarno koristi.** Order keys / fractional indexing — Figma,
kolaborativni editori (Google Docs), uređene liste u bazama, drag-and-drop
redosled. Srodno: "order-maintenance problem".

**Veza sa projektom.** Ovo je evolucija `samosvesni_broj` / `rang_broj`:
sa celobrojnih pozicija (fiksna mreža) na racionalne pozicije (umetljiva
mreža). Drugi identitet (gde stoji) postaje kontinuiran umesto diskretan.

**Poznata zamka.** Ponovljeno uzimanje sredine smanjuje razmak — ključevi
rastu u dužini/preciznosti. Rešenja: ključevi proizvoljne dužine
(string-ključevi, npr. baza-62 između dva stringa) ili povremeno
rebalansiranje. Treba obraditi, nije prepreka.

**Sledeći korak kad se vratimo.** Napisati `PozicioniBroj` sa racionalnim
(ili string) ključem i operacijom `ubaci_izmedju(a, b)`, plus test da
poredak ostaje tačan posle mnogo ubacivanja bez presortiravanja.

> **Realizovano:** `src/bin/pozicioni_broj.rs` (string ključevi, `izmedju(a,b)`,
> ubacivanje bez presortiravanja, rast ključa umesto preciznog zida).

---

## Blokovi i spoljni identifikatori (rad na nivou relacija)

**Ideja.** Broj nosi i relacioni identitet. U `2 < 3 < 4`, brojevi 2 i 4 su
*spoljni identifikatori* trojke — definišu njenu poziciju. Sa blokovima
`[2,3,4]`, `[4,5,6]`, `[6,7,8]` ne sortiramo svaki broj posebno, nego
**mečujemo blokove po spoljnim identifikatorima (min, max, granice)**. Rad na
nivou relacija između blokova, ne pojedinačnih poređenja.

**Gde se stvarno koristi (prior art — graditi NA ovome, ne od nule):**
- **Zone maps / min-max indeksi** — kolone u Parquet/ORC bazama čuvaju min/max
  svakog bloka; upit preskače blokove čiji [min,max] ne može da pogodi, bez
  gledanja pojedinačnih vrednosti (*data skipping*). Ovo je tačno "mečuj po
  spoljnim identifikatorima".
- **Run-based merge / eksterno sortiranje / LSM kompakcija** — spajanje
  sortiranih blokova poređenjem *granica*, ne svih elemenata.
- **Range-partition merge** — distribuirani sort (Spark/MapReduce) deli na
  particije po opsegu ključeva i spaja po opsezima.

**Potencijalne primene (iz originalne beleške):**
- Distribuirano sortiranje / merge (blokovi sa min/max identifikatorima).
- Kolaborativni sistemi (elementi sa "između A i B" — veže se na pozicioni ključ).
- Streaming sortiranje (prepoznavanje blokova čim stignu — run detekcija).
- Pattern matching u sekvencama (traženje bloka po spoljnim identifikatorima).

**Poštena ograda.** Dobitak je na nivou *spajanja/preskakanja*. Da bi blok imao
čist min/max, mora prvo interno da bude uređen (trošak negde). Preklapajući
blokovi (`[2,3,4]` i `[4,5,6]` dele 4) traže pažnju oko granica. Nije mana —
samo granica gde "blok-nivo" pomaže.
