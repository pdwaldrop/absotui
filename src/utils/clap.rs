use clap::{Arg, Command};

pub fn clap() {
    let matches = Command::new("absotui")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::new("update")
                .long("update")
                .help("Run update script via curl")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("uninstall")
                .long("uninstall")
                .help("Run uninstall script via curl")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    if matches.get_flag("uninstall") {
        run_hello_absotui("uninstall");
    }
    if matches.get_flag("update") {
        run_hello_absotui("update");
    }
}

// Fetches hello_absotui.sh fresh from `stable` and runs it; the script checks its own checksum against
// the latest release, so nothing here has to stay in sync with it.
fn run_hello_absotui(command: &str) {
    let script = format!(
        r#"tmpfile=$(mktemp) && curl -LsSf https://raw.githubusercontent.com/pdwaldrop/absotui/stable/hello_absotui.sh -o "$tmpfile" && bash "$tmpfile" {command} && rm -f "$tmpfile""#
    );
    let status = std::process::Command::new("bash")
        .arg("-c")
        .arg(script)
        .status()
        .expect("failed to run hello_absotui.sh");
    std::process::exit(status.code().unwrap_or(1));
}

