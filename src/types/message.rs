#[macro_export]
macro_rules! message {
    (($message:ident; $({$message_names:ident: $message_types:ident}),*); ($method:ident; $({$method_types:ident: $method_values:expr}),*)) => {
        enumeration!($method; $({$method_types: $method_values}),*);

        #[derive(Serialize, Deserialize)]
        pub struct $message {
            method: String,
            $($message_names: $message_types,)*
        }

        impl $message {
           pub fn new(method: $method, $($message_names: $message_types,)*) -> Value {
                json!(Self {
                    method: method.value(),
                    $($message_names,)*
                })
            }
        }
    };
    ($message:ident; $({$message_names:ident: $message_types:ident}),*) => {
        #[derive(Serialize, Deserialize)]
        pub struct $message {
            $($message_names: $message_types,)*
        }

        impl $message {
            #[allow(dead_code)]
            pub fn new($($message_names: $message_types,)*) -> Value {
                json!(Self {
                    $($message_names,)*
                })
            }
        }
    };
}
