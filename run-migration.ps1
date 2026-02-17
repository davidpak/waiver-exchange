# Migration Runner Script for Waiver Exchange
# This script runs the bot system tables migration

param(
    [string]$DatabaseUrl = "postgresql://postgres:password@localhost:5432/waiver_exchange",
    [string]$MigrationFile = "engine/account-service/migrations/005_bot_system_tables.sql"
)

Write-Host "🚀 Running Waiver Exchange Migration" -ForegroundColor Green
Write-Host "=====================================" -ForegroundColor Green

# Check if psql is available
try {
    $psqlVersion = & psql --version 2>$null
    Write-Host "✅ PostgreSQL client found: $psqlVersion" -ForegroundColor Green
} catch {
    Write-Host "❌ PostgreSQL client (psql) not found in PATH" -ForegroundColor Red
    Write-Host "Please install PostgreSQL or add psql to your PATH" -ForegroundColor Yellow
    Write-Host "You can download PostgreSQL from: https://www.postgresql.org/download/" -ForegroundColor Yellow
    exit 1
}

# Check if migration file exists
if (-not (Test-Path $MigrationFile)) {
    Write-Host "❌ Migration file not found: $MigrationFile" -ForegroundColor Red
    exit 1
}

Write-Host "📁 Migration file: $MigrationFile" -ForegroundColor Blue
Write-Host "🗄️  Database URL: $DatabaseUrl" -ForegroundColor Blue

# Run the migration
Write-Host "🔄 Running migration..." -ForegroundColor Yellow
try {
    & psql $DatabaseUrl -f $MigrationFile
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✅ Migration completed successfully!" -ForegroundColor Green
        Write-Host "🎉 Bot system tables have been created" -ForegroundColor Green
    } else {
        Write-Host "❌ Migration failed with exit code: $LASTEXITCODE" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host "❌ Error running migration: $_" -ForegroundColor Red
    exit 1
}

Write-Host "🎯 Next steps:" -ForegroundColor Cyan
Write-Host "   1. Set up player ID mapping data" -ForegroundColor White
Write-Host "   2. Create House bot accounts" -ForegroundColor White
Write-Host "   3. Implement SportsDataIO Fetcher service" -ForegroundColor White
Write-Host "   4. Implement Reference Price Engine (RPE)" -ForegroundColor White
