# Tests install.ps1 offline against a fake release served locally.
#   pwsh tests/install_test.ps1      (or: powershell -File tests/install_test.ps1)
$ErrorActionPreference = "Stop"
$Root = Split-Path $PSScriptRoot -Parent
$Shell = (Get-Process -Id $PID).Path
$Work = Join-Path ([IO.Path]::GetTempPath()) ("freeport-test-" + [Guid]::NewGuid().ToString("N"))
$Srv = Join-Path $Work "srv"; $Build = Join-Path $Work "build"; $Bin = Join-Path $Work "bin"
New-Item -ItemType Directory -Force $Srv, $Build | Out-Null
$Target = "x86_64-pc-windows-msvc"
$Python = if (Get-Command python -ErrorAction SilentlyContinue) { "python" } else { "python3" }

# A fake release: a real, runnable exe plus a marker so each version differs.
function Make-Release($V, $Marker) {
    $Name = "freeport-$V-$Target"
    New-Item -ItemType Directory -Force (Join-Path $Build $Name), (Join-Path $Srv "download\$V") | Out-Null
    $Exe = Join-Path $Build "$Name\freeport.exe"
    Copy-Item (Join-Path $env:SystemRoot "System32\hostname.exe") $Exe -Force
    $Bytes = [Text.Encoding]::ASCII.GetBytes($Marker)
    $Fs = [IO.File]::Open($Exe, 'Append'); $Fs.Write($Bytes, 0, $Bytes.Length); $Fs.Close()
    $Zip = Join-Path $Srv "download\$V\$Name.zip"
    if (Test-Path $Zip) { Remove-Item $Zip }
    Compress-Archive -Path (Join-Path $Build $Name) -DestinationPath $Zip
    return $Zip
}
function Add-Sum($Zip) {
    $Hash = (Get-FileHash $Zip -Algorithm SHA256).Hash.ToLower()
    Set-Content "$Zip.sha256" "$Hash  $(Split-Path $Zip -Leaf)" -Encoding ascii
}

Make-Release "v0.1.9" "old" | Out-Null                                  # old release: no checksum
Add-Sum (Make-Release "v0.2.0" "good")                                  # good release
Make-Release "v0.2.1" "nosum" | Out-Null                                # new release, checksum missing
$Z = Make-Release "v0.2.2" "original"; Add-Sum $Z; Make-Release "v0.2.2" "tampered" | Out-Null  # replaced after the sum
$Z = Make-Release "v0.2.3" "badsum"; Set-Content "$Z.sha256" "not-a-hash" -Encoding ascii

$Port = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$Port.Start(); $PortNo = $Port.LocalEndpoint.Port; $Port.Stop()
$Server = Start-Process $Python -ArgumentList "-m", "http.server", $PortNo, "--bind", "127.0.0.1" -WorkingDirectory $Srv -PassThru -WindowStyle Hidden
for ($i = 0; $i -lt 50; $i++) {
    try { Invoke-WebRequest "http://127.0.0.1:$PortNo/" -UseBasicParsing | Out-Null; break } catch { Start-Sleep -Milliseconds 100 }
}

$Failed = 0
function Check($Ok, $Label) {
    if ($Ok) { Write-Host "ok   - $Label" } else { Write-Host "FAIL - $Label"; $script:Failed = 1 }
}
# Run install.ps1 the way users do (piped to iex) in a fresh process.
function Run($V, $Vars = @{}) {
    Remove-Item $Bin -Recurse -Force -ErrorAction SilentlyContinue
    $env:FREEPORT_BASE_URL = "http://127.0.0.1:$PortNo"; $env:FREEPORT_VERSION = $V
    $env:FREEPORT_INSTALL_DIR = $Bin; $env:FREEPORT_LANG = "en"; $env:FREEPORT_SKIP_VERIFY = $null
    foreach ($K in $Vars.Keys) { Set-Item "env:$K" $Vars[$K] }
    $script:Out = (& $Shell -NoProfile -Command "Get-Content '$Root\install.ps1' -Raw | Invoke-Expression" *>&1 | Out-String)
    $script:Status = $LASTEXITCODE
}

try {
    Run "v0.2.0"
    Check (($Status -eq 0) -and (Test-Path "$Bin\freeport.exe") -and ($Out -match "SHA-256 verified")) "valid release installs and is verified"
    Run "v0.2.2"
    Check (($Status -ne 0) -and -not (Test-Path "$Bin\freeport.exe") -and ($Out -match "mismatch")) "altered archive is refused"
    Run "v0.2.2" @{ FREEPORT_SKIP_VERIFY = "1" }
    Check (($Status -eq 0) -and (Test-Path "$Bin\freeport.exe") -and ($Out -match "skipped")) "FREEPORT_SKIP_VERIFY=1 skips the check"
    Run "v0.2.1"
    Check (($Status -ne 0) -and -not (Test-Path "$Bin\freeport.exe") -and ($Out -match "no checksum is published")) "missing checksum is refused from 0.2.0 on"
    Run "v0.1.9"
    Check (($Status -eq 0) -and (Test-Path "$Bin\freeport.exe") -and ($Out -match "before 0.2.0 have none")) "old release without checksum installs with a warning"
    Run "v0.2.3"
    Check (($Status -ne 0) -and ($Out -match "malformed")) "malformed checksum file is refused"
    Run "v9.9.9"
    Check (($Status -ne 0) -and ($Out -match "download failed")) "unknown version gives a clear error"
    Run "v0.2.0" @{ FREEPORT_LANG = "fr" }
    Check (($Status -eq 0) -and ($Out -match "SHA-256 vérifié")) "messages follow FREEPORT_LANG=fr"
    Run "v0.2.0"
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    Check ($UserPath -like "*$Bin*") "install directory is added to the user PATH"
} finally {
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($UserPath) { [Environment]::SetEnvironmentVariable("Path", (($UserPath -split ';' | Where-Object { $_ -and $_ -ne $Bin }) -join ';'), "User") }
    Stop-Process -Id $Server.Id -Force -ErrorAction SilentlyContinue
    Remove-Item $Work -Recurse -Force -ErrorAction SilentlyContinue
}
exit $Failed
