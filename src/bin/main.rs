fn main() {
    std::process::Command::new("make")
        .arg("test")
        .status()
        .unwrap();
}
