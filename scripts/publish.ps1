#requires -Version 5.1
<#
.SYNOPSIS
    Upload a signed installer to HEAXHub's installer_packages via a presigned
    PUT, then register the package metadata (version + SHA-256).

.DESCRIPTION
    Two-step publish (no signing key, no long-lived credential in the payload):

      1. Ask HEAXHub for a presigned PUT URL for this artifact. HEAXHub records
         the package row (app_id, version, sha256, size_bytes, signed=true) and
         returns a short-lived URL to object storage.
      2. PUT the file bytes directly to that presigned URL (object storage),
         bypassing the API for the large transfer.

    Mirrors the build pipeline in HEAXHub plan-v2 §21 and split-strategy §8.1:
    HEAXHub holds NO signing key — it only stores the SHA-256 and a `signed`
    flag; the signature is already baked into the artifact by scripts/sign.ps1.

    Auth: a bearer publish token from the environment ($env:HEAXHUB_PUBLISH_TOKEN,
    a CI SECRET). The presigned URL itself carries its own signature, so the file
    PUT sends no bearer. NOTHING is hardcoded here.

    Required environment variables:
      HEAXHUB_BASE_URL        e.g. https://heaxhub.local   (non-secret VAR)
      HEAXHUB_PUBLISH_TOKEN   bearer token for the publish API (SECRET)

.PARAMETER FilePath
    Absolute path to the signed artifact (.exe / .msi).

.PARAMETER Sha256
    Lowercase hex SHA-256 of the artifact (computed by the workflow).

.PARAMETER AppId
    Catalog app id. Defaults to 'hwax-agent'.

.PARAMETER Version
    SemVer of the release, e.g. '1.0.3' (a leading 'v' is stripped).

.EXAMPLE
    ./scripts/publish.ps1 -FilePath 'C:\out\HWAXAgent_1.0.3_x64-setup.exe' `
      -Sha256 'abc...123' -AppId 'hwax-agent' -Version 'v1.0.3'

.NOTES
    Windows PowerShell 5.1-safe: no ternary, no ?? / ?. , no && / || .
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$FilePath,

    [Parameter(Mandatory = $true)]
    [string]$Sha256,

    [Parameter(Mandatory = $false)]
    [string]$AppId = 'hwax-agent',

    [Parameter(Mandatory = $true)]
    [string]$Version
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-RequiredEnv {
    param([string]$Name)
    $value = [System.Environment]::GetEnvironmentVariable($Name)
    if ([string]::IsNullOrWhiteSpace($value)) {
        throw "Required environment variable '$Name' is not set. " +
              "It must be provided as a CI secret/var — never hardcode it."
    }
    return $value
}

# ── Validate input ───────────────────────────────────────────────────────────
if (-not (Test-Path -LiteralPath $FilePath)) {
    throw "Artifact not found: $FilePath"
}
$FilePath = (Resolve-Path -LiteralPath $FilePath).Path
$fileName = [System.IO.Path]::GetFileName($FilePath)
$sizeBytes = (Get-Item -LiteralPath $FilePath).Length

if ($Sha256 -notmatch '^[0-9a-f]{64}$') {
    throw "Sha256 must be 64 lowercase hex chars; got '$Sha256'."
}

# Normalize version: strip a leading 'v' (tags arrive as v1.0.3).
$cleanVersion = $Version
if ($cleanVersion.StartsWith('v')) { $cleanVersion = $cleanVersion.Substring(1) }

$baseUrl = (Get-RequiredEnv 'HEAXHUB_BASE_URL').TrimEnd('/')
$token   = Get-RequiredEnv 'HEAXHUB_PUBLISH_TOKEN'

# Re-verify the SHA-256 locally so we never publish a hash that doesn't match
# the bytes we are about to upload (defense in depth — see EDR-WHITELIST.md).
$actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $FilePath).Hash.ToLower()
if ($actual -ne $Sha256.ToLower()) {
    throw "Local SHA-256 mismatch for '$fileName': expected $Sha256, computed $actual."
}

Write-Host "[publish] $fileName  ($sizeBytes bytes)  app=$AppId  version=$cleanVersion"

# ── Step 1: register the package row and get a presigned PUT URL ──────────────
$registerUri = "$baseUrl/api/v1/installers"
$registerBody = @{
    app_id      = $AppId
    version     = $cleanVersion
    file_name   = $fileName
    sha256      = $Sha256.ToLower()
    size_bytes  = $sizeBytes
    signed      = $true
    format      = ([System.IO.Path]::GetExtension($fileName)).TrimStart('.').ToLower()
} | ConvertTo-Json -Compress

$authHeaders = @{ Authorization = "Bearer $token" }

Write-Host "[publish] requesting presigned URL: POST $registerUri"
$register = Invoke-RestMethod -Method Post -Uri $registerUri `
    -Headers $authHeaders -ContentType 'application/json' -Body $registerBody

if ($null -eq $register -or [string]::IsNullOrWhiteSpace($register.upload_url)) {
    throw "Publish API did not return an upload_url for '$fileName'."
}
$uploadUrl = $register.upload_url

# ── Step 2: PUT the bytes to the presigned object-storage URL ────────────────
# The presigned URL carries its own auth — do NOT attach the bearer here.
Write-Host "[publish] uploading bytes to presigned URL (object storage)..."
Invoke-RestMethod -Method Put -Uri $uploadUrl `
    -InFile $FilePath -ContentType 'application/octet-stream' | Out-Null

Write-Host "[publish] OK — '$fileName' published as $AppId $cleanVersion (sha256 $Sha256)."
