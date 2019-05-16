fn main() {
    let mut buffer = String::new();
    loop {
        std::io::stdin().read_line(&mut buffer).unwrap();

        println!("{:#?}", parsing::parse_markdown(buffer.trim()));
        buffer.clear();
    }
}
