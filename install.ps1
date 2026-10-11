# Install freeport on Windows: downloads the binary of the latest release,
# checks its SHA-256, and needs nothing else installed. In PowerShell:
#
#   irm https://raw.githubusercontent.com/cyrilleadjalala/freeport/main/install.ps1 | iex
#
# Optional environment variables:
#   FREEPORT_VERSION        version to install (e.g. v0.2.0); default: the latest
#   FREEPORT_INSTALL_DIR    install directory; default: %LOCALAPPDATA%\freeport
#   FREEPORT_SKIP_VERIFY=1  skip the SHA-256 check (not recommended)
#   FREEPORT_LANG           en or fr (default: the Windows display language)
#   FREEPORT_BASE_URL       release URL prefix, for testing only
#
# Everything runs in a child scope so the preferences below do not leak into
# the caller's session when the script is piped to `iex`.
& {
    $ErrorActionPreference = "Stop"
    $ProgressPreference = "SilentlyContinue"
    # Windows PowerShell 5.1 does not enable TLS 1.2 by default; GitHub requires it.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

    $Repo = "cyrilleadjalala/freeport"
    $Base = if ($env:FREEPORT_BASE_URL) { $env:FREEPORT_BASE_URL } else { "https://github.com/$Repo/releases" }
    $Target = "x86_64-pc-windows-msvc"

    $LangCode = if ($env:FREEPORT_LANG) { $env:FREEPORT_LANG } else { (Get-Culture).TwoLetterISOLanguageName }
    $IsFrench = "$LangCode" -like "fr*"
    function Say($En, $FrText) { if ($IsFrench) { $FrText } else { $En } }
    function Fail($En, $FrText) { throw ("freeport: " + (Say $En $FrText)) }

    if (-not [Environment]::Is64BitOperatingSystem) {
        Fail "32-bit Windows is not supported" "Windows 32 bits non pris en charge"
    }

    # The latest version, read from the redirect of /releases/latest (no API rate limit).
    function Get-LatestTag {
        try {
            $Req = [Net.HttpWebRequest]::Create("$Base/latest")
            $Req.AllowAutoRedirect = $false
            $Req.UserAgent = "freeport-install"
            $Resp = $Req.GetResponse()
            try { $Loc = $Resp.Headers["Location"] } finally { $Resp.Close() }
            if ($Loc) { return ($Loc.TrimEnd('/') -split '/')[-1] }
        } catch { }
        try {
            return (Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ "User-Agent" = "freeport-install" }).tag_name
        } catch { return $null }
    }

    $Version = $env:FREEPORT_VERSION
    if (-not $Version) { $Version = Get-LatestTag }
    if ($Version -notmatch '^v\d') {
        Fail "cannot determine the latest version (got: '$Version')" "impossible de déterminer la dernière version (obtenu : '$Version')"
    }

    $Dir = $env:FREEPORT_INSTALL_DIR
    if (-not $Dir) { $Dir = Join-Path $env:LOCALAPPDATA "freeport" }

    $Name = "freeport-$Version-$Target"
    $Url = "$Base/download/$Version/$Name.zip"
    $Tmp = Join-Path ([IO.Path]::GetTempPath()) ("freeport-" + [Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force $Tmp | Out-Null
    try {
        Write-Host (Say "Installing freeport $Version ($Target) into $Dir" "Installation de freeport $Version ($Target) dans $Dir")
        $Zip = Join-Path $Tmp "freeport.zip"
        try {
            Invoke-WebRequest $Url -OutFile $Zip -UseBasicParsing
        } catch {
            Fail "download failed: does version $Version exist for $Target?" "téléchargement impossible : la version $Version existe-t-elle pour $Target ?"
        }

        if ($env:FREEPORT_SKIP_VERIFY -eq "1") {
            Write-Warning (Say "SHA-256 check skipped (FREEPORT_SKIP_VERIFY=1)" "vérification SHA-256 ignorée (FREEPORT_SKIP_VERIFY=1)")
        } else {
            $SumFile = Join-Path $Tmp "freeport.sha256"
            $HasSum = $true
            try { Invoke-WebRequest "$Url.sha256" -OutFile $SumFile -UseBasicParsing } catch { $HasSum = $false }
            if ($HasSum) {
                $Expected = ((Get-Content $SumFile -Raw).Trim() -split '\s+')[0].ToLower()
                if ($Expected -notmatch '^[0-9a-f]{64}$') {
                    Fail "malformed checksum file for $Version" "fichier d'empreinte invalide pour $Version"
                }
                $Actual = (Get-FileHash $Zip -Algorithm SHA256).Hash.ToLower()
                if ($Actual -ne $Expected) {
                    Fail "SHA-256 mismatch, refusing to install (expected $Expected, got $Actual)" "SHA-256 différent, installation refusée (attendu $Expected, obtenu $Actual)"
                }
                Write-Host (Say "SHA-256 verified" "SHA-256 vérifié")
            } elseif (([version]($Version.TrimStart('v') -replace '[-+].*$', '')) -ge [version]'0.2.0') {
                Fail "no checksum is published for $Version, refusing to install" "aucune empreinte publiée pour $Version, installation refusée"
            } else {
                Write-Warning (Say "no checksum is published for $Version (releases before 0.2.0 have none), continuing" "aucune empreinte publiée pour $Version (les versions antérieures à 0.2.0 n'en ont pas), poursuite")
            }
        }

        Expand-Archive $Zip -DestinationPath $Tmp -Force
        New-Item -ItemType Directory -Force $Dir | Out-Null
        Copy-Item (Join-Path $Tmp "$Name\freeport.exe") (Join-Path $Dir "freeport.exe") -Force
    } finally {
        Remove-Item $Tmp -Recurse -Force -ErrorAction SilentlyContinue
    }

    # Add the directory to the user's PATH (once), and to the current session.
    $UserPath = [string][Environment]::GetEnvironmentVariable("Path", "User")
    if (";$UserPath;" -notlike "*;$Dir;*") {
        $NewPath = if ($UserPath) { $UserPath.TrimEnd(';') + ";$Dir" } else { $Dir }
        [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    }
    if (";$env:Path;" -notlike "*;$Dir;*") { $env:Path = "$env:Path;$Dir" }

    & (Join-Path $Dir "freeport.exe") --version
}
