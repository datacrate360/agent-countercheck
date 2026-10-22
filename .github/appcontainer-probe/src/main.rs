//! Run the same PowerShell probe in an AppContainer three ways and print what worked.
//! Output lines: `<setup>: <probe> = ok|FAILED (<why>)`.

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only");
}

#[cfg(windows)]
fn main() {
    use rappct::{
        AppContainerProfile, KnownCapability, LaunchOptions, SecurityCapabilitiesBuilder,
        StdioConfig, launch_in_container_with_io,
    };

    let python = std::env::var("PROBE_PYTHON").unwrap_or_default();
    // Results go to a file in a folder granted to the container (no pipes, no custom environment).
    let out_dir = std::env::temp_dir().join(format!("acprobe-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&out_dir);
    let make_script = |setup: &str| format!(
        r#"
$ErrorActionPreference = 'Stop'
$out = '{out}\{setup}.txt'
function Probe($name, $block) {{
  try {{ $r = & $block; Add-Content -Path $out -Value "{setup}: $name = ok $r" }}
  catch {{ Add-Content -Path $out -Value "{setup}: $name = FAILED ($($_.Exception.Message))" }}
}}
Add-Content -Path $out -Value "{setup}: started"
Probe 'internet 1.1.1.1:443' {{ $c = [Net.Sockets.TcpClient]::new(); if (-not $c.ConnectAsync('1.1.1.1', 443).Wait(8000)) {{ throw 'timeout' }}; $c.Close() }}
Probe 'localhost, same process' {{ $l = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0); $l.Start(); $p = $l.LocalEndpoint.Port; $c = [Net.Sockets.TcpClient]::new(); if (-not $c.ConnectAsync('127.0.0.1', $p).Wait(5000)) {{ throw 'timeout' }}; $c.Close(); $l.Stop() }}
Probe 'localhost, another process' {{
  $l = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0); $l.Start(); $p = $l.LocalEndpoint.Port
  $code = "`$c = [Net.Sockets.TcpClient]::new(); if (-not `$c.ConnectAsync('127.0.0.1', $p).Wait(5000)) {{ exit 3 }}; exit 0"
  $proc = Start-Process -FilePath "$PSHOME\powershell.exe" -ArgumentList '-NoProfile','-Command',$code -NoNewWindow -Wait -PassThru
  $l.Stop(); if ($proc.ExitCode -ne 0) {{ throw "client exit $($proc.ExitCode)" }}
}}
Probe 'Git sh' {{ $o = & 'C:\Program Files\Git\bin\sh.exe' -c 'echo hi' 2>&1; if ($LASTEXITCODE -ne 0) {{ throw "exit $LASTEXITCODE $o" }}; "($o)" }}
Probe 'Python ({python})' {{ $o = & '{python}' -c 'print(1)' 2>&1; if ($LASTEXITCODE -ne 0) {{ throw "exit $LASTEXITCODE $o" }} }}
Probe 'write %TEMP%' {{ Set-Content -Path (Join-Path $env:TEMP 'appcontainer-probe.txt') -Value x }}
"#,
        out = out_dir.display()
    );
    let encode = |script: String| {
        base64(
            &script
                .encode_utf16()
                .flat_map(|u| u.to_le_bytes())
                .collect::<Vec<u8>>(),
        )
    };
    let ps = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
    let name = format!("countercheck.probe.{}", std::process::id());
    let profile = match AppContainerProfile::ensure(&name, "countercheck probe", None) {
        Ok(p) => p,
        Err(e) => {
            println!("could not create an AppContainer profile: {e}");
            return;
        }
    };
    let sid = profile.sid.as_string().to_string();
    if let Err(e) = rappct::acl::grant_to_package(
        rappct::acl::ResourcePath::Directory(out_dir.clone()),
        &profile.sid,
        rappct::acl::AccessMask::GENERIC_ALL,
    ) {
        println!("could not grant the output folder: {e:?}");
    }
    let run = |setup: &str, caps: &[KnownCapability]| {
        let sec = match SecurityCapabilitiesBuilder::new(&profile.sid)
            .with_known(caps)
            .build()
        {
            Ok(s) => s,
            Err(e) => {
                println!("{setup}: capabilities: {e}");
                return;
            }
        };
        let encoded = encode(make_script(setup));
        for (variant, stdio) in [("stdio null", StdioConfig::Null), ("stdio inherit", StdioConfig::Inherit)] {
            let opts = LaunchOptions {
                exe: ps.into(),
                cmdline: Some(format!(
                    "\"{ps}\" -NoProfile -NonInteractive -EncodedCommand {encoded}"
                )),
                cwd: Some(r"C:\Windows\System32".into()),
                env: None,
                stdio,
                ..Default::default()
            };
            match launch_in_container_with_io(&sec, &opts) {
                Ok(child) => {
                    let code = child.wait(Some(std::time::Duration::from_secs(120)));
                    println!("{setup} ({variant}): powershell exit {code:?}");
                    match std::fs::read_to_string(out_dir.join(format!("{setup}.txt"))) {
                        Ok(t) => print!("{t}"),
                        Err(e) => println!("{setup}: no results file: {e}"),
                    }
                    return;
                }
                Err(e) => println!("{setup} ({variant}): launch failed: {e:?}"),
            }
        }
    };
    let elevated = std::process::Command::new("net")
        .arg("session")
        .output()
        .is_ok_and(|o| o.status.success());
    println!("AppContainer {sid}; this process elevated (admin): {elevated}");
    run("no capabilities", &[]);
    run(
        "internetClient + privateNetworkClientServer",
        &[
            KnownCapability::InternetClient,
            KnownCapability::PrivateNetworkClientServer,
        ],
    );
    // The loopback exemption: a machine-wide firewall setting (needs admin; CI runs as admin).
    let add = std::process::Command::new("CheckNetIsolation.exe")
        .args(["LoopbackExempt", "-a", &format!("-p={sid}")])
        .output();
    println!(
        "loopback exemption added: {:?}",
        add.as_ref().map(|o| o.status.code())
    );
    run("no capabilities + loopback exemption", &[]);
    let _ = std::process::Command::new("CheckNetIsolation.exe")
        .args(["LoopbackExempt", "-d", &format!("-p={sid}")])
        .output();
    let _ = profile.delete();
}

#[cfg(windows)]
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}
