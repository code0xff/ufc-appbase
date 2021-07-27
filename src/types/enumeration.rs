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

            fn find(value: &str) -> Option<$enum> {
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
