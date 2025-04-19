fn main() {
    let input = std::fs::read_to_string("data.json").unwrap();
    println!(
        "is ok: {}",
        serde_json::from_str::<serde_json::Value>(input.as_str()).is_ok()
    )
}
