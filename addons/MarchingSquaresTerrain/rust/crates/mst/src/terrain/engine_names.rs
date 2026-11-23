//! Names the engine's undo history needs as strings, spelled through the Rust item they name
//! so that renaming the method or field fails the build instead of a later undo.
macro_rules! method_name {
    ($class:ty, $method:ident) => {{
        let _ = <$class>::$method;
        stringify!($method)
    }};
}
macro_rules! property_name {
    ($class:ty, $field:ident) => {{
        let _ = |t: &$class| {
            let _ = &t.$field;
        };
        stringify!($field)
    }};
}
