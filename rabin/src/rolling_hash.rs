// The rolling hash implementation in the file uses the Rabin fingerprinting method of irreducible polynomials over a
// finited field. The Rabin fingerprint is NOT considered to be cryptographically secure, but it is a fast algorithm
// that can be used to cut a file into chunks without leaking information on the contents of the file.

// The default irreducible polynomial is x^64 + x^4 + x^3 + x + 1. This would normally require 65 bits to store, but in
// this implementation we assume that bit 64 (0 indexed) is set and do not store it.  0x1B == 00011011 which is read as:
//    bit 0: constant 1    (always set)
//    bit 1: x             (set)
//    bit 2: x^2           (not set)
//    bit 3: x^3           (set)
//    bit 4: x^4           (set)
//    bit 5: x^5           (not set)
//       etc...
// The value for this polynomial is taken from "Table of Low-Weight Binary Irreducible Polynomials" published by Hewlett
// Packard at: https://www.hpl.hp.com/techreports/98/HPL-98-135.pdf
const DEFAULT_IRREDUCIBLE_POLYNOMIAL_64: u64 = 0x1B;

// This is the basis of the Rabin fingerprint, where overflow when shifting results in dividing by a irreducible
// polynomial and using the remainder.
fn shift_left_n_bits_with_mod_64(mut number: u64, n: u8) -> u64 {
    for _ in 0..n {
        // We will need to mod the irreducible poly if shifting left would leave bit 65 set. Check bit 64 now to see if
        // we need to do that
        let needs_mod = number & 0x8000000000000000 == 0x8000000000000000;

        // Do the shift
        number <<= 1;

        // In polynomial math of this nature, XOR is equivalent to mod
        if needs_mod {
            number ^= DEFAULT_IRREDUCIBLE_POLYNOMIAL_64;
        }
    }

    number
}

// A rolling hash is a hash function that operates over a windows of a certain number of bytes. The rolling nature comes
// from the property that the hash of bytes [1..17] is the same as first hashing [0..16] and then pushing one more byte
// into the hash. Thus the algorithm produces a strong (but not cryptographic) hash of a small number of bytes. As a
// strong hash, it has the properties of near-random output (hashing any particular set of bytes will produce what looks
// like a random number), but is repeatable (hashing two identical set of bytes will produce identical output).

// This implementation uses a hard-coded window size of 16 bytes.
const WINDOW_SIZE: usize = 16;
const TWICE_WINDOW_SIZE: usize = 2 * WINDOW_SIZE;
const WINDOW_MASK: usize = 15; // 0x0F
const BITS_PER_BYTE: u8 = 8;

// The RollingHash struct stores a precomputed table of what happens mathematically to the hash when a new byte is
// pushed into the rolling window and the old byte is popped off. The push and pops tables could be moved to static
// constants if the construction time of RollingHash becomes an issue.
pub struct RollingHash {
    push: [u64; 256],
    pop: [u64; 256],
    hash: u64,
    queue: [u8; WINDOW_SIZE],
    next: usize,
}

impl RollingHash {
    pub fn new() -> RollingHash {
        // Create the push table by pre-computing what happens to every possible top byte when it gets modded
        let mut push = [0u64; 256];
        for i in 0u64..256u64 {
            let number = i << 56;
            push[i as usize] = shift_left_n_bits_with_mod_64(number, BITS_PER_BYTE);
        }

        // Create the pop table by pre computing the same value except we also need to include the size of the window
        let mut pop = [0u64; 256];
        for i in 0u64..256u64 {
            pop[i as usize] = shift_left_n_bits_with_mod_64(i, BITS_PER_BYTE * WINDOW_SIZE as u8);
        }

        RollingHash {
            push: push,
            pop: pop,
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
        self.hash = ((self.hash << 8) | (b as u64)) ^ self.push[high_byte];

        // Remove the old byte
        let old_byte = self.queue[self.next] as usize;
        self.hash ^= self.pop[old_byte];

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
