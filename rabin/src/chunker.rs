// These two bitmasks are used to quickly check if a u64 has a certain number of '1' bits in the low word. The primary
// bitmask checks for 11 bits and the secondary checks for 10 bits.
const PRIMARY_BITMASK: u64 = 2047; // 2^11 - 1
const SECONDARY_BITMASK: u64 = 1023; // 2^10 - 1

// The Chunker takes a large number of bytes and breaks it into variably sized chunks based upon a two-divisor system
// that picks consistent break-points for the same hash of data. See the README for more information.
pub struct Chunker<'a> {
    hasher: crate::rolling_hash::RollingHash,
    mem: &'a [u8],
    min: usize,
    max: usize,
}

impl<'a> Chunker<'a> {
    // Creates a new Chunker where the chunk sizes will be at least 'min' (unless there aren't enough bytes left in the
    // data) and at most 'max'.
    pub fn new(mem: &'a [u8], min: usize, max: usize) -> Chunker<'a> {
        Chunker {
            hasher: crate::rolling_hash::RollingHash::new(),
            mem: mem,
            min: min,
            max: max,
        }
    }

    // Removes the specified number of bytes from the list of bytes to chunk and returns them.
    fn pop_front_chunk(&mut self, len: usize) -> &'a [u8] {
        let chunk = &self.mem[0..len];
        self.mem = &self.mem[len..];
        chunk
    }
}

// Chunks are discovered using this iterator, which will return Some(chunk_bytes) until all bytes have been chunked.
impl<'a> Iterator for Chunker<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        let len = self.mem.len();

        // If we've used all the bytes, return None
        if 0 == len {
            return None;
        }

        // If the remaining bytes are less than or equal to the minimum chunk size, just return them
        if len < self.min {
            let chunk = self.mem;
            self.mem = &self.mem[0..0];
            return Some(chunk);
        }

        // Calculate the hash of all bytes up to the minimum chunk size. This is efficient because the rolling hasher is
        // smart enough to skip calculations up to the rolling window size.
        self.hasher.reset();
        self.hasher.hash_bytes(&self.mem[0..self.min]);

        // Add one byte at a time to the hasher until we find a primary breaking point. If we don't find one by the max
        // size we'll need to use the secondary point if we can find it
        let mut secondary = 0;
        for i in self.min..self.max {
            // Don't exceed the total length of the memory buffer
            if i >= len {
                break;
            }

            // Add this byte and get the hash for the last few bytes.
            self.hasher.hash_byte(self.mem[i]);
            let hash = self.hasher.hash();

            // If we reached a primary boundary, this is where we make the chunk. Using '&' to check for a boundary has
            // a significant performance bump over '%'. The problem is that the divisor has to be a power of 2
            if hash & PRIMARY_BITMASK == PRIMARY_BITMASK {
                return Some(self.pop_front_chunk(i));
            }

            // Check for secondary boundary. We simply store the index of the last secondary boundary we found in the
            // hopes that we'll find a primary or another secondary.
            if hash & SECONDARY_BITMASK == SECONDARY_BITMASK {
                secondary = i;
            }
        }

        // If we reach this point, we didn't find a primary boundary. That means we need to make the chunk at either the
        // secondary break point (if we found one) or the max chunk size
        if 0 == secondary {
            secondary = self.max;
        }
        if secondary > len {
            secondary = len;
        }

        Some(self.pop_front_chunk(secondary))
    }
}
