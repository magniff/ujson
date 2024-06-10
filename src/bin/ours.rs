fn main() {
    let input = std::fs::read_to_string("data.json").unwrap();
    let t0 = std::time::Instant::now();
    ujson::from_str(input.as_str()).unwrap();
    let t1 = std::time::Instant::now();
    println!("ours: {:?}", t1 - t0);
}
