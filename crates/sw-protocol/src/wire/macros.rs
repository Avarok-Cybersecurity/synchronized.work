/// A struct carried as a CBOR map with the given small-integer keys (envelopes).
macro_rules! wire_map {
    ($(#[$m:meta])* pub struct $name:ident { $($(#[$fm:meta])* $k:literal => $f:ident : $t:ty),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name { $($(#[$fm])* pub $f: $t),* }
        impl $crate::wire::Wire for $name {
            fn to_value(&self) -> ciborium::value::Value {
                let mut out = $crate::wire::MapOut::default();
                $( out.field($k as u64, &self.$f); )*
                out.build()
            }
            fn from_value(value: ciborium::value::Value) -> Result<Self, $crate::WireError> {
                let mut map = $crate::wire::MapIn::new(value)?;
                let out = Self { $($f: map.field($k as u64)?),* };
                map.finish()?;
                Ok(out)
            }
        }
    };
}
pub(crate) use wire_map;

/// A struct carried as a CBOR map keyed by its field names (payloads). With a trailing
/// `check = f;`, a decoded value is also refused unless `f(&value)` accepts it (the shape
/// rules a type cannot express, e.g. "`reason` iff `refused`").
macro_rules! wire_text {
    ($(#[$m:meta])* pub struct $name:ident { $($(#[$fm:meta])* $f:ident : $t:ty),* $(,)? }) => {
        $crate::wire::wire_text! {
            $(#[$m])* pub struct $name { $($(#[$fm])* $f: $t),* }
            check = |_: &$name| Ok(());
        }
    };
    ($(#[$m:meta])* pub struct $name:ident { $($(#[$fm:meta])* $f:ident : $t:ty),* $(,)? }
     check = $check:expr;) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name { $($(#[$fm])* pub $f: $t),* }
        impl $crate::wire::Wire for $name {
            fn to_value(&self) -> ciborium::value::Value {
                let mut out = $crate::wire::MapOut::default();
                $( out.field(stringify!($f), &self.$f); )*
                out.build()
            }
            fn from_value(value: ciborium::value::Value) -> Result<Self, $crate::WireError> {
                let mut map = $crate::wire::MapIn::new(value)?;
                let out = Self { $($f: map.field(stringify!($f))?),* };
                map.finish()?;
                let check: fn(&Self) -> Result<(), $crate::WireError> = $check;
                check(&out)?;
                Ok(out)
            }
        }
    };
}
pub(crate) use wire_text;

/// A fieldless enum carried as one of a fixed set of strings.
macro_rules! wire_str {
    ($t:ty { $($text:literal => $variant:path),* $(,)? }) => {
        impl $crate::wire::Wire for $t {
            fn to_value(&self) -> ciborium::value::Value {
                let text: &str = match self { $($variant => $text),* };
                ciborium::value::Value::Text(text.to_owned())
            }
            fn from_value(value: ciborium::value::Value) -> Result<Self, $crate::WireError> {
                match <String as $crate::wire::Wire>::from_value(value)?.as_str() {
                    $($text => Ok($variant),)*
                    _ => Err($crate::WireError::UnknownCode(stringify!($t))),
                }
            }
        }
    };
}
pub(crate) use wire_str;
