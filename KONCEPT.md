# Koncept — samosvesni broj (okvir za dalji rad)

> Ovaj dokument stoji **iznad** benchmarka. Benchmark i radix256 su *jedna
> primena* principa prepoznavanja, a ne granica koncepta. Dalji rad treba da
> čuva ovu postavku, a ne da je menja da bi se uklopila u unapred izabran
> algoritam ili meru.

## Osnovna postavka

Samosvesni broj **nije zamena za broj**, niti samo struktura koja ga opisuje.
On zadržava izvornu svrhu i identitet broja, a dobija **dodatne okvire
identiteta** koji mu omogućavaju da prepozna sopstveni položaj i odnos prema
drugim brojevima.

Primer:

**2 < 3 < 4**

Broj 3 ostaje broj 3 — njegova osnovna vrednost i namena nisu promenjene. Ali
kroz dodatni okvir identiteta može da prepozna da je 2 njegov prethodnik, a 4
njegov sledbenik. Time nastaje **dodatni relacioni objekat, bez gubitka
izvornog broja**.

## Tri nivoa koja treba sačuvati

1. **Osnovni identitet** — broj ostaje ono što jeste.
2. **Dodatni okviri identiteta** — omogućavaju mu da prepozna svoj položaj,
   kontekst i odnose.
3. **Primena** — ti okviri mogu da se koriste za organizaciju, sortiranje i
   druge mehanizme koji se tek razvijaju.

## Šta benchmark NIJE

- Nije svođenje ideje na poređenje dva algoritma.
- `RangBroj` **nije** samo optimizacija sortiranja.
- `radix256` je **jedna konkretna primena** principa prepoznavanja, a ne
  granica koncepta.

Benchmark meri *ponašanje jedne primene*. Ne definiše domet ideje.

## Granica (poštena, deo same postavke)

Prepoznavanje prethodnika i sledbenika **neposredno iz vrednosti** funkcioniše
za odgovarajući **gust, uzastopni** numerički domen (tamo je sused = vrednost
± 1, zakon a ne podatak).

Kod **proizvoljnog skupa retkih** brojeva ti odnosi se ne mogu uvek izvesti
samo iz vrednosti — tada je potreban **dodatni kontekst** (zapisana relacija,
rang, ili pozicioni ključ). To nije mana ideje nego njena granica: označava
gde relacija prelazi iz *računate* u *uskladištenu*.

## Smernica za dalji rad

Čuvamo osnovnu postavku: **broj zadržava sebe, a dodatnim okvirima dobija nove
mogućnosti.** Razvoj ide od koncepta ka primenama (organizacija, sortiranje,
pozicioni ključevi, i dalje), a ne obrnuto — ne prilagođavamo ideju unapred
izabranom algoritmu ili benchmarku.

**Status:** konceptualni okvir, u razvoju.
