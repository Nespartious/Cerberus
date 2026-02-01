# Cerberus Local Code Checks (PowerShell)
# Run this before committing to catch issues early
# Usage: .\scripts\Check-Code.ps1 [-Full]

param(
    [switch]$Full
)

$ErrorActionPreference = "Stop"

Write-Host "🐺 Cerberus Code Checks" -ForegroundColor Cyan
Write-Host "========================" -ForegroundColor Cyan

function Test-Check {
    param([string]$Name, [scriptblock]$Command)
    try {
        & $Command 2>&1 | Out-Null
        if ($LASTEXITCODE -eq 0) {
            Write-Host "  ✓ $Name" -ForegroundColor Green
            return $true
        } else {
            Write-Host "  ✗ $Name" -ForegroundColor Red
            return $false
        }
    } catch {
        Write-Host "  ✗ $Name - $_" -ForegroundColor Red
        return $false
    }
}

$allPassed = $true

# 1. Format check
Write-Host "`n📝 Checking code formatting..." -ForegroundColor Yellow
$fmtResult = cargo fmt --all -- --check 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✓ Code is formatted correctly" -ForegroundColor Green
} else {
    Write-Host "  ✗ Run 'cargo fmt --all' to fix formatting" -ForegroundColor Red
    $allPassed = $false
}

# 2. Clippy (catches AI hallucinations)
Write-Host "`n🔍 Running Clippy lints..." -ForegroundColor Yellow
$clippyArgs = @(
    "--all-targets", "--all-features", "--",
    "-D", "warnings",
    "-D", "clippy::unwrap_used",
    "-D", "clippy::expect_used", 
    "-D", "clippy::panic",
    "-D", "clippy::todo",
    "-D", "clippy::unimplemented"
)

if ($Full) {
    $clippyArgs += @("-W", "clippy::pedantic", "-A", "clippy::module_name_repetitions", "-A", "clippy::must_use_candidate")
}

$clippyOutput = cargo clippy @clippyArgs 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✓ Clippy checks passed" -ForegroundColor Green
} else {
    Write-Host "  ✗ Clippy found issues:" -ForegroundColor Red
    $clippyOutput | Where-Object { $_ -match "error|warning" } | Select-Object -First 10 | ForEach-Object { Write-Host "    $_" -ForegroundColor DarkRed }
    $allPassed = $false
}

# 3. Debug build
Write-Host "`n🔨 Checking debug build..." -ForegroundColor Yellow
$buildOutput = cargo build --all-targets 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✓ Debug build successful" -ForegroundColor Green
} else {
    Write-Host "  ✗ Debug build failed" -ForegroundColor Red
    $allPassed = $false
}

# 4. Full mode extras
if ($Full) {
    Write-Host "`n🚀 Checking release build..." -ForegroundColor Yellow
    $releaseOutput = cargo build --release 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  ✓ Release build successful" -ForegroundColor Green
    } else {
        Write-Host "  ✗ Release build failed" -ForegroundColor Red
        $allPassed = $false
    }

    Write-Host "`n📚 Checking documentation..." -ForegroundColor Yellow
    $env:RUSTDOCFLAGS = "-D warnings"
    $docOutput = cargo doc --no-deps 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  ✓ Documentation builds correctly" -ForegroundColor Green
    } else {
        Write-Host "  ⚠ Documentation has warnings" -ForegroundColor Yellow
    }
}

Write-Host "`n========================" -ForegroundColor Cyan
if ($allPassed) {
    Write-Host "All checks passed! ✨" -ForegroundColor Green
} else {
    Write-Host "Some checks failed. Fix issues before committing." -ForegroundColor Red
    exit 1
}
