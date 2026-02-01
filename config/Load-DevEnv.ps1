# Load Cerberus Development Environment Variables
# Usage: . .\config\Load-DevEnv.ps1

Write-Host "Loading Cerberus dev environment..." -ForegroundColor Cyan

# Backend Configuration
$env:CERBERUS_BACKEND_URL = "http://sigilahzwq5u34gdh2bl3ymokyc7kobika55kyhztsucdoub73hz7qid.onion/"
$env:CERBERUS_VANITY = "sigil"
$env:CERBERUS_SERVICE_NAME = "Sigil"

# Local Development
$env:CERBERUS_REDIS_URL = "redis://127.0.0.1:6379"
$env:CERBERUS_LISTEN_ADDR = "127.0.0.1:8888"
$env:CERBERUS_LOG_LEVEL = "debug"
$env:CERBERUS_THREAT_LEVEL = "5"

# Testing
$env:CERBERUS_TEST_CIRCUIT = "test-circuit-sigil-001"
$env:CERBERUS_MOCK_BACKEND_PORT = "8082"

# Feature Flags
$env:CERBERUS_BYPASS_CAPTCHA = "false"
$env:CERBERUS_VERBOSE_REQUESTS = "true"

Write-Host "Environment loaded for: $($env:CERBERUS_SERVICE_NAME) ($($env:CERBERUS_VANITY))" -ForegroundColor Green
Write-Host "  Backend: $($env:CERBERUS_BACKEND_URL)" -ForegroundColor Gray
Write-Host "  Redis:   $($env:CERBERUS_REDIS_URL)" -ForegroundColor Gray
Write-Host "  Fortify: $($env:CERBERUS_LISTEN_ADDR)" -ForegroundColor Gray
