#[macro_export]
macro_rules! verify_str {
    ($params:ident; $($name:expr),*) => {
        $(if $params.get($name).is_none() || !$params.get($name).unwrap().is_string() { return Err(format!("invalid {} value", $name)); })*
    };
}

#[macro_export]
macro_rules! verify_num {
    ($params:ident; $($name:expr),*) => {
        $(if $params.get($name).is_none() || !$params.get($name).unwrap().is_number() { return Err(format!("invalid {} value", $name)); })*
    };
}

#[macro_export]
macro_rules! verify_arr {
    ($params:ident; $($name:expr),*) => {
        $(if $params.get($name).is_none() || !$params.get($name).unwrap().is_array() { return Err(format!("invalid {} value", $name)); })*
    };
}
