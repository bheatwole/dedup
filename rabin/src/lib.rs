pub mod chunker;
pub mod rolling_hash;

// This extension to SHA256 allows for using just part of the hash as an ID at the cost of increasing the chance of a
// collision.
pub trait ExtendableHashExt {
    fn hash_chunk_112(&mut self, chunk: &[u8]) -> [u8; 14];
    fn hash_chunk_128(&mut self, chunk: &[u8]) -> [u8; 16];
    fn hash_chunk_144(&mut self, chunk: &[u8]) -> [u8; 18];
    fn hash_chunk_160(&mut self, chunk: &[u8]) -> [u8; 20];
}

// Each of these functions generates the full SHA256 value and then just uses part of the result as the hash.
impl ExtendableHashExt for sha3::Sha3_256 {
    fn hash_chunk_112(&mut self, chunk: &[u8]) -> [u8; 14] {
        use sha3::Digest;

        self.input(chunk);
        let out = self.result_reset();

        let mut hash = [0u8; 14];
        hash.copy_from_slice(&out[0..14]);
        hash
    }

    fn hash_chunk_128(&mut self, chunk: &[u8]) -> [u8; 16] {
        use sha3::Digest;

        self.input(chunk);
        let out = self.result_reset();

        let mut hash = [0u8; 16];
        hash.copy_from_slice(&out[0..16]);
        hash
    }

    fn hash_chunk_144(&mut self, chunk: &[u8]) -> [u8; 18] {
        use sha3::Digest;

        self.input(chunk);
        let out = self.result_reset();

        let mut hash = [0u8; 18];
        hash.copy_from_slice(&out[0..18]);
        hash
    }

    fn hash_chunk_160(&mut self, chunk: &[u8]) -> [u8; 20] {
        use sha3::Digest;

        self.input(chunk);
        let out = self.result_reset();

        let mut hash = [0u8; 20];
        hash.copy_from_slice(&out[0..20]);
        hash
    }
}

// This is a helper function to make calculating a version 2 SHA256 hash a one-liner
pub fn hash_chunk_sha256(chunk: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.input(chunk);

    let mut output = [0u8; 32];
    output.copy_from_slice(hasher.result().as_slice());
    output
}

#[cfg(test)]
mod tests {

    use rand::distributions::Distribution;

    #[test]
    fn test_rolling_hash() {
        let source = [
            1u8, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8,
            9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0,
        ];

        // Initialize the rolling hash with the first 15 bytes of the source
        let mut rolling = crate::rolling_hash::RollingHash::new();
        rolling.hash_bytes(&source[0..15]);

        let mut hashed = crate::rolling_hash::RollingHash::new();

        // The result of adding one byte of the data at a time to the first hash should be the same as hashing just
        // those 16 bytes as one group.
        for i in 0..source.len() - 15 {
            // Add one more byte to the original hash
            rolling.hash_byte(source[i + 15]);

            // Hash the same bytes all at once.
            hashed.reset();
            hashed.hash_bytes(&source[i..i + 16]);

            // They gotta match!
            assert_eq!(rolling.hash(), hashed.hash());
        }
    }

    #[test]
    fn test_rolling_hash_random_distribution() {
        const TEST_BYTES: usize = 2 * 1024 * 1024;
        const SIX_PERCENT: u32 = TEST_BYTES as u32 / 4096;
        const LOWER_DISTRIBUTION: u32 = TEST_BYTES as u32 / 256 - SIX_PERCENT;
        const UPPER_DISTRIBUTION: u32 = TEST_BYTES as u32 / 256 + SIX_PERCENT;

        // We want to confirm that the rolling hash generates an even distribution of values, so we will hash some
        // random data and check the last byte of the hash to see if it is approximately even. That means we need 256
        // buckets to store the counts
        let mut buckets = [0u32; 256];

        // Create a random byte generator
        let mut rng = rand::thread_rng();
        let mut byte_iter = rand::distributions::Standard.sample_iter(&mut rng);

        // Hash 15 bytes to seed the rolling hash
        let mut rolling = crate::rolling_hash::RollingHash::new();
        for _ in 0..15 {
            rolling.hash_byte(byte_iter.next().unwrap());
        }

        // Hash an additional large number of bytes (2MiB), putting the bottom u8 of the hash into 256 buckets
        for _ in 0..TEST_BYTES {
            rolling.hash_byte(byte_iter.next().unwrap());

            let b = (rolling.hash() as u8) as usize;
            buckets[b] += 1;
        }

        // Distribution should be +/- SIX_PERCENT in each bucket
        for i in 0..256 {
            assert!(
                buckets[i] >= LOWER_DISTRIBUTION && buckets[i] <= UPPER_DISTRIBUTION,
                "bucket {} had {} but should have been between {} and {}",
                i,
                buckets[i],
                LOWER_DISTRIBUTION,
                UPPER_DISTRIBUTION
            );
        }
    }

    #[test]
    fn test_chunk_hash_random_distribution() {
        use crate::ExtendableHashExt;
        use sha3::{Digest, Sha3_256};

        const ITERATIONS: usize = 1024 * 16;
        const BYTES_PER_ITERATION: usize = 256;
        const OUTPUT_PER_ITERATION: usize = 18;
        const TEST_BYTES: usize = ITERATIONS * OUTPUT_PER_ITERATION;
        const TEN_PERCENT: u32 = TEST_BYTES as u32 / 256 / 10;
        const LOWER_DISTRIBUTION: u32 = TEST_BYTES as u32 / 256 - TEN_PERCENT;
        const UPPER_DISTRIBUTION: u32 = TEST_BYTES as u32 / 256 + TEN_PERCENT;

        // We want to confirm that the chunk hash generates an even distribution of values, so we will hash some
        // random data and check the last byte of the hash to see if it is approximately even. That means we need 256
        // buckets to store the counts
        let mut buckets = [0u32; 256];

        // Create a random byte generator
        let mut rng = rand::thread_rng();
        let mut byte_iter = rand::distributions::Standard.sample_iter(&mut rng);
        let mut source = [0u8; BYTES_PER_ITERATION];

        // Hash a large amount of random data, putting the bottom u8 of the hash into 256 buckets
        let mut hasher = Sha3_256::new();
        for _ in 0..ITERATIONS {
            for i in 0..BYTES_PER_ITERATION {
                source[i] = byte_iter.next().unwrap();
            }

            hasher.reset();
            let hash = hasher.hash_chunk_144(&source);
            for &b in hash.iter() {
                buckets[b as usize] += 1;
            }
        }

        // Distribution should be +/- TEN_PERCENT in each bucket. The percentage is larger for this test than the
        // rolling hash because we have to do fewer iterations or the test takes too long. With more iterations, the
        // distribution should be better.
        for i in 0..256 {
            assert!(
                buckets[i] >= LOWER_DISTRIBUTION && buckets[i] <= UPPER_DISTRIBUTION,
                "bucket {} had {} but should have been between {} and {}",
                i,
                buckets[i],
                LOWER_DISTRIBUTION,
                UPPER_DISTRIBUTION
            );
        }
    }
}
