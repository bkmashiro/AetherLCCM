param(
    [switch]$SkipPython
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$vcvars = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
$protocolManifest = Join-Path $repoRoot "protocol\Cargo.toml"

Write-Host "[phase1] rust: initialize MSVC toolchain + run protocol tests"
if (-not (Test-Path $vcvars)) {
    Write-Error "vcvars script not found: $vcvars"
}
cmd /c "`"$vcvars`" && cargo test --manifest-path `"$protocolManifest`""

if (-not $SkipPython) {
    Write-Host "[phase1] python: run simulation tests"
    $simRoot = Join-Path $repoRoot "sim"
    Push-Location $simRoot
    if (Get-Command python -ErrorAction SilentlyContinue) {
        python -m unittest discover -s tests -p "test_*.py" -v
        if ($LASTEXITCODE -ne 0) {
            Pop-Location
            exit $LASTEXITCODE
        }
    }
    else {
        Write-Warning "python not available; skipping pytest"
    }
    Pop-Location
}

Write-Host "[phase1] done"
