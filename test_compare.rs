fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= std::hint::black_box(x ^ y);
    }
    result == 0
}

fn main() {
    assert!(constant_time_eq("hello", "hello"));
    assert!(!constant_time_eq("hello", "world"));
    assert!(!constant_time_eq("hello", "hell"));
}
