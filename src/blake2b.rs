//! Blake2B hash function

// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::blake2::{EngineB as Engine, LastBlock};
use crate::cryptoutil::{copy_memory, write_u64v_le};
use crate::digest::Digest;
use crate::mac::{Mac, MacResult};
use crate::util::secure_memset;
use alloc::vec::Vec;
use core::iter::repeat;

/// Blake2b Context
#[derive(Clone)]
pub struct Blake2b<const BITS: usize> {
    eng: Engine,
    buf: [u8; Engine::BLOCK_BYTES],
    buflen: usize,
    computed: bool, // whether the final digest has been computed
}

impl<const BITS: usize> Blake2b<BITS> {
    /// Create a new Blake2b context with a specific output size in bytes
    ///
    /// the size need to be between 0 (non included) and 64 bytes (included)
    pub fn new() -> Self {
        assert!(BITS > 0 && ((BITS + 7) / 8) <= Engine::MAX_OUTLEN);
        Self::new_keyed(&[])
    }

    /// Similar to `new` but also takes a variable size key
    /// to tweak the context initialization
    pub fn new_keyed(key: &[u8]) -> Self {
        assert!(BITS > 0 && ((BITS + 7) / 8) <= Engine::MAX_OUTLEN);
        assert!(key.len() <= Engine::MAX_KEYLEN);

        let mut buf = [0u8; Engine::BLOCK_BYTES];

        let eng = Engine::new((BITS + 7) / 8, key.len());
        let buflen = if !key.is_empty() {
            buf[0..key.len()].copy_from_slice(key);
            Engine::BLOCK_BYTES
        } else {
            0
        };

        Blake2b {
            eng,
            buf,
            buflen,
            computed: false,
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        if input.is_empty() {
            return;
        }
        let fill = Engine::BLOCK_BYTES - self.buflen;

        if input.len() > fill {
            copy_memory(&input[0..fill], &mut self.buf[self.buflen..]);
            self.buflen = 0;
            self.eng.increment_counter(Engine::BLOCK_BYTES_NATIVE);
            self.eng
                .compress(&self.buf[0..Engine::BLOCK_BYTES], LastBlock::No);

            input = &input[fill..];

            while input.len() > Engine::BLOCK_BYTES {
                self.eng.increment_counter(Engine::BLOCK_BYTES_NATIVE);
                self.eng
                    .compress(&input[0..Engine::BLOCK_BYTES], LastBlock::No);
                input = &input[Engine::BLOCK_BYTES..];
            }
        }
        copy_memory(input, &mut self.buf[self.buflen..]);
        self.buflen += input.len();
    }

    fn finalize(&mut self, out: &mut [u8]) {
        assert!(out.len() == ((BITS + 7) / 8));
        if !self.computed {
            self.eng.increment_counter(self.buflen as u64);
            secure_memset(&mut self.buf[self.buflen..], 0);
            self.eng
                .compress(&self.buf[0..Engine::BLOCK_BYTES], LastBlock::Yes);

            write_u64v_le(&mut self.buf[0..64], &self.eng.h);
            self.computed = true;
        }
        copy_memory(&self.buf[0..out.len()], out);
    }

    /// Reset the context to the state after calling `new`
    pub fn reset(&mut self) {
        self.eng.reset((BITS + 7) / 8, 0);
        self.computed = false;
        self.buflen = 0;
        secure_memset(&mut self.buf[..], 0);
    }

    pub fn reset_with_key(&mut self, key: &[u8]) {
        assert!(key.len() <= Engine::MAX_KEYLEN);

        self.eng.reset((BITS + 7) / 8, key.len());
        self.computed = false;
        secure_memset(&mut self.buf[..], 0);

        if !key.is_empty() {
            self.buf[0..key.len()].copy_from_slice(key);
            self.buflen = Engine::BLOCK_BYTES;
        } else {
            self.buf = [0; Engine::BLOCK_BYTES];
            self.buflen = 0;
        }
    }

    pub fn blake2b(out: &mut [u8], input: &[u8], key: &[u8]) {
        let mut hasher: Self = if !key.is_empty() {
            Blake2b::new_keyed(key)
        } else {
            Blake2b::new()
        };

        hasher.update(input);
        hasher.finalize(out);
    }
}

impl<const BITS: usize> Digest for Blake2b<BITS> {
    const OUTPUT_BITS: usize = BITS;
    fn input(&mut self, msg: &[u8]) {
        self.update(msg);
    }
    fn reset(&mut self) {
        Blake2b::reset(self);
    }
    fn result(&mut self, out: &mut [u8]) {
        self.finalize(out);
    }
    fn block_size(&self) -> usize {
        Engine::BLOCK_BYTES
    }
}

impl<const BITS: usize> Mac for Blake2b<BITS> {
    /**
     * Process input data.
     *
     * # Arguments
     * * data - The input data to process.
     *
     */
    fn input(&mut self, data: &[u8]) {
        self.update(data);
    }

    /**
     * Reset the Mac state to begin processing another input stream.
     */
    fn reset(&mut self) {
        Blake2b::reset(self);
    }

    /**
     * Obtain the result of a Mac computation as a MacResult.
     */
    fn result(&mut self) -> MacResult {
        let mut mac: Vec<u8> = repeat(0).take((BITS + 7) / 8).collect();
        self.raw_result(&mut mac);
        MacResult::new_from_owned(mac)
    }

    /**
     * Obtain the result of a Mac computation as [u8]. This method should be used very carefully
     * since incorrect use of the Mac code could result in permitting a timing attack which defeats
     * the security provided by a Mac function.
     */
    fn raw_result(&mut self, output: &mut [u8]) {
        self.finalize(output);
    }

    /**
     * Get the size of the Mac code, in bytes.
     */
    fn output_bytes(&self) -> usize {
        (BITS + 7) / 8
    }
}

#[cfg(test)]
mod hash_tests {
    use super::Blake2b;

    #[test]
    fn test_vector() {
        let mut out = [0u8; 64];
        Blake2b::<512>::blake2b(&mut out, b"abc", &[]);
        let expected = [
            0xBA, 0x80, 0xA5, 0x3F, 0x98, 0x1C, 0x4D, 0x0D, 0x6A, 0x27, 0x97, 0xB6, 0x9F, 0x12,
            0xF6, 0xE9, 0x4C, 0x21, 0x2F, 0x14, 0x68, 0x5A, 0xC4, 0xB7, 0x4B, 0x12, 0xBB, 0x6F,
            0xDB, 0xFF, 0xA2, 0xD1, 0x7D, 0x87, 0xC5, 0x39, 0x2A, 0xAB, 0x79, 0x2D, 0xC2, 0x52,
            0xD5, 0xDE, 0x45, 0x33, 0xCC, 0x95, 0x18, 0xD3, 0x8A, 0xA8, 0xDB, 0xF1, 0x92, 0x5A,
            0xB9, 0x23, 0x86, 0xED, 0xD4, 0x00, 0x99, 0x23,
        ];
        assert_eq!(&out[..], &expected[..])
    }
}

#[cfg(test)]
mod mac_tests {
    use super::Blake2b;
    use crate::mac::Mac;
    use std::vec::Vec;

    #[test]
    fn test_reset_with_key_same_as_new_keyed_if_empty() {
        const KEY: &[u8] = &[];
        const INPUT: &[u8] = &[];
        let mut m = Blake2b::<256>::new_keyed(&KEY);
        m.input(&INPUT);

        let mac1 = m.result();

        m.reset_with_key(&KEY);
        m.input(&INPUT);

        let mac2 = m.result();

        assert_eq!(mac1.code(), mac2.code());
    }

    #[test]
    fn test_blake2b_mac() {
        let key: Vec<u8> = (0..64).collect();
        let mut m = Blake2b::<512>::new_keyed(&key[..]);
        m.input(&[1, 2, 4, 8]);
        let expected = [
            0x8e, 0xc6, 0xcb, 0x71, 0xc4, 0x5c, 0x3c, 0x90, 0x91, 0xd0, 0x8a, 0x37, 0x1e, 0xa8,
            0x5d, 0xc1, 0x22, 0xb5, 0xc8, 0xe2, 0xd9, 0xe5, 0x71, 0x42, 0xbf, 0xef, 0xce, 0x42,
            0xd7, 0xbc, 0xf8, 0x8b, 0xb0, 0x31, 0x27, 0x88, 0x2e, 0x51, 0xa9, 0x21, 0x44, 0x62,
            0x08, 0xf6, 0xa3, 0x58, 0xa9, 0xe0, 0x7d, 0x35, 0x3b, 0xd3, 0x1c, 0x41, 0x70, 0x15,
            0x62, 0xac, 0xd5, 0x39, 0x4e, 0xee, 0x73, 0xae,
        ];
        assert_eq!(m.result().code().to_vec(), expected.to_vec());
    }
}

#[cfg(all(test, feature = "with-bench"))]
mod bench {
    use test::Bencher;

    use super::Blake2b;
    use crate::digest::Digest;

    #[bench]
    pub fn blake2b_10(bh: &mut Bencher) {
        let mut sh = Blake2b::new(64);
        let bytes = [1u8; 10];
        bh.iter(|| {
            sh.input(&bytes);
        });
        bh.bytes = bytes.len() as u64;
    }

    #[bench]
    pub fn blake2b_1k(bh: &mut Bencher) {
        let mut sh = Blake2b::new(64);
        let bytes = [1u8; 1024];
        bh.iter(|| {
            sh.input(&bytes);
        });
        bh.bytes = bytes.len() as u64;
    }

    #[bench]
    pub fn blake2b_64k(bh: &mut Bencher) {
        let mut sh = Blake2b::new(64);
        let bytes = [1u8; 65536];
        bh.iter(|| {
            sh.input(&bytes);
        });
        bh.bytes = bytes.len() as u64;
    }
}
