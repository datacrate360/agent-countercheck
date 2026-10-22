//! Run the same PowerShell probe in an AppContainer three ways and print what worked.
//! Output lines: `<setup>: <probe> = ok|FAILED (<why>)`.

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only");
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Start `cmdline` in the AppContainer `ac` with the given capability SIDs; its exit code.
#[cfg(windows)]
fn launch(ac: windows_sys::Win32::Security::PSID, caps: &[&str], cmdline: &str) -> Result<u32, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
    use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
    use windows_sys::Win32::Security::{PSID, SECURITY_CAPABILITIES, SID_AND_ATTRIBUTES};
    use windows_sys::Win32::System::Threading::*;
    const SE_GROUP_ENABLED: u32 = 4;
    unsafe {
        let mut cap_sids: Vec<SID_AND_ATTRIBUTES> = Vec::new();
        for c in caps {
            let w = wide(c);
            let mut sid: PSID = std::ptr::null_mut();
            if ConvertStringSidToSidW(w.as_ptr(), &mut sid) == 0 {
                return Err(format!("capability SID {c}: error {}", GetLastError()));
            }
            cap_sids.push(SID_AND_ATTRIBUTES { Sid: sid, Attributes: SE_GROUP_ENABLED });
        }
        let sc = SECURITY_CAPABILITIES {
            AppContainerSid: ac,
            Capabilities: if cap_sids.is_empty() { std::ptr::null_mut() } else { cap_sids.as_mut_ptr() },
            CapabilityCount: cap_sids.len() as u32,
            Reserved: 0,
        };
        let mut size = 0usize;
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size);
        let mut buf = vec![0u64; size.div_ceil(8)];
        let list = buf.as_mut_ptr().cast();
        if InitializeProcThreadAttributeList(list, 1, 0, &mut size) == 0 {
            return Err(format!("InitializeProcThreadAttributeList: {}", GetLastError()));
        }
        if UpdateProcThreadAttribute(
            list,
            0,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
            std::ptr::addr_of!(sc).cast(),
            std::mem::size_of::<SECURITY_CAPABILITIES>(),
            std::ptr::null_mut(),
            std::ptr::null(),
        ) == 0
        {
            return Err(format!("UpdateProcThreadAttribute: {}", GetLastError()));
        }
        let mut si = STARTUPINFOEXW::default();
        si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        si.lpAttributeList = list;
        let mut cmd = wide(cmdline);
        let cwd = wide(r"C:\Windows\System32");
        let mut pi = PROCESS_INFORMATION::default();
        let ok = CreateProcessW(
            std::ptr::null(),
            cmd.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW,
            std::ptr::null(),
            cwd.as_ptr(),
            std::ptr::addr_of!(si.StartupInfo),
            &mut pi,
        );
        DeleteProcThreadAttributeList(list);
        if ok == 0 {
            return Err(format!("CreateProcessW: error {}", GetLastError()));
        }
        WaitForSingleObject(pi.hProcess, 180_000);
        let mut code = 0u32;
        windows_sys::Win32::System::Threading::GetExitCodeProcess(pi.hProcess, &mut code);
        CloseHandle(pi.hThread);
        CloseHandle(pi.hProcess);
        Ok(code)
    }
}

#[cfg(windows)]
fn main() {
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::Isolation::{CreateAppContainerProfile, DeleteAppContainerProfile, DeriveAppContainerSidFromAppContainerName};
    use windows_sys::Win32::Security::PSID;

    let python = std::env::var("PROBE_PYTHON").unwrap_or_default();
    // Results go to a file in a folder granted to the container.
    let out_dir = std::env::var_os("PROBE_OUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("acprobe-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&out_dir);
    let make_script = |setup: &str| {
        format!(
            r#"
$ErrorActionPreference = 'Stop'
$out = '{out}\{file}.txt'
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
Probe 'write the user temp folder' {{ Set-Content -Path (Join-Path $env:TEMP 'appcontainer-probe.txt') -Value x }}
"#,
            out = out_dir.display(),
            file = setup.replace(' ', "_").replace('+', "and"),
        )
    };
    let encode = |script: String| {
        base64(&script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect::<Vec<u8>>())
    };
    let ps = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
    let name = format!("countercheck.probe.{}", std::process::id());
    let wname = wide(&name);
    let mut ac: PSID = std::ptr::null_mut();
    // SAFETY (evidence only): valid NUL-terminated strings and out pointer.
    let hr = unsafe {
        CreateAppContainerProfile(wname.as_ptr(), wname.as_ptr(), wname.as_ptr(), std::ptr::null(), 0, &mut ac)
    };
    if hr < 0 {
        let hr2 = unsafe { DeriveAppContainerSidFromAppContainerName(wname.as_ptr(), &mut ac) };
        if hr2 < 0 {
            println!("could not create an AppContainer profile: 0x{hr:08x}");
            return;
        }
    }
    let mut text: *mut u16 = std::ptr::null_mut();
    let sid = unsafe {
        ConvertSidToStringSidW(ac, &mut text);
        let mut n = 0;
        while *text.add(n) != 0 {
            n += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(text, n))
    };
    let grant = std::process::Command::new("icacls")
        .arg(&out_dir)
        .args(["/grant", &format!("*{sid}:(OI)(CI)F")])
        .output();
    println!("granted the results folder: {:?}", grant.map(|o| o.status.code()));
    let elevated = std::process::Command::new("net")
        .arg("session")
        .output()
        .is_ok_and(|o| o.status.success());
    println!("AppContainer {sid}; this process elevated (admin): {elevated}");
    let run = |setup: &str, caps: &[&str]| {
        let line = format!(
            "\"{ps}\" -NoProfile -NonInteractive -EncodedCommand {}",
            encode(make_script(setup))
        );
        match launch(ac, caps, &line) {
            Ok(code) => println!("{setup}: powershell exit {code}"),
            Err(e) => println!("{setup}: launch failed: {e}"),
        }
        let file = out_dir.join(format!("{}.txt", setup.replace(' ', "_").replace('+', "and")));
        match std::fs::read_to_string(&file) {
            Ok(t) => print!("{t}"),
            Err(e) => println!("{setup}: no results file: {e}"),
        }
    };
    run("no capabilities", &[]);
    // internetClient (S-1-15-3-1) + privateNetworkClientServer (S-1-15-3-3).
    run(
        "internetClient + privateNetworkClientServer",
        &["S-1-15-3-1", "S-1-15-3-3"],
    );
    // The loopback exemption: a machine-wide firewall setting (needs admin; CI runs as admin).
    let add = std::process::Command::new("CheckNetIsolation.exe")
        .args(["LoopbackExempt", "-a", &format!("-p={sid}")])
        .output();
    println!(
        "loopback exemption added: {:?}",
        add.as_ref().map(|o| (o.status.code(), String::from_utf8_lossy(&o.stdout).trim().to_string()))
    );
    let listed = std::process::Command::new("CheckNetIsolation.exe")
        .args(["LoopbackExempt", "-s"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains(&sid.to_lowercase()));
    println!("loopback exemption listed afterwards: {listed:?}");
    run("no capabilities + loopback exemption", &[]);
    let _ = std::process::Command::new("CheckNetIsolation.exe")
        .args(["LoopbackExempt", "-d", &format!("-p={sid}")])
        .output();
    unsafe { DeleteAppContainerProfile(wname.as_ptr()) };
}

#[cfg(windows)]
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
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
