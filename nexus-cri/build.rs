fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "linux" {
        println!("cargo:warning=Nexus-VMM requires a Linux host with KVM. Go buy a real operating system.");
        std::process::exit(1);
    }
}
