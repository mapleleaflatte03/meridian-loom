fn main() {
    println!("{}", std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
}
