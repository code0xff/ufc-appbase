pub trait Enumeration {
    fn value(&self) -> String;
    fn find(name: &str) -> Option<Self> where Self: Sized;
    fn valid(value: &str) -> bool;
}

#[macro_export]
macro_rules! enumeration {
    ($enum:ident; $({$types:ident: $values:expr}),*) => {
        #[derive(Debug, PartialEq, Eq, Clone)]
        pub enum $enum {
            $($types,)*
        }

        impl Enumeration for $enum {
            fn value(&self) -> String {
                match self {
                    $($enum::$types => String::from($values),)*
                }
            }

            fn find(value: &str) -> Option<Self> {
                match value {
                     $($values => Some($enum::$types),)*
                    _ => None,
                }
            }

            fn valid(value: &str) -> bool {
                Self::find(value).is_some()
            }
        }
    }
}

#[cfg(test)]
mod enumeration_test {
    use crate::types::enumeration::Enumeration;
    use crate::enumeration;

    enumeration!(TestType; {A: "a"}, {B: "b"});

    #[test]
    fn enumeration_value_test() {
        let value = TestType::A.value();
        assert_eq!(value, "a");
    }

    #[test]
    fn enumeration_find_test() {
        let test_type = TestType::find("a").unwrap();
        assert_eq!(test_type, TestType::A);
    }

    #[test]
    fn enumeration_valid_test() {
        let valid = TestType::valid("a");
        assert!(valid);
    }

    #[test]
    fn enumeration_invalid_test() {
        let invalid = TestType::valid("c");
        assert!(!invalid);
    }
}
