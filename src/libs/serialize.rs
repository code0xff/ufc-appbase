pub fn deserialize(val: &[u8]) -> String {
    String::from_utf8(val.to_vec()).unwrap()
}