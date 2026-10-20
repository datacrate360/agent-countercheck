# Install the countercheck command-line tool on Windows from GitHub Releases.
#
#   irm https://raw.githubusercontent.com/datacrate360/agent-countercheck/PROD/install.ps1 | iex
#
# Options (environment variables):
#   COUNTERCHECK_VERSION      version to install, e.g. 0.3.0 (default: the latest release)
#   COUNTERCHECK_INSTALL_DIR  where to put countercheck.exe (default: %LOCALAPPDATA%\Programs\countercheck)
#   COUNTERCHECK_BASE_URL     download from here instead of GitHub (mirrors, testing)
#
# Every download is checked against the release's SHA256SUMS.txt before anything is installed.
# Works in PowerShell 7 (pwsh) and Windows PowerShell 5.1. Errors stop the script with `throw`,
# never `exit`, so a pasted one-liner doesn't close your PowerShell window.

& {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue' # Invoke-WebRequest is very slow with the progress bar

    $Repo = 'datacrate360/agent-countercheck'
    $App = 'countercheck'

    function Say([string]$Message) { Write-Host "countercheck-install: $Message" }
    function Die([string]$Message) { throw "countercheck-install: error: $Message" }
    function Fetch([string]$Url, [string]$Dest) {
        try {
            Invoke-WebRequest -Uri $Url -OutFile $Dest -UseBasicParsing
        } catch {
            Die "download failed: $Url ($($_.Exception.Message))"
        }
    }

    # Windows PowerShell 5.1 may not offer TLS 1.2 by default; GitHub needs it.
    if ($PSVersionTable.PSVersion.Major -lt 6) {
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    }

    $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    switch ($arch) {
        'AMD64' { $platform = 'windows-x86_64' }
        'ARM64' { $platform = 'windows-x86_64'; Say 'no ARM64 build yet; installing the x86_64 build (Windows 11 runs it with emulation)' }
        default { Die "no build for Windows on $arch yet" }
    }

    $version = $env:COUNTERCHECK_VERSION
    if (-not $version) {
        try {
            $latest = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
        } catch {
            Die 'could not look up the latest release; set COUNTERCHECK_VERSION'
        }
        $version = [string]$latest.tag_name
        if (-not $version) { Die 'could not find the latest version; set COUNTERCHECK_VERSION' }
    }
    $version = $version -replace '^v', ''

    $base = if ($env:COUNTERCHECK_BASE_URL) { $env:COUNTERCHECK_BASE_URL.TrimEnd('/') } else { "https://github.com/$Repo/releases/download/v$version" }
    $installDir = if ($env:COUNTERCHECK_INSTALL_DIR) { $env:COUNTERCHECK_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\$App" }
    $asset = "$App-$version-$platform.zip"

    $work = Join-Path ([IO.Path]::GetTempPath()) ("countercheck-install-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        Say "downloading $asset"
        Fetch "$base/$asset" (Join-Path $work $asset)
        Fetch "$base/SHA256SUMS.txt" (Join-Path $work 'SHA256SUMS.txt')

        # Lines are "<sha256>  <file>" (or "<sha256> *<file>").
        $expected = $null
        foreach ($line in Get-Content (Join-Path $work 'SHA256SUMS.txt')) {
            $parts = $line.Trim() -split '\s+', 2
            if ($parts.Count -eq 2 -and ($parts[1] -eq $asset -or $parts[1] -eq "*$asset")) {
                $expected = $parts[0].ToLowerInvariant()
            }
        }
        if (-not $expected) { Die "$asset is not listed in SHA256SUMS.txt" }
        $actual = (Get-FileHash -Path (Join-Path $work $asset) -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($expected -ne $actual) { Die "checksum mismatch for $asset (expected $expected, got $actual); nothing was installed" }
        Say 'checksum ok'

        $unpacked = Join-Path $work 'unpacked'
        Expand-Archive -Path (Join-Path $work $asset) -DestinationPath $unpacked
        $exe = Join-Path $unpacked "$App.exe"
        if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { Die "the archive has no $App.exe" }

        New-Item -ItemType Directory -Path $installDir -Force | Out-Null
        $target = Join-Path $installDir "$App.exe"
        Copy-Item -LiteralPath $exe -Destination $target -Force
        # Downloaded files carry a "from the internet" mark; this one was checked against SHA256SUMS.txt.
        Unblock-File -LiteralPath $target -ErrorAction SilentlyContinue

        $installed = & $target --version
        Say "installed $installed to $target"
    } finally {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }

    $onPath = ($env:Path -split ';') | Where-Object { $_ -and ($_.TrimEnd('\') -ieq $installDir.TrimEnd('\')) }
    if (-not $onPath) {
        Say 'add it to your PATH (then open a new terminal):'
        Say "  [Environment]::SetEnvironmentVariable('Path', `"$installDir;`" + [Environment]::GetEnvironmentVariable('Path', 'User'), 'User')"
    }
    Say 'next: cd your-project; countercheck init; countercheck verify'
}
