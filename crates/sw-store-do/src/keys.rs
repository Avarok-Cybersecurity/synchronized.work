//! Storage keys. Metadata and pages share the object's one key space, so each has its own
//! prefix and neither can collide with the other.

pub const META_PREFIX: &str = "m:";
pub const PAGE_PREFIX: &str = "p:";

pub fn meta_key(key: &str) -> String {
    format!("{META_PREFIX}{key}")
}

/// `p:` and 16 lowercase hex digits: lossless for every `u64`, and ordered like the ids.
pub fn page_key(page_id: u64) -> String {
    format!("{PAGE_PREFIX}{page_id:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_ids_past_two_to_the_53_stay_distinct() {
        let a = (1u64 << 53) + 1;
        let b = 1u64 << 53;
        assert_ne!(page_key(a), page_key(b));
        assert_eq!(page_key(u64::MAX), "p:ffffffffffffffff");
        assert_eq!(page_key(0), "p:0000000000000000");
    }

    #[test]
    fn a_meta_key_never_reads_as_a_page_key() {
        assert!(!meta_key("p:0000000000000000").starts_with(PAGE_PREFIX));
    }
}
