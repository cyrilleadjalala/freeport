# Installe freeport sous Windows : télécharge le binaire de la dernière version,
# sans rien d'autre à installer. À lancer dans PowerShell :
#
#   irm https://raw.githubusercontent.com/cyrilleadjalala/freeport/main/install.ps1 | iex
#
# Variables d'environnement facultatives :
#   FREEPORT_VERSION      version à installer (ex. v0.1.2) ; défaut : la dernière
#   FREEPORT_INSTALL_DIR  dossier d'installation ; défaut : %LOCALAPPDATA%\freeport
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
# Windows PowerShell 5.1 n'active pas TLS 1.2 par défaut, requis par GitHub.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Repo = "cyrilleadjalala/freeport"
$Target = "x86_64-pc-windows-msvc"

if (-not [Environment]::Is64BitOperatingSystem) {
    throw "freeport : Windows 32 bits non pris en charge"
}

$Version = $env:FREEPORT_VERSION
if (-not $Version) {
    $Version = (Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ "User-Agent" = "freeport-install" }).tag_name
}
if ($Version -notmatch '^v\d') {
    throw "freeport : impossible de déterminer la dernière version (obtenu : '$Version')"
}

$Dir = $env:FREEPORT_INSTALL_DIR
if (-not $Dir) { $Dir = Join-Path $env:LOCALAPPDATA "freeport" }

$Name = "freeport-$Version-$Target"
$Tmp = Join-Path ([IO.Path]::GetTempPath()) ("freeport-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force $Tmp | Out-Null
try {
    Write-Host "Installation de freeport $Version ($Target) dans $Dir"
    $Zip = Join-Path $Tmp "freeport.zip"
    Invoke-WebRequest "https://github.com/$Repo/releases/download/$Version/$Name.zip" -OutFile $Zip -UseBasicParsing
    Expand-Archive $Zip -DestinationPath $Tmp -Force
    New-Item -ItemType Directory -Force $Dir | Out-Null
    Copy-Item (Join-Path $Tmp "$Name\freeport.exe") (Join-Path $Dir "freeport.exe") -Force
} finally {
    Remove-Item $Tmp -Recurse -Force -ErrorAction SilentlyContinue
}

# Ajoute le dossier au PATH de l'utilisateur (sans doublon), et à la session en cours.
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (";$UserPath;" -notlike "*;$Dir;*") {
    [Environment]::SetEnvironmentVariable("Path", ($UserPath.TrimEnd(';') + ";$Dir"), "User")
}
if (";$env:Path;" -notlike "*;$Dir;*") { $env:Path = "$env:Path;$Dir" }

& (Join-Path $Dir "freeport.exe") --version
