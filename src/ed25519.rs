//! ED25519 Signature Scheme
//!
//! # Examples
//!
//! Creating a signature, and verifying the signature:
//!
//! ```
//! use cryptoxide::ed25519;
//!
//! let message = "messages".as_bytes();
//! let seed = [0u8;32]; // seed only for example !
//! let (secret, public) = ed25519::keypair(&seed[..]);
//! let signature = ed25519::signature(message, &secret[..]);
//! let verified = ed25519::verify(message, &public[..], &signature[..]);
//! assert!(verified);
//! ```
//!

use crate::curve25519::{curve25519, ge_scalarmult_base, sc_muladd, sc_reduce, Fe, GeP2, GeP3};
use crate::digest::Digest;
use crate::sha2::Sha512;
use crate::util::fixed_time_eq;
use core::ops::{Add, Mul, Sub};

pub const SEED_LENGTH: usize = 32;
pub const PRIVATE_KEY_LENGTH: usize = 64;
pub const PUBLIC_KEY_LENGTH: usize = 32;
pub const SIGNATURE_LENGTH: usize = 64;

static L: [u8; 32] = [
    0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x14, 0xde, 0xf9, 0xde, 0xa2, 0xf7, 0x9c, 0xd6, 0x58, 0x12, 0x63, 0x1a, 0x5c, 0xf5, 0xd3, 0xed,
];

/// Create a keypair of secret key and public key
pub fn keypair(seed: &[u8]) -> ([u8; PRIVATE_KEY_LENGTH], [u8; PUBLIC_KEY_LENGTH]) {
    assert!(
        seed.len() == SEED_LENGTH,
        "Seed should be {} bytes long!",
        SEED_LENGTH
    );

    let mut secret: [u8; PRIVATE_KEY_LENGTH] = {
        let mut hash_output: [u8; PRIVATE_KEY_LENGTH] = [0; PRIVATE_KEY_LENGTH];
        let mut hasher = Sha512::new();
        hasher.input(seed);
        hasher.result(&mut hash_output);
        hash_output[0] &= 248;
        hash_output[31] &= 63;
        hash_output[31] |= 64;
        hash_output
    };

    let a = ge_scalarmult_base(&secret[0..32]);
    let public_key = a.to_bytes();
    for (dest, src) in (&mut secret[32..64]).iter_mut().zip(public_key.iter()) {
        *dest = *src;
    }
    for (dest, src) in (&mut secret[0..32]).iter_mut().zip(seed.iter()) {
        *dest = *src;
    }
    (secret, public_key)
}

/// Generate a signature for the given message using a normal ED25519 secret key
pub fn signature(message: &[u8], secret_key: &[u8]) -> [u8; SIGNATURE_LENGTH] {
    assert!(
        secret_key.len() == PRIVATE_KEY_LENGTH,
        "Private key should be {} bytes long!",
        PRIVATE_KEY_LENGTH
    );

    let seed = &secret_key[0..32];
    let public_key = &secret_key[32..64];
    let az: [u8; 64] = {
        let mut hash_output: [u8; 64] = [0; 64];
        let mut hasher = Sha512::new();
        hasher.input(seed);
        hasher.result(&mut hash_output);
        hash_output[0] &= 248;
        hash_output[31] &= 63;
        hash_output[31] |= 64;
        hash_output
    };

    let nonce = {
        let mut hash_output: [u8; 64] = [0; 64];
        let mut hasher = Sha512::new();
        hasher.input(&az[32..64]);
        hasher.input(message);
        hasher.result(&mut hash_output);
        sc_reduce(&mut hash_output[0..64]);
        hash_output
    };

    let mut signature: [u8; SIGNATURE_LENGTH] = [0; SIGNATURE_LENGTH];
    let r: GeP3 = ge_scalarmult_base(&nonce[0..32]);
    for (result_byte, source_byte) in (&mut signature[0..32]).iter_mut().zip(r.to_bytes().iter()) {
        *result_byte = *source_byte;
    }
    for (result_byte, source_byte) in (&mut signature[32..64]).iter_mut().zip(public_key.iter()) {
        *result_byte = *source_byte;
    }

    {
        let mut hasher = Sha512::new();
        hasher.input(signature.as_ref());
        hasher.input(message);
        let mut hram: [u8; 64] = [0; 64];
        hasher.result(&mut hram);
        sc_reduce(&mut hram);
        sc_muladd(
            &mut signature[32..64],
            &hram[0..32],
            &az[0..32],
            &nonce[0..32],
        );
    }

    signature
}

/// generate the public key associated with an extended secret key
pub fn to_public(extended_secret: &[u8]) -> [u8; PUBLIC_KEY_LENGTH] {
    let a = ge_scalarmult_base(&extended_secret[0..32]);
    let public_key = a.to_bytes();
    public_key
}

/// Generate a signature for the given message using an extended ED25519 secret key
pub fn signature_extended(message: &[u8], extended_secret: &[u8]) -> [u8; SIGNATURE_LENGTH] {
    assert!(
        extended_secret.len() == PRIVATE_KEY_LENGTH,
        "Private key should be {} bytes long!",
        PRIVATE_KEY_LENGTH
    );
    let public_key = to_public(extended_secret);

    let nonce = {
        let mut hash_output: [u8; 64] = [0; 64];
        let mut hasher = Sha512::new();
        hasher.input(&extended_secret[32..64]);
        hasher.input(message);
        hasher.result(&mut hash_output);
        sc_reduce(&mut hash_output[0..64]);
        hash_output
    };

    let mut signature: [u8; SIGNATURE_LENGTH] = [0; SIGNATURE_LENGTH];
    let r: GeP3 = ge_scalarmult_base(&nonce[0..32]);
    for (result_byte, source_byte) in (&mut signature[0..32]).iter_mut().zip(r.to_bytes().iter()) {
        *result_byte = *source_byte;
    }
    for (result_byte, source_byte) in (&mut signature[32..64]).iter_mut().zip(public_key.iter()) {
        *result_byte = *source_byte;
    }

    {
        let mut hasher = Sha512::new();
        hasher.input(signature.as_ref());
        hasher.input(message);
        let mut hram: [u8; 64] = [0; 64];
        hasher.result(&mut hram);
        sc_reduce(&mut hram);
        sc_muladd(
            &mut signature[32..64],
            &hram[0..32],
            &extended_secret[0..32],
            &nonce[0..32],
        );
    }

    signature
}

fn check_s_lt_l(s: &[u8]) -> bool {
    let mut c: u8 = 0;
    let mut n: u8 = 1;

    let mut i = 31;
    loop {
        c |= ((((s[i] as i32) - (L[i] as i32)) >> 8) as u8) & n;
        n &= ((((s[i] ^ L[i]) as i32) - 1) >> 8) as u8;
        if i == 0 {
            break;
        } else {
            i -= 1;
        }
    }

    c == 0
}

/// Verify that a signature is valid for a given message for an associated public key
pub fn verify(message: &[u8], public_key: &[u8], signature: &[u8]) -> bool {
    assert!(
        public_key.len() == PUBLIC_KEY_LENGTH,
        "Public key should be {} bytes long!",
        PUBLIC_KEY_LENGTH
    );
    assert!(
        signature.len() == SIGNATURE_LENGTH,
        "signature should be {} bytes long!",
        SIGNATURE_LENGTH
    );

    if check_s_lt_l(&signature[32..64]) {
        return false;
    }

    let a = match GeP3::from_bytes_negate_vartime(public_key) {
        Some(g) => g,
        None => {
            return false;
        }
    };
    let mut d = 0;
    for pk_byte in public_key.iter() {
        d |= *pk_byte;
    }
    if d == 0 {
        return false;
    }

    let mut hasher = Sha512::new();
    hasher.input(&signature[0..32]);
    hasher.input(public_key);
    hasher.input(message);
    let mut hash: [u8; 64] = [0; 64];
    hasher.result(&mut hash);
    sc_reduce(&mut hash);

    let r = GeP2::double_scalarmult_vartime(hash.as_ref(), a, &signature[32..64]);
    let rcheck = r.to_bytes();

    fixed_time_eq(rcheck.as_ref(), &signature[0..32])
}

/// Curve25519 DH (Diffie Hellman) between a curve25519 public key and a ed25519 private key
pub fn exchange(public_key: &[u8], private_key: &[u8]) -> [u8; 32] {
    let ed_y = Fe::from_bytes(&public_key);
    // Produce public key in Montgomery form.
    let mont_x = edwards_to_montgomery_x(&ed_y);

    // Produce private key from seed component (bytes 0 to 32)
    // of the Ed25519 extended private key (64 bytes).
    let mut hasher = Sha512::new();
    hasher.input(&private_key[0..32]);
    let mut hash: [u8; 64] = [0; 64];
    hasher.result(&mut hash);
    // Clamp the hash such that it is a valid private key
    hash[0] &= 248;
    hash[31] &= 127;
    hash[31] |= 64;

    let shared_mont_x: [u8; 32] = curve25519(&hash, &mont_x.to_bytes()); // priv., pub.

    shared_mont_x
}

fn edwards_to_montgomery_x(ed_y: &Fe) -> Fe {
    let ed_z = &Fe([1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let temp_x = ed_z.add(ed_y);
    let temp_z = ed_z.sub(ed_y);
    let temp_z_inv = temp_z.invert();

    let mont_x = temp_x.mul(temp_z_inv);

    mont_x
}

#[cfg(test)]
mod tests {
    use super::{exchange, keypair, signature, verify};
    use crate::curve25519::{curve25519, curve25519_base};
    use crate::digest::Digest;
    use crate::sha2::Sha512;

    fn do_keypair_case(seed: [u8; 32], expected_secret: [u8; 64], expected_public: [u8; 32]) {
        let (actual_secret, actual_public) = keypair(seed.as_ref());
        assert_eq!(actual_secret.to_vec(), expected_secret.to_vec());
        assert_eq!(actual_public.to_vec(), expected_public.to_vec());
    }

    #[test]
    fn keypair_cases() {
        do_keypair_case(
            [
                0x26, 0x27, 0xf6, 0x85, 0x97, 0x15, 0xad, 0x1d, 0xd2, 0x94, 0xdd, 0xc4, 0x76, 0x19,
                0x39, 0x31, 0xf1, 0xad, 0xb5, 0x58, 0xf0, 0x93, 0x97, 0x32, 0x19, 0x2b, 0xd1, 0xc0,
                0xfd, 0x16, 0x8e, 0x4e,
            ],
            [
                0x26, 0x27, 0xf6, 0x85, 0x97, 0x15, 0xad, 0x1d, 0xd2, 0x94, 0xdd, 0xc4, 0x76, 0x19,
                0x39, 0x31, 0xf1, 0xad, 0xb5, 0x58, 0xf0, 0x93, 0x97, 0x32, 0x19, 0x2b, 0xd1, 0xc0,
                0xfd, 0x16, 0x8e, 0x4e, 0x5d, 0x6d, 0x23, 0x6b, 0x52, 0xd1, 0x8e, 0x3a, 0xb6, 0xd6,
                0x07, 0x2f, 0xb6, 0xe4, 0xc7, 0xd4, 0x6b, 0xd5, 0x9a, 0xd9, 0xcc, 0x19, 0x47, 0x26,
                0x5f, 0x00, 0xb7, 0x20, 0xfa, 0x2c, 0x8f, 0x66,
            ],
            [
                0x5d, 0x6d, 0x23, 0x6b, 0x52, 0xd1, 0x8e, 0x3a, 0xb6, 0xd6, 0x07, 0x2f, 0xb6, 0xe4,
                0xc7, 0xd4, 0x6b, 0xd5, 0x9a, 0xd9, 0xcc, 0x19, 0x47, 0x26, 0x5f, 0x00, 0xb7, 0x20,
                0xfa, 0x2c, 0x8f, 0x66,
            ],
        );
        do_keypair_case(
            [
                0x29, 0x23, 0xbe, 0x84, 0xe1, 0x6c, 0xd6, 0xae, 0x52, 0x90, 0x49, 0xf1, 0xf1, 0xbb,
                0xe9, 0xeb, 0xb3, 0xa6, 0xdb, 0x3c, 0x87, 0x0c, 0x3e, 0x99, 0x24, 0x5e, 0x0d, 0x1c,
                0x06, 0xb7, 0x47, 0xde,
            ],
            [
                0x29, 0x23, 0xbe, 0x84, 0xe1, 0x6c, 0xd6, 0xae, 0x52, 0x90, 0x49, 0xf1, 0xf1, 0xbb,
                0xe9, 0xeb, 0xb3, 0xa6, 0xdb, 0x3c, 0x87, 0x0c, 0x3e, 0x99, 0x24, 0x5e, 0x0d, 0x1c,
                0x06, 0xb7, 0x47, 0xde, 0x5d, 0x83, 0x31, 0x26, 0x56, 0x0c, 0xb1, 0x9a, 0x14, 0x19,
                0x37, 0x27, 0x78, 0x96, 0xf0, 0xfd, 0x43, 0x7b, 0xa6, 0x80, 0x1e, 0xb2, 0x10, 0xac,
                0x4c, 0x39, 0xd9, 0x00, 0x72, 0xd7, 0x0d, 0xa8,
            ],
            [
                0x5d, 0x83, 0x31, 0x26, 0x56, 0x0c, 0xb1, 0x9a, 0x14, 0x19, 0x37, 0x27, 0x78, 0x96,
                0xf0, 0xfd, 0x43, 0x7b, 0xa6, 0x80, 0x1e, 0xb2, 0x10, 0xac, 0x4c, 0x39, 0xd9, 0x00,
                0x72, 0xd7, 0x0d, 0xa8,
            ],
        );
    }

    #[test]
    fn keypair_matches_mont() {
        let seed = [
            0x26, 0x27, 0xf6, 0x85, 0x97, 0x15, 0xad, 0x1d, 0xd2, 0x94, 0xdd, 0xc4, 0x76, 0x19,
            0x39, 0x31, 0xf1, 0xad, 0xb5, 0x58, 0xf0, 0x93, 0x97, 0x32, 0x19, 0x2b, 0xd1, 0xc0,
            0xfd, 0x16, 0x8e, 0x4e,
        ];
        let (ed_private, ed_public) = keypair(seed.as_ref());

        let mut hasher = Sha512::new();
        hasher.input(&ed_private[0..32]);
        let mut hash: [u8; 64] = [0; 64];
        hasher.result(&mut hash);
        hash[0] &= 248;
        hash[31] &= 127;
        hash[31] |= 64;

        let cv_public = curve25519_base(&hash);

        let edx_ss = exchange(&ed_public, &ed_private);
        let cv_ss = curve25519(&hash, &cv_public);

        assert_eq!(edx_ss.to_vec(), cv_ss.to_vec());
    }

    fn do_sign_verify_case(seed: [u8; 32], message: &[u8], expected_signature: [u8; 64]) {
        let (secret_key, public_key) = keypair(seed.as_ref());
        let mut actual_signature = signature(message, secret_key.as_ref());
        assert_eq!(expected_signature.to_vec(), actual_signature.to_vec());
        assert!(verify(
            message,
            public_key.as_ref(),
            actual_signature.as_ref()
        ));

        for &(index, flip) in [(0, 1), (31, 0x80), (20, 0xff)].iter() {
            actual_signature[index] ^= flip;
            assert!(!verify(
                message,
                public_key.as_ref(),
                actual_signature.as_ref()
            ));
            actual_signature[index] ^= flip;
        }

        let mut public_key_corrupt = public_key;
        public_key_corrupt[0] ^= 1;
        assert!(!verify(
            message,
            public_key_corrupt.as_ref(),
            actual_signature.as_ref()
        ));
    }

    #[test]
    fn sign_verify_cases() {
        do_sign_verify_case(
            [
                0x2d, 0x20, 0x86, 0x83, 0x2c, 0xc2, 0xfe, 0x3f, 0xd1, 0x8c, 0xb5, 0x1d, 0x6c, 0x5e,
                0x99, 0xa5, 0x75, 0x9f, 0x02, 0x21, 0x1f, 0x85, 0xe5, 0xff, 0x2f, 0x90, 0x4a, 0x78,
                0x0f, 0x58, 0x00, 0x6f,
            ],
            [
                0x89, 0x8f, 0x9c, 0x4b, 0x2c, 0x6e, 0xe9, 0xe2, 0x28, 0x76, 0x1c, 0xa5, 0x08, 0x97,
                0xb7, 0x1f, 0xfe, 0xca, 0x1c, 0x35, 0x28, 0x46, 0xf5, 0xfe, 0x13, 0xf7, 0xd3, 0xd5,
                0x7e, 0x2c, 0x15, 0xac, 0x60, 0x90, 0x0c, 0xa3, 0x2c, 0x5b, 0x5d, 0xd9, 0x53, 0xc9,
                0xa6, 0x81, 0x0a, 0xcc, 0x64, 0x39, 0x4f, 0xfd, 0x14, 0x98, 0x26, 0xd9, 0x98, 0x06,
                0x29, 0x2a, 0xdd, 0xd1, 0x3f, 0xc3, 0xbb, 0x7d, 0xac, 0x70, 0x1c, 0x5b, 0x4a, 0x2d,
                0x61, 0x5d, 0x15, 0x96, 0x01, 0x28, 0xed, 0x9f, 0x73, 0x6b, 0x98, 0x85, 0x4f, 0x6f,
                0x07, 0x05, 0xb0, 0xf0, 0xda, 0xcb, 0xdc, 0x2c, 0x26, 0x2d, 0x27, 0x39, 0x75, 0x19,
                0x14, 0x9b, 0x0e, 0x4c, 0xbe, 0x16, 0x77, 0xc5, 0x76, 0xc1, 0x39, 0x7a, 0xae, 0x5c,
                0xe3, 0x49, 0x16, 0xe3, 0x51, 0x31, 0x04, 0x63, 0x2e, 0xc2, 0x19, 0x0d, 0xb8, 0xd2,
                0x22, 0x89, 0xc3, 0x72, 0x3c, 0x8d, 0x01, 0x21, 0x3c, 0xad, 0x80, 0x3f, 0x4d, 0x75,
                0x74, 0xc4, 0xdb, 0xb5, 0x37, 0x31, 0xb0, 0x1c, 0x8e, 0xc7, 0x5d, 0x08, 0x2e, 0xf7,
                0xdc, 0x9d, 0x7f, 0x1b, 0x73, 0x15, 0x9f, 0x63, 0xdb, 0x56, 0xaa, 0x12, 0xa2, 0xca,
                0x39, 0xea, 0xce, 0x6b, 0x28, 0xe4, 0xc3, 0x1d, 0x9d, 0x25, 0x67, 0x41, 0x45, 0x2e,
                0x83, 0x87, 0xe1, 0x53, 0x6d, 0x03, 0x02, 0x6e, 0xe4, 0x84, 0x10, 0xd4, 0x3b, 0x21,
                0x91, 0x88, 0xba, 0x14, 0xa8, 0xaf,
            ]
            .as_ref(),
            [
                0x91, 0x20, 0x91, 0x66, 0x1e, 0xed, 0x18, 0xa4, 0x03, 0x4b, 0xc7, 0xdb, 0x4b, 0xd6,
                0x0f, 0xe2, 0xde, 0xeb, 0xf3, 0xff, 0x3b, 0x6b, 0x99, 0x8d, 0xae, 0x20, 0x94, 0xb6,
                0x09, 0x86, 0x5c, 0x20, 0x19, 0xec, 0x67, 0x22, 0xbf, 0xdc, 0x87, 0xbd, 0xa5, 0x40,
                0x91, 0x92, 0x2e, 0x11, 0xe3, 0x93, 0xf5, 0xfd, 0xce, 0xea, 0x3e, 0x09, 0x1f, 0x2e,
                0xe6, 0xbc, 0x62, 0xdf, 0x94, 0x8e, 0x99, 0x09,
            ],
        );
        do_sign_verify_case(
            [
                0x33, 0x19, 0x17, 0x82, 0xc1, 0x70, 0x4f, 0x60, 0xd0, 0x84, 0x8d, 0x75, 0x62, 0xa2,
                0xfa, 0x19, 0xf9, 0x92, 0x4f, 0xea, 0x4e, 0x77, 0x33, 0xcd, 0x45, 0xf6, 0xc3, 0x2f,
                0x21, 0x9a, 0x72, 0x91,
            ],
            [
                0x77, 0x13, 0x43, 0x5a, 0x0e, 0x34, 0x6f, 0x67, 0x71, 0xae, 0x5a, 0xde, 0xa8, 0x7a,
                0xe7, 0xa4, 0x52, 0xc6, 0x5d, 0x74, 0x8f, 0x48, 0x69, 0xd3, 0x1e, 0xd3, 0x67, 0x47,
                0xc3, 0x28, 0xdd, 0xc4, 0xec, 0x0e, 0x48, 0x67, 0x93, 0xa5, 0x1c, 0x67, 0x66, 0xf7,
                0x06, 0x48, 0x26, 0xd0, 0x74, 0x51, 0x4d, 0xd0, 0x57, 0x41, 0xf3, 0xbe, 0x27, 0x3e,
                0xf2, 0x1f, 0x28, 0x0e, 0x49, 0x07, 0xed, 0x89, 0xbe, 0x30, 0x1a, 0x4e, 0xc8, 0x49,
                0x6e, 0xb6, 0xab, 0x90, 0x00, 0x06, 0xe5, 0xa3, 0xc8, 0xe9, 0xc9, 0x93, 0x62, 0x1d,
                0x6a, 0x3b, 0x0f, 0x6c, 0xba, 0xd0, 0xfd, 0xde, 0xf3, 0xb9, 0xc8, 0x2d,
            ]
            .as_ref(),
            [
                0x4b, 0x8d, 0x9b, 0x1e, 0xca, 0x54, 0x00, 0xea, 0xc6, 0xf5, 0xcc, 0x0c, 0x94, 0x39,
                0x63, 0x00, 0x52, 0xf7, 0x34, 0xce, 0x45, 0x3e, 0x94, 0x26, 0xf3, 0x19, 0xdd, 0x96,
                0x03, 0xb6, 0xae, 0xae, 0xb9, 0xd2, 0x3a, 0x5f, 0x93, 0xf0, 0x6a, 0x46, 0x00, 0x18,
                0xf0, 0x69, 0xdf, 0x19, 0x44, 0x48, 0xf5, 0x60, 0x51, 0xab, 0x9e, 0x6b, 0xfa, 0xeb,
                0x64, 0x10, 0x16, 0xf7, 0xa9, 0x0b, 0xe2, 0x0c,
            ],
        );
    }
}
