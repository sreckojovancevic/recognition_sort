# Radix LENS - matrica testova: disk x dubina x fsync (OptimizedHeap, 15 ponavljanja)
# Pokretanje iz E:\samo\suit:  powershell -ExecutionPolicy Bypass -File .\run_disks.ps1
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
cargo build --release
$exe  = ".\target\release\radix_lens_bench.exe"
$logs = ".\logs"
New-Item -ItemType Directory -Force -Path $logs | Out-Null

$disks = @(
    @{ Name = "C_nvme"; Dir = "C:\temp_sort_pipe1" },
    @{ Name = "D_ssd";  Dir = "D:\temp_sort_pipe1" },
    @{ Name = "E_hdd";  Dir = "E:\temp_sort_pipe1" }
)

foreach ($d in $disks) {
    foreach ($fs in @($false, $true)) {
        foreach ($depth in @(0, 1)) {
            $tag  = "{0}_d{1}_{2}" -f $d.Name, $depth, ($(if ($fs) { "fsync" } else { "nofsync" }))
            $argv = @("--scenario","external","--target-dir",$d.Dir,"--elements","100000000",
                      "--block-size-mb","32","--read-buffer-kb","2048","--write-buffer-kb","4096",
                      "--threads","8","--repetitions","15","--pipeline","--pipeline-depth","$depth",
                      "--strategy","optimized-heap")
            if ($fs) { $argv += "--fsync" }
            Write-Host "=== $tag ===" -ForegroundColor Cyan
            & $exe @argv 2>&1 | Tee-Object -FilePath "$logs\$tag.txt"
        }
    }
}
Write-Host "Gotovo. Logovi su u $logs" -ForegroundColor Green
