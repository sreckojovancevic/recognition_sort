# Radix LENS - komparativni benchmark: standardni sortovi u memoriji + sve faze eksternog sortiranja
# Pokretanje iz E:\samo\suit:  powershell -ExecutionPolicy Bypass -File .\run_compare.ps1
# Samo poređenje u memoriji:  powershell -ExecutionPolicy Bypass -File .\run_compare.ps1 -OnlyMem
param([switch]$OnlyMem)
$ErrorActionPreference = "Continue"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8   # ispis programa je UTF-8
$pause = 10   # sekunde pauze između metoda i između faza
Set-Location $PSScriptRoot
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "build nije uspeo" }

$exe   = ".\target\release\radix_lens_bench.exe"
$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
New-Item -ItemType Directory -Force -Path ".\logs" | Out-Null
$log     = ".\logs\compare_$stamp.log"
$summary = ".\logs\compare_${stamp}_summary.csv"

$common = @("--elements","100000000","--block-size-mb","32","--read-buffer-kb","2048","--write-buffer-kb","4096","--seed","42")
$ext    = @("--scenario","external","--target-dir","C:\temp_sort_pipe1","--repetitions","15")

# Oznaka, opis, argumenti (svaka faza = prethodna + jedna izmena)
$runs = @(
  @{ L="mem";                 D="Standardni sortovi u memoriji (5 ponavljanja)";             A=@("--compare-std","--threads","8","--repetitions","5","--pause-secs","$pause") },
  @{ L="f1-pocetak";          D="OptimizedHeap, sekvencijalna priprema, std sort";            A=$ext+@("--threads","8","--strategy","optimized-heap") },
  @{ L="f2-pipeline";         D="+ pipeline dubine 0";                                        A=$ext+@("--threads","8","--strategy","optimized-heap","--pipeline","--pipeline-depth","0") },
  @{ L="f3-packed-loser-tree";D="+ PackedLoserTree (sekvencijalni merge)";                    A=$ext+@("--threads","8","--strategy","packed-loser-tree","--pipeline","--pipeline-depth","0") },
  @{ L="f4-paralelni-merge";  D="+ paralelni merge, 8 niti";                                  A=$ext+@("--threads","8","--strategy","packed-loser-tree","--pipeline","--pipeline-depth","0","--merge-threads","8") },
  @{ L="f5-radix-2-writera";  D="+ radix sort blokova, 2 writer niti";                        A=$ext+@("--threads","8","--strategy","packed-loser-tree","--pipeline","--pipeline-depth","0","--merge-threads","8","--block-sort","radix","--writers","2") },
  @{ L="f6-threads-6";        D="+ 6 rayon niti (preporučena konfiguracija)";                 A=$ext+@("--threads","6","--strategy","packed-loser-tree","--pipeline","--pipeline-depth","0","--merge-threads","8","--block-sort","radix","--writers","2") }
)

"Radix LENS - komparativni benchmark, $stamp" | Out-File $log -Encoding utf8
"Napomena: faze f1-f6 koriste trenutni kod; refaktor merge koda (11.10.) je ugrađen u sve, pa se ne vidi kao zaseban korak." | Out-File $log -Append -Encoding utf8
"Napomena: sort u memoriji drži ceo niz i ne radi I/O; eksterno sortiranje radi sa ~150 MiB i upisuje/čita fajlove." | Out-File $log -Append -Encoding utf8
"oznaka,opis,metoda,vrsta,ukupno_med_ms,min_ms,max_ms,priprema_med_ms,merge_med_ms,mem_mib,ispravno" | Out-File $summary -Encoding utf8

$i = 0
if ($OnlyMem) { $runs = @($runs[0]) }
foreach ($r in $runs) {
  $i++
  if ($i -gt 1) { Write-Host "... pauza $pause s" -ForegroundColor DarkGray; Start-Sleep -Seconds $pause }
  Write-Host ("=== [{0}/{1}] {2}: {3} ===" -f $i, $runs.Count, $r.L, $r.D) -ForegroundColor Cyan
  "`n=== $($r.L): $($r.D) ===" | Out-File $log -Append -Encoding utf8
  $argv = $common + $r.A + @("--label", $r.L)
  $out = & $exe @argv 2>&1 | ForEach-Object { "$_" }
  $out | Out-File $log -Append -Encoding utf8
  $out | Where-Object { $_ -notlike "SUMMARY,*" } | Write-Host
  foreach ($line in ($out | Where-Object { $_ -like "SUMMARY,*" })) {
    $f = $line.Substring(8).Split(",")
    # f: oznaka, metoda, vrsta, ukupno, min, max, priprema, merge, mem, ispravno
    ($f[0], ('"' + $r.D + '"'), $f[1], $f[2], $f[3], $f[4], $f[5], $f[6], $f[7], $f[8], $f[9]) -join "," | Out-File $summary -Append -Encoding utf8
  }
}
Write-Host "Gotovo. Log: $log  |  Zbir: $summary" -ForegroundColor Green
