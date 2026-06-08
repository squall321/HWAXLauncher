#requires -Version 5.1
<#
.SYNOPSIS
    Upload a signed installer to HEAXHub via a single multipart POST to
    /api/v1/apps/{AppId}/installers.

.DESCRIPTION
    One-step publish (no signing key, no long-lived credential in the payload):

      POST {base}/api/v1/apps/{AppId}/installers as multipart/form-data with the
      fields `version`, `os`, `signed`, the file under field name `installer`,
      and (optionally) the Tauri minisign `.sig` under field name `signature`.
      HEAXHub streams the bytes to its installer store, computes the SHA-256
      itself, and returns the package row JSON
      { id, app_id, version, os, installer_url, sha256, size_bytes, signed }.

    HEAXHub holds NO signing key — the signature is already baked into the
    artifact by scripts/sign.ps1; HEAXHub only records the SHA-256 and a `signed`
    flag. We re-verify the server-returned SHA-256 against the local hash so a
    corrupted upload is caught immediately.

    NOTE: the earlier presigned two-step (POST /api/v1/installers -> upload_url ->
    PUT to object storage) was never implemented server-side. The current
    deployment stores installers on local disk behind the multipart endpoint
    above (HEAXHub installers.py `upload_installer`).

    Auth: a bearer publish token from the environment ($env:HEAXHUB_PUBLISH_TOKEN,
    a CI SECRET). NOTHING is hardcoded here.

    Required environment variables:
      HEAXHUB_BASE_URL        e.g. https://hwax.sec.samsung.net/heax-hub (non-secret VAR)
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
    [string]$Version,

    [Parameter(Mandatory = $false)]
    [string]$Os = 'windows-x64',

    # Tauri minisign signature (what `tauri build` emits, e.g. "<file>.sig") —
    # uploaded so the updater feed (/api/v1/installers/hwax-agent/latest) can
    # serve a verifiable self-update. Defaults to "<FilePath>.sig" if present.
    # This is NOT the Authenticode signature (that is embedded in the .exe).
    [Parameter(Mandatory = $false)]
    [string]$SignaturePath
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

# Resolve the optional Tauri minisign .sig: explicit -SignaturePath wins, else
# auto-detect "<FilePath>.sig". Absent ⇒ upload installer only (the feed will
# 204 until a signed build is published).
if ([string]::IsNullOrWhiteSpace($SignaturePath)) {
    $maybeSig = "$FilePath.sig"
    if (Test-Path -LiteralPath $maybeSig) {
        $SignaturePath = (Resolve-Path -LiteralPath $maybeSig).Path
    }
}
elseif (-not (Test-Path -LiteralPath $SignaturePath)) {
    throw "Signature file not found: $SignaturePath"
}

Write-Host "[publish] $fileName  ($sizeBytes bytes)  app=$AppId  version=$cleanVersion"

# ── Upload: single multipart POST to /api/v1/apps/{AppId}/installers ──────────
# Windows PowerShell 5.1's Invoke-RestMethod has no -Form, so build the
# multipart/form-data body by hand into a MemoryStream. iso-8859-1 (latin1)
# round-trips raw bytes 1:1 through a .NET string, so text parts stay text while
# the binary file parts are written as raw bytes.
$uploadUri = "$baseUrl/api/v1/apps/$AppId/installers"
$boundary  = [System.Guid]::NewGuid().ToString()
$LF        = "`r`n"
$enc       = [System.Text.Encoding]::GetEncoding('iso-8859-1')
$ms        = New-Object System.IO.MemoryStream

function Add-Text {
    param([System.IO.MemoryStream]$Stream, [System.Text.Encoding]$Enc, [string]$Text)
    $b = $Enc.GetBytes($Text); $Stream.Write($b, 0, $b.Length)
}
function Add-FilePart {
    param([System.IO.MemoryStream]$Stream, [System.Text.Encoding]$Enc,
          [string]$Boundary, [string]$Lf, [string]$Name, [string]$Path)
    $name = [System.IO.Path]::GetFileName($Path)
    Add-Text $Stream $Enc "--$Boundary$Lf"
    Add-Text $Stream $Enc "Content-Disposition: form-data; name=`"$Name`"; filename=`"$name`"$Lf"
    Add-Text $Stream $Enc "Content-Type: application/octet-stream$Lf$Lf"
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $Stream.Write($bytes, 0, $bytes.Length)
    Add-Text $Stream $Enc $Lf
}

foreach ($field in @(
        @{ name = 'version'; value = $cleanVersion },
        @{ name = 'os';      value = $Os },
        @{ name = 'signed';  value = 'true' })) {
    Add-Text $ms $enc "--$boundary$LF"
    Add-Text $ms $enc "Content-Disposition: form-data; name=`"$($field.name)`"$LF$LF"
    Add-Text $ms $enc "$($field.value)$LF"
}
Add-FilePart $ms $enc $boundary $LF 'installer' $FilePath
if (-not [string]::IsNullOrWhiteSpace($SignaturePath)) {
    Add-FilePart $ms $enc $boundary $LF 'signature' $SignaturePath
    Write-Host "[publish] including signature: $([System.IO.Path]::GetFileName($SignaturePath))"
}
Add-Text $ms $enc "--$boundary--$LF"
$body = $ms.ToArray()

$authHeaders = @{ Authorization = "Bearer $token" }

Write-Host "[publish] uploading: POST $uploadUri (os=$Os, multipart, $sizeBytes bytes)"
$resp = Invoke-RestMethod -Method Post -Uri $uploadUri `
    -Headers $authHeaders `
    -ContentType "multipart/form-data; boundary=$boundary" `
    -Body $body

# Defense in depth: HEAXHub computes its own SHA-256 from the streamed bytes;
# confirm it matches what we signed and uploaded.
if ($null -eq $resp -or [string]::IsNullOrWhiteSpace($resp.sha256)) {
    throw "Publish API did not return a package row for '$fileName'."
}
if ($resp.sha256.ToLower() -ne $Sha256.ToLower()) {
    throw "Server SHA-256 '$($resp.sha256)' != local '$Sha256' for '$fileName'."
}

Write-Host "[publish] OK — '$fileName' published as $AppId $cleanVersion (id=$($resp.id), sha256 $($resp.sha256))."
Write-Host "[publish] installer_url: $($resp.installer_url)"
