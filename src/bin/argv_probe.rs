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
            GetConsoleMode, GetStdHandle, ReadConsoleW, SetConsoleMode, CONSOLE_MODE,
            ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT, STD_INPUT_HANDLE,
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
        let mut raw_mode = CONSOLE_MODE::default();
        // SAFETY: the same live console handle and a writable mode value are used synchronously.
        unsafe { GetConsoleMode(handle, &mut raw_mode).unwrap() };
        assert_eq!(
            raw_mode.0 & (ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT).0,
            0
        );
        println!(
            "CONTROL_READY api=ReadConsoleW original_mode={:#x} raw_mode={:#x}",
            mode.0, raw_mode.0
        );
        std::io::stdout().flush().unwrap();
        let mut units = [0u16; 2];
        let mut received = 0;
        while received < units.len() {
            let mut count = 0;
            // Read one character per call so Ctrl+Z is tested at the start of its own read.
            // ReadFile can convert leading Ctrl+Z to EOF even in raw mode on supported Windows hosts.
            // SAFETY: at least one writable UTF-16 unit and count are live for this synchronous read;
            // the console handle was checked above and no input-control structure is supplied.
            unsafe {
                ReadConsoleW(
                    handle,
                    units[received..].as_mut_ptr().cast(),
                    1,
                    &mut count,
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
        println!("CONTROL_READ count={received} utf16_units={units:?}");
        assert_eq!(received, 2);
        assert_eq!(units, [3, 26]);
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
