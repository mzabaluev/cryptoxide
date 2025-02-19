//! This module implements the PBKDF2 Key Derivation Function as specified in [Specification][1].
//!
//! # Examples
//!
//! ```
//! use cryptoxide::{pbkdf2::pbkdf2, hmac::Hmac, sha2::Sha256};
//!
//! let password = b"password";
//! let salt = b"salt";
//! let c = 2;
//! let mut out = [0u8; 64];
//! pbkdf2(&mut Hmac::new(Sha256::new(), password), salt, c, &mut out);
//! ```
//!
//! [1]: <https://tools.ietf.org/html/rfc2898>

// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::cryptoutil::{copy_memory, write_u32_be};
use crate::mac::Mac;
use alloc::vec::Vec;
use core::iter::repeat;

// Calculate a block of the output of size equal to the output_bytes of the underlying Mac function
// `mac` - The Mac function to use
// `salt` - the salt value to use
// `c` - the iteration count
// `idx` - the 1 based index of the block
// `scratch` - a temporary variable the same length as the block
// `block` - the block of the output to calculate
fn calculate_block<M: Mac>(
    mac: &mut M,
    salt: &[u8],
    c: u32,
    idx: u32,
    scratch: &mut [u8],
    block: &mut [u8],
) {
    // Perform the 1st iteration. The output goes directly into block
    mac.input(salt);
    let mut idx_buf = [0u8; 4];
    write_u32_be(&mut idx_buf, idx);
    mac.input(&idx_buf);
    mac.raw_result(block);
    mac.reset();

    // Perform the 2nd iteration. The input comes from block and is output into scratch. scratch is
    // then exclusive-or added into block. After all this, the input to the next step is now in
    // scratch and block is left to just accumulate the exclusive-of sum of remaining iterations.
    if c > 1 {
        mac.input(block);
        mac.raw_result(scratch);
        mac.reset();
        for (output, &input) in block.iter_mut().zip(scratch.iter()) {
            *output ^= input;
        }
    }

    // Perform all remaining iterations
    for _ in 2..c {
        mac.input(scratch);
        mac.raw_result(scratch);
        mac.reset();
        for (output, &input) in block.iter_mut().zip(scratch.iter()) {
            *output ^= input;
        }
    }
}

/**
 * Execute the PBKDF2 Key Derivation Function. The Scrypt Key Derivation Function generally provides
 * better security, so, applications that do not have a requirement to use PBKDF2 specifically
 * should consider using that function instead.
 *
 * # Arguments
 * * `mac` - The Pseudo Random Function to use.
 * * `salt` - The salt value to use.
 * * `c` - The iteration count. Users should carefully determine this value as it is the primary
 *       factor in determining the security of the derived key.
 * * `output` - The output buffer to fill with the derived key value.
 *
 */
pub fn pbkdf2<M: Mac>(mac: &mut M, salt: &[u8], c: u32, output: &mut [u8]) {
    assert!(c > 0);

    let os = mac.output_bytes();

    // A temporary storage array needed by calculate_block. This is really only necessary if c > 1.
    // Most users of pbkdf2 should use a value much larger than 1, so, this allocation should almost
    // always be necessary. A big exception is Scrypt. However, this allocation is unlikely to be
    // the bottleneck in Scrypt performance.
    let mut scratch: Vec<u8> = repeat(0).take(os).collect();

    let mut idx: u32 = 0;

    for chunk in output.chunks_mut(os) {
        // The block index starts at 1. So, this is supposed to run on the first execution.
        idx = idx.checked_add(1).expect("PBKDF2 size limit exceeded.");

        if chunk.len() == os {
            calculate_block(mac, salt, c, idx, &mut scratch, chunk);
        } else {
            let mut tmp: Vec<u8> = repeat(0).take(os).collect();
            calculate_block(mac, salt, c, idx, &mut scratch[..], &mut tmp[..]);
            let chunk_len = chunk.len();
            copy_memory(&tmp[..chunk_len], chunk);
        }
    }
}

#[cfg(test)]
mod test {
    use super::pbkdf2;
    use crate::hmac::Hmac;
    use crate::sha1::Sha1;

    #[test]
    fn test1() {
        let password = b"password";
        let salt = b"salt";
        let c = 2;
        let mut out = [0u8; 20];
        pbkdf2(&mut Hmac::new(Sha1::new(), password), salt, c, &mut out);
        assert_eq!(
            out,
            [
                0xea, 0x6c, 0x01, 0x4d, 0xc7, 0x2d, 0x6f, 0x8c, 0xcd, 0x1e, 0xd9, 0x2a, 0xce, 0x1d,
                0x41, 0xf0, 0xd8, 0xde, 0x89, 0x57
            ]
        )
    }
}
