fn main() {
    let input = std::fs::read_to_string("data.json").unwrap();
    println!("is ok: {}", ujson::from_str(input.as_str()).is_ok())
}
