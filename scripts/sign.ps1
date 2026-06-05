#requires -Version 5.1
<#
.SYNOPSIS
    Authenticode-sign a Windows artifact (.exe / .msi) using a code-signing
    certificate that lives in Azure Key Vault (or an internal HSM) — the key
    NEVER leaves the vault and NEVER touches this repo.

.DESCRIPTION
    Thin wrapper around AzureSignTool (https://github.com/vcsjones/AzureSignTool),
    which signs locally while the private key stays in Key Vault. Authentication
    is OIDC-first (federated workload identity in CI, no stored secret); a client
    secret is accepted only as a fallback for non-OIDC runners.

    Every value comes from the ENVIRONMENT (CI secrets) — there is NOTHING
    hardcoded here. See .github/workflows/build-and-sign.yml and
    docs/EDR-WHITELIST.md (signing posture).

    Required environment variables (set as GitHub Actions SECRETS):
      AZURE_KEY_VAULT_URL    e.g. https://<vault-name>.vault.azure.net
      AZURE_KEY_VAULT_CERT   the certificate NAME in the vault
      AZURE_CLIENT_ID        app registration / managed-identity client id
      AZURE_TENANT_ID        directory (tenant) id
    Optional:
      AZURE_CLIENT_SECRET    only if the runner cannot use OIDC (fallback)
      TIMESTAMP_URL          RFC-3161 timestamp authority (defaults below)

.PARAMETER FilePath
    Absolute path to the artifact to sign.

.PARAMETER TimestampUrl
    RFC-3161 timestamp server. Defaults to $env:TIMESTAMP_URL or DigiCert.

.EXAMPLE
    ./scripts/sign.ps1 -FilePath 'C:\out\HWAXAgent_1.0.3_x64-setup.exe'

.NOTES
    Windows PowerShell 5.1-safe: no ternary, no ?? / ?. , no && / || .
    NEVER pass a key/password as a literal argument — secrets come from env only.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$FilePath,

    [Parameter(Mandatory = $false)]
    [string]$TimestampUrl
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-RequiredEnv {
    param([string]$Name)
    $value = [System.Environment]::GetEnvironmentVariable($Name)
    if ([string]::IsNullOrWhiteSpace($value)) {
        throw "Required environment variable '$Name' is not set. " +
              "It must be provided as a CI secret — never hardcode it."
    }
    return $value
}

# ── Validate input ───────────────────────────────────────────────────────────
if (-not (Test-Path -LiteralPath $FilePath)) {
    throw "File to sign not found: $FilePath"
}
$FilePath = (Resolve-Path -LiteralPath $FilePath).Path

# ── Pull config from the environment (CI secrets) ────────────────────────────
$vaultUrl  = Get-RequiredEnv 'AZURE_KEY_VAULT_URL'
$certName  = Get-RequiredEnv 'AZURE_KEY_VAULT_CERT'
$clientId  = Get-RequiredEnv 'AZURE_CLIENT_ID'
$tenantId  = Get-RequiredEnv 'AZURE_TENANT_ID'

# Optional client secret (OIDC is preferred; this is the fallback path).
$clientSecret = [System.Environment]::GetEnvironmentVariable('AZURE_CLIENT_SECRET')

if ([string]::IsNullOrWhiteSpace($TimestampUrl)) {
    $envTs = [System.Environment]::GetEnvironmentVariable('TIMESTAMP_URL')
    if ([string]::IsNullOrWhiteSpace($envTs)) {
        $TimestampUrl = 'http://timestamp.digicert.com'
    }
    else {
        $TimestampUrl = $envTs
    }
}

# ── Locate AzureSignTool (install once per job if absent) ────────────────────
$azureSignTool = (Get-Command 'AzureSignTool' -ErrorAction SilentlyContinue)
if ($null -eq $azureSignTool) {
    Write-Host '[sign] AzureSignTool not found; installing as a dotnet global tool...'
    # dotnet is the *tool host* only — the SIGNED app remains Tauri/Rust. This
    # does not introduce a .NET dependency into the product.
    & dotnet tool install --global AzureSignTool
    if ($LASTEXITCODE -ne 0) { throw "Failed to install AzureSignTool (exit $LASTEXITCODE)." }
    $toolsPath = Join-Path $env:USERPROFILE '.dotnet\tools'
    if ($env:PATH -notlike "*$toolsPath*") { $env:PATH = "$toolsPath;$env:PATH" }
}

# ── Build the argument list. Secrets are passed by value from env vars only;
#    none of them are ever written to disk or logged. ─────────────────────────
$azArgs = @(
    'sign',
    '--azure-key-vault-url', $vaultUrl,
    '--azure-key-vault-certificate', $certName,
    '--azure-key-vault-client-id', $clientId,
    '--azure-key-vault-tenant-id', $tenantId,
    '--timestamp-rfc3161', $TimestampUrl,
    '--file-digest', 'sha256',
    '--verbose'
)

if (-not [string]::IsNullOrWhiteSpace($clientSecret)) {
    # Fallback (non-OIDC): client-secret auth.
    $azArgs += @('--azure-key-vault-client-secret', $clientSecret)
}
else {
    # Preferred: federated/managed identity — AzureSignTool picks up the OIDC
    # token from the environment, so no secret is supplied here.
    $azArgs += '--azure-key-vault-managed-identity'
}

$azArgs += $FilePath

Write-Host "[sign] Signing: $FilePath"
Write-Host "[sign] Vault:   $vaultUrl  (cert '$certName')"
Write-Host "[sign] Timestamp: $TimestampUrl"

& AzureSignTool @azArgs
if ($LASTEXITCODE -ne 0) {
    throw "AzureSignTool failed for '$FilePath' (exit $LASTEXITCODE)."
}

# ── Verify the resulting signature (publisher trust = EDR allow-list item 3) ──
$sig = Get-AuthenticodeSignature -LiteralPath $FilePath
if ($sig.Status -ne 'Valid') {
    throw "Post-sign verification FAILED for '$FilePath': status=$($sig.Status)."
}

$thumb = $sig.SignerCertificate.Thumbprint
Write-Host "[sign] OK — '$([System.IO.Path]::GetFileName($FilePath))' signed. Thumbprint: $thumb"
Write-Host "[sign] (Register this thumbprint with the EDR — see docs/EDR-WHITELIST.md item 3.)"
