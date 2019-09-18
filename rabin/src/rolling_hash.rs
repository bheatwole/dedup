// This file includes all the necessary statics and consts to run the rolling hash
include!(concat!(env!("OUT_DIR"), "/static_rolling_hash_autogen.rs"));


// A rolling hash is a hash function that operates over a windows of a certain number of bytes. The rolling nature comes
// from the property that the hash of bytes [1..17] is the same as first hashing [0..16] and then pushing one more byte
// into the hash. Thus the algorithm produces a strong (but not cryptographic) hash of a small number of bytes. As a
// strong hash, it has the properties of near-random output (hashing any particular set of bytes will produce what looks
// like a random number), but is repeatable (hashing two identical set of bytes will produce identical output).


// The RollingHash struct stores a precomputed table of what happens mathematically to the hash when a new byte is
// pushed into the rolling window and the old byte is popped off. The push and pops tables could be moved to static
// constants if the construction time of RollingHash becomes an issue.
pub struct RollingHash {
    hash: u64,
    queue: [u8; WINDOW_SIZE],
    next: usize,
}

impl RollingHash {
    pub fn new() -> RollingHash {

        RollingHash {
            hash: 0,
            queue: [0; WINDOW_SIZE],
            next: 0,
        }
    }

    // Returns the current hash value
    pub fn hash(&self) -> u64 {
        self.hash
    }

    // Resets the hash to it's default state
    pub fn reset(&mut self) {
        self.hash = 0;
        self.queue = [0; WINDOW_SIZE];
    }

    // Adds a single byte to the hash.
    pub fn hash_byte(&mut self, b: u8) {
        // Concat the new byte onto the hash
        let high_byte = (self.hash >> 56) as usize;
        self.hash = ((self.hash << 8) | (b as u64)) ^ ROLLING_HASH_PUSH_TABLE[high_byte];

        // Remove the old byte
        let old_byte = self.queue[self.next] as usize;
        self.hash ^= ROLLING_HASH_POP_TABLE[old_byte];

        // Update the circular byte queue. The next position will range from 0-15 and then wrap around
        self.queue[self.next] = b;
        self.next = (self.next + 1) & WINDOW_MASK; // same as next % WINDOW_SIZE. this provides wrap-around using simple
                                                   // instructions, but requires that WINDOW_SIZE is a power of two.
    }

    // Hashes the specified bytes. If there are a large number of bytes, hash_bytes will skip to the last window to save
    // processing time.
    pub fn hash_bytes(&mut self, mut bytes: &[u8]) {
        // If the additional bytes are longer than twice the window, its faster just to reset the hash and hash the 16
        // bytes at the end
        if bytes.len() > TWICE_WINDOW_SIZE {
            self.reset();
            bytes = &bytes[bytes.len() - WINDOW_SIZE..];
        }

        for &b in bytes {
            self.hash_byte(b);
        }
    }
}
