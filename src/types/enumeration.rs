pub trait Enumeration {
    fn value(&self) -> String;
    fn find(name: &str) -> Option<Self> where Self: Sized;
}
