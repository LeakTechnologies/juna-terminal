use std::io::Write;

fn emit_sustained(n: usize) {
    let mut w = std::io::BufWriter::new(std::io::stdout());
    for i in 0..n {
        let _ = writeln!(w, "LINE[{i:06}]=abcdefghijklmnopqrstuvwxyz0123456789");
    }
    let _ = w.flush();
    println!("SUSTAIN_END={n}");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    #[cfg(windows)]
    if args.first().map(String::as_str) == Some("--control-bytes") {
        use windows::Win32::System::Console::{
            GetConsoleMode, GetStdHandle, SetConsoleMode, CONSOLE_MODE, ENABLE_ECHO_INPUT,
            ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT, STD_INPUT_HANDLE,
        };
        let mut mode = CONSOLE_MODE::default();
        // SAFETY: the borrowed console input handle is live for this fixture; mode is writable.
        let handle = unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE).unwrap();
            GetConsoleMode(handle, &mut mode).unwrap();
            SetConsoleMode(
                handle,
                mode & !(ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT),
            )
            .unwrap();
            handle
        };
        println!("CONTROL_READY original_mode={:#x}", mode.0);
        std::io::stdout().flush().unwrap();
        let mut bytes = [0; 2];
        let mut received = 0;
        while received < bytes.len() {
            let mut count = 0;
            // SAFETY: a writable bounded buffer and count are live for this synchronous console read.
            unsafe {
                windows::Win32::Storage::FileSystem::ReadFile(
                    handle,
                    Some(&mut bytes[received..]),
                    Some(&mut count),
                    None,
                )
                .unwrap();
            }
            if count == 0 {
                break;
            }
            received += count as usize;
        }
        // SAFETY: restoring the input mode on the same live console handle before fixture exit.
        unsafe {
            SetConsoleMode(handle, mode).unwrap();
        }
        println!("CONTROL_READ count={received} bytes={bytes:?}");
        assert_eq!(received, 2);
        assert_eq!(bytes, [3, 26]);
        println!("CONTROL_BYTES_OK");
        return;
    }

    if args.first().map(String::as_str) == Some("--environment") {
        println!(
            "ENV={}",
            std::env::var("JUNA_ENV_PROBE").unwrap_or_default()
        );
        return;
    }

    if args.first().map(String::as_str) == Some("--spawn-grandchild") {
        let mut c = std::process::Command::new("cmd.exe");
        c.args(["/C", "ping", "-n", "60", "127.0.0.1", ">", "nul"]);
        match c.spawn() {
            Ok(p) => {
                println!("GRANDCHILD={}", p.id());
                let _ = std::io::stdout().flush();
            }
            Err(e) => println!("GRANDCHILD_ERR={e}"),
        }
        return;
    }

    if let Some(rest) = args.first() {
        if let Some(n) = rest.strip_prefix("--sustain=") {
            let n: usize = n.parse().unwrap_or(0);
            emit_sustained(n);
            return;
        }
    }

    println!("COUNT={}", args.len());
    for (i, a) in args.iter().enumerate() {
        println!("ARG[{i}]={}:{a}", a.len());
    }
    println!("END");
}
