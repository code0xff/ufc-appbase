pub fn deserialize(val: &[u8]) -> String {
    String::from_utf8(val.to_vec()).unwrap()
}

#[cfg(test)]
mod serialize_test {
    use crate::libs::serialize::deserialize;

    #[test]
    fn deserialize_test() {
        let serialized = "test".as_bytes();
        let deserialized = deserialize(serialized);
        assert_eq!(deserialized.as_str(), "test");
    }
}
