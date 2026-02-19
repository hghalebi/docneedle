const DEFAULT: usize = 128;

pub const DEFAULT_EMBEDDING_DIMENSIONS: usize = DEFAULT;

pub trait Embedder {
    fn dimensions(&self) -> usize;
    fn embed(&self, text: &str) -> Vec<f32>;
}

#[derive(Debug, Clone, Copy)]
pub struct CharacterNgramEmbedder {
    pub dimensions: usize,
}

impl Default for CharacterNgramEmbedder {
    fn default() -> Self {
        Self {
            dimensions: DEFAULT_EMBEDDING_DIMENSIONS,
        }
    }
}

impl Embedder for CharacterNgramEmbedder {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0f32; self.dimensions.max(1)];
        let lowered = text.to_lowercase();
        let chars: Vec<char> = lowered.chars().collect();

        if chars.is_empty() {
            return vector;
        }

        for window in chars.windows(3) {
            let token = window.iter().collect::<String>();
            let mut hash = 1469598103934665603u64;
            for byte in token.bytes() {
                hash ^= byte as u64;
                hash = hash.wrapping_mul(1099511628211);
            }
            let bucket = (hash % vector.len() as u64) as usize;
            vector[bucket] += 1.0;
        }

        let magnitude = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for value in &mut vector {
                *value /= magnitude;
            }
        }

        vector
    }
}

#[cfg(test)]
mod tests {
    use super::{CharacterNgramEmbedder, Embedder};

    #[test]
    fn embedder_is_deterministic() {
        let embedder = CharacterNgramEmbedder::default();
        let first = embedder.embed("Hydraulic pressure and flow");
        let second = embedder.embed("Hydraulic pressure and flow");
        assert_eq!(first, second);
    }

    #[test]
    fn embedder_outputs_expected_length() {
        let embedder = CharacterNgramEmbedder { dimensions: 32 };
        let vector = embedder.embed("abc");
        assert_eq!(vector.len(), 32);
    }
}
