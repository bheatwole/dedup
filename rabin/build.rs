use std::io::Write;

// Don't want to define the these consts in two files (and have to maintain them), so we'll define them here and also
// write the ones that are used in the runtime code to the autogen file
const WINDOW_SIZE: usize = 16;                      // This implementation uses a hard-coded window size of 16 bytes.
const TWICE_WINDOW_SIZE: usize = 2 * WINDOW_SIZE;
const WINDOW_MASK: usize = WINDOW_SIZE - 1; // 0x0F
const BITS_PER_BYTE: u8 = 8;

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

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = std::path::Path::new(&out_dir).join("static_rolling_hash_autogen.rs");
    let mut f = std::fs::File::create(&dest_path).unwrap();
    
    // These consts are also used at runtime
    writeln!(f, "const WINDOW_SIZE: usize = {};", WINDOW_SIZE).unwrap();
    writeln!(f, "const TWICE_WINDOW_SIZE: usize = {};", TWICE_WINDOW_SIZE).unwrap();
    writeln!(f, "const WINDOW_MASK: usize = {};", WINDOW_MASK).unwrap();
    writeln!(f, "").unwrap();
    
    // Create the push table by pre-computing what happens to every possible top byte when it gets modded
    writeln!(f, "static ROLLING_HASH_PUSH_TABLE: [u64; 256] = [").unwrap();
    for i in 0u64..256u64 {
        let number = i << 56;
        writeln!(f, "    {},", shift_left_n_bits_with_mod_64(number, BITS_PER_BYTE)).unwrap();
    }
    writeln!(f, "];").unwrap();
    
    // Create the pop table by pre computing the same value except we also need to include the size of the window
    writeln!(f, "static ROLLING_HASH_POP_TABLE: [u64; 256] = [").unwrap();
    for i in 0u64..256u64 {
        writeln!(f, "    {},", shift_left_n_bits_with_mod_64(i, BITS_PER_BYTE * WINDOW_SIZE as u8)).unwrap();
    }
    writeln!(f, "];").unwrap();
}