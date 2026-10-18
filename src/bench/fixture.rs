use super::{publish, report::marker, wait_file};
use std::{
    io::Write,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use windows::Win32::{Foundation::HANDLE, System::Console::*};
struct Modes {
    input: HANDLE,
    output: HANDLE,
    old_input: CONSOLE_MODE,
    old_output: CONSOLE_MODE,
}
impl Drop for Modes {
    fn drop(&mut self) {
        // SAFETY: these borrowed handles belong to this fixture console and the saved modes came from them.
        unsafe {
            let _ = SetConsoleMode(self.input, self.old_input);
            let _ = SetConsoleMode(self.output, self.old_output);
        }
    }
}
fn paint(epoch: u32) -> Result<(), String> {
    let [r, g, b] = marker(epoch);
    let mut out = std::io::stdout().lock();
    write!(
        out,
        "\x1b[4;4H\x1b[48;2;{r};{g};{b}m{}\x1b[0m",
        " ".repeat(60)
    )
    .and_then(|_| out.flush())
    .map_err(|_| "Fixture output failed".into())
}
pub fn run(root: &Path, mode: &str, samples: u32, blocks: usize) -> Result<(), String> {
    if !matches!(mode, "idle" | "flood" | "ascii" | "unicode" | "colour")
        || !(1..=30).contains(&samples)
        || !(1..=128).contains(&blocks)
    {
        return Err("Fixture workload outside bounded limits".into());
    }
    let (input, output, mut old_input, mut old_output);
    // SAFETY: this fixture is launched only in its owned ConPTY, not a user's console. Output slots are live.
    unsafe {
        input = GetStdHandle(STD_INPUT_HANDLE).map_err(|_| "No fixture input")?;
        output = GetStdHandle(STD_OUTPUT_HANDLE).map_err(|_| "No fixture output")?;
        old_input = CONSOLE_MODE::default();
        old_output = CONSOLE_MODE::default();
        GetConsoleMode(input, &mut old_input).map_err(|_| "No input mode")?;
        GetConsoleMode(output, &mut old_output).map_err(|_| "No output mode")?;
    }
    let _modes = Modes {
        input,
        output,
        old_input,
        old_output,
    };
    // SAFETY: live owned-console handles; raw input preserves ordinary keys and restores modes on exit.
    unsafe {
        SetConsoleMode(input, ENABLE_EXTENDED_FLAGS | ENABLE_VIRTUAL_TERMINAL_INPUT)
            .map_err(|_| "Raw input unavailable")?;
        SetConsoleMode(
            output,
            old_output | ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING,
        )
        .map_err(|_| "VT output unavailable")?;
    }
    print!("\x1b[2J\x1b[?25l");
    paint(0)?;
    publish(&root.join("ready"), b"ready")?;
    wait_file(&root.join("go"), Duration::from_secs(45))?;
    let stop = Arc::new(AtomicBool::new(false));
    let flood = if mode == "flood" {
        let stop = stop.clone();
        Some(std::thread::spawn(move || -> Result<(), String> {
            // Repaint a fixed row, preserving the independently coloured response row.
            let block = format!("\x1b[8;1H{}", "load ".repeat(12)).repeat(256);
            while !stop.load(Ordering::Relaxed) {
                let mut output = std::io::stdout().lock();
                output
                    .write_all(block.as_bytes())
                    .and_then(|_| output.flush())
                    .map_err(|_| "Flood output failed")?;
                drop(output);
                std::thread::yield_now();
            }
            Ok(())
        }))
    } else {
        None
    };
    if matches!(mode, "ascii" | "unicode" | "colour") {
        let line = match mode {
            "unicode" => "Unicode: e\u{301} 中文 日本語 🦊 terminal\r\n".to_owned(),
            "colour" => "\x1b[38;2;90;170;230mcolour\x1b[0m output 0123456789\r\n".to_owned(),
            _ => format!("{}\r\n", "abcdefgh01234567".repeat(4)),
        };
        let block = line.repeat(128);
        let mut out = std::io::stdout().lock();
        for _ in 0..blocks {
            out.write_all(block.as_bytes())
                .map_err(|_| "Output workload failed")?;
        }
        out.flush().map_err(|_| "Output flush failed")?;
        drop(out);
        paint(1)?;
        publish(
            &root.join("bytes"),
            (block.len() * blocks).to_string().as_bytes(),
        )?;
    } else {
        for epoch in 1..=samples {
            let (mut character, mut count) = (0u16, 0u32);
            // SAFETY: raw console handle is live; a single writable UTF-16 scalar slot is supplied.
            unsafe {
                ReadConsoleW(
                    input,
                    &mut character as *mut _ as *mut _,
                    1,
                    &mut count,
                    None,
                )
                .map_err(|_| "Input read failed")?;
            }
            let received = super::clock::ticks()?;
            if count != 1 || !matches!(character, 65 | 97) {
                return Err("Targeted input did not deliver expected key".into());
            }
            paint(epoch)?;
            publish(
                &root.join(format!("received-{epoch}")),
                received.to_string().as_bytes(),
            )?;
        }
    }
    stop.store(true, Ordering::Relaxed);
    if let Some(thread) = flood {
        thread.join().map_err(|_| "Flood thread panicked")??;
    }
    publish(&root.join("complete"), b"complete")?;
    wait_file(&root.join("stop"), Duration::from_secs(45))?;
    Ok(())
}
