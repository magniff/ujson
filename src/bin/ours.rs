fn main() {
    let input = std::fs::read_to_string("data.json").unwrap();
    let how_many = 100;
    let mut cumulative = 0.0;
    for _ in 0..how_many {
        let t0 = std::time::Instant::now();
        let _result = ujson::from_str(input.as_str()).unwrap();
        let t1 = std::time::Instant::now();
        cumulative += (t1 - t0).as_secs_f64();
    }
    println!("serde: {:?}", cumulative / how_many as f64)
}
