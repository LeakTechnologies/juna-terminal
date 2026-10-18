#[cfg(windows)]
#[path = "../bench/mod.rs"]
mod bench;
fn main() {
    #[cfg(windows)]
    if let Err(error) = bench::run() {
        eprintln!("juna-bench: {error}");
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    {
        eprintln!("juna-bench currently requires Windows");
        std::process::exit(2);
    }
}
