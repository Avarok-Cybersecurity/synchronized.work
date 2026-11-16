use core::fmt;

/// A permanent Citadel client id. It names which account is connected, never who authorised
/// anything (design §5.2).
pub type Cid = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdError {
    /// An `id_fp` must be exactly 16 characters of lowercase RFC 4648 base32 (`a-z`, `2-7`).
    BadIdFp,
    /// A 128-bit id must be exactly 32 lowercase hex characters.
    BadHex,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadIdFp => f.write_str("id_fp must be 16 lowercase base32 characters"),
            Self::BadHex => f.write_str("id must be 32 lowercase hex characters"),
        }
    }
}

impl std::error::Error for IdError {}

/// The Person id: the fingerprint of a portable Citadel identity key (identity doc rev b §3.2,
/// master plan §5.1 `Identity::id_fp`). This crate only validates its shape; computing it from
/// a public key is the identity crate's job, reached through `sw_protocol::IdentityPort`.
/// Lowercase because it is the first label of every Endpoint username
/// (`<id_fp>.<slug>@synchronized.work`) and usernames are lowercase.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IdFp(String);

impl IdFp {
    pub const LEN: usize = 16;

    pub fn parse(text: &str) -> Result<Self, IdError> {
        let ok = text.len() == Self::LEN
            && text
                .bytes()
                .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b));
        if ok {
            Ok(Self(text.to_owned()))
        } else {
            Err(IdError::BadIdFp)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IdFp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A 128-bit id for Tasks, Agents and signals, assigned client-side from injected entropy
/// and accepted by the server only if unused (design §2.1).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id128(pub [u8; 16]);

impl Id128 {
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn from_hex(text: &str) -> Result<Self, IdError> {
        let bytes = text.as_bytes();
        if bytes.len() != 32 {
            return Err(IdError::BadHex);
        }
        let mut out = [0u8; 16];
        for (i, pair) in bytes.chunks(2).enumerate() {
            out[i] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
        }
        Ok(Self(out))
    }
}

fn nibble(c: u8) -> Result<u8, IdError> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        _ => Err(IdError::BadHex),
    }
}

impl fmt::Debug for Id128 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id128({})", self.to_hex())
    }
}

impl fmt::Display for Id128 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}
