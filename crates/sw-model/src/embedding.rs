//! The hashed lexical task embedding (design §2.3, D6). The client that writes a Task computes
//! it; the server recomputes it from the payload and refuses a mismatch, so it must be exactly
//! reproducible on every target: integer accumulation, one `f32` normalisation at the end.

use sw_crypto::sha3_256;
use unicode_normalization::UnicodeNormalization;

pub const DIMS: usize = 256;

/// The fixed hash key (versioned so a change of scheme is a new key, never a silent drift).
const KEY: &[u8] = b"sw/embed/v1";

/// The fixed stop list. Changing it changes every embedding: bump [`KEY`] with it.
const STOP: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "in", "into", "is", "it", "of",
    "on", "or", "that", "the", "this", "to", "with",
];

/// NFKC, lowercase, split on non-alphanumerics, drop stop words.
pub fn tokens(text: &str) -> Vec<String> {
    let normalized: String = text.nfkc().collect::<String>().to_lowercase();
    normalized
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty() && !STOP.contains(t))
        .map(str::to_owned)
        .collect()
}

fn keyed_hash(feature: &str) -> u64 {
    let mut input = KEY.to_vec();
    input.push(0);
    input.extend_from_slice(feature.as_bytes());
    let digest = sha3_256(&input);
    let mut word = [0u8; 8];
    word.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(word)
}

/// Tokens and adjacent bigrams hashed into 256 signed buckets: the exact integer form a
/// `TASK_WRITE` carries and the server recomputes.
pub fn buckets(title: &str, description: &str) -> [i32; DIMS] {
    let toks = tokens(&format!("{title} {description}"));
    let bigrams = toks.windows(2).map(|w| format!("{} {}", w[0], w[1]));
    let mut buckets = [0i32; DIMS];
    for feature in toks.iter().cloned().chain(bigrams) {
        let h = keyed_hash(&feature);
        let index = (h % DIMS as u64) as usize;
        buckets[index] += if h >> 63 == 1 { -1 } else { 1 };
    }
    buckets
}

/// [`buckets`], L2-normalised. An input with no tokens yields the zero vector.
pub fn embed(title: &str, description: &str) -> [f32; DIMS] {
    let buckets = buckets(title, description);
    let norm_sq: i64 = buckets.iter().map(|&b| i64::from(b) * i64::from(b)).sum();
    let mut out = [0f32; DIMS];
    if norm_sq == 0 {
        return out;
    }
    let norm = (norm_sq as f64).sqrt();
    for (o, &b) in out.iter_mut().zip(buckets.iter()) {
        *o = (f64::from(b) / norm) as f32;
    }
    out
}

pub fn cosine(a: &[f32; DIMS], b: &[f32; DIMS]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
