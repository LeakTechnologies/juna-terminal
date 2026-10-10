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