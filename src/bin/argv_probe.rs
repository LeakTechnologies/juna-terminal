use std::io::Write;

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

    println!("COUNT={}", args.len());
    for (i, a) in args.iter().enumerate() {
        println!("ARG[{i}]={}:{a}", a.len());
    }
    println!("END");
}