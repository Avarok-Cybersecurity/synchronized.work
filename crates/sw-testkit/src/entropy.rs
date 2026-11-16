use sw_crypto::{Entropy, sha3_256};

/// A reproducible byte stream: block `i` is `SHA3-256(label ‖ u64be(i))`.
#[derive(Debug, Clone)]
pub struct LabelledEntropy {
    label: Vec<u8>,
    block: u64,
}

impl LabelledEntropy {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.as_bytes().to_vec(),
            block: 0,
        }
    }

    pub fn bytes<const N: usize>(&mut self) -> [u8; N] {
        let mut out = [0u8; N];
        self.fill(&mut out);
        out
    }
}

impl Entropy for LabelledEntropy {
    fn fill(&mut self, out: &mut [u8]) {
        for chunk in out.chunks_mut(32) {
            let mut input = self.label.clone();
            input.extend_from_slice(&self.block.to_be_bytes());
            self.block += 1;
            let block = sha3_256(&input);
            chunk.copy_from_slice(&block[..chunk.len()]);
        }
    }
}

/// Exactly the given bytes, in order; running out is a broken known-answer setup.
#[derive(Debug, Clone)]
pub struct ReplayedEntropy {
    bytes: Vec<u8>,
    at: usize,
}

impl ReplayedEntropy {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, at: 0 }
    }

    pub fn exhausted(&self) -> bool {
        self.at == self.bytes.len()
    }
}

impl Entropy for ReplayedEntropy {
    fn fill(&mut self, out: &mut [u8]) {
        let end = self.at + out.len();
        out.copy_from_slice(
            self.bytes
                .get(self.at..end)
                .expect("known-answer entropy ran out"),
        );
        self.at = end;
    }
}
