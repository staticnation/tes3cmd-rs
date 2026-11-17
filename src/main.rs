fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = std::io::stdout().lock();
    let mut err = std::io::stderr().lock();
    let code = tes3cmd::cli::run(&args, &mut out, &mut err);
    std::process::exit(code);
}
