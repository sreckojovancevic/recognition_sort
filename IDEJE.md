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
