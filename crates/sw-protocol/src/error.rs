use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    /// Not CBOR, or not one complete item.
    Malformed,
    /// Valid CBOR that is not in core deterministic form.
    NonCanonical,
    /// A float, tag, `null` or `undefined`.
    ForbiddenType,
    DuplicateKey,
    UnknownKey(u64),
    MissingKey(u64),
    Type(&'static str),
    Range(&'static str),
    Length(&'static str),
    UnknownCode(&'static str),
    Shape(&'static str),
    Encode,
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for WireError {}
