use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockModeDecrypt, KeyIvInit};
use log::error;
use sha2::{Digest, Sha256};
use std::env;

// `.env` is read once at startup, hence the restart.
const MISSING_SECRET_MESSAGE: &str = "No secret found in .env. Do this:\n\
mkdir -p ~/.config/absotui\n\
KEY=$(od -An -tx1 -N32 /dev/urandom | tr -d ' \\n')\n\
echo \"ABSOTUI_SECRET_KEY=$KEY\" >> ~/.config/absotui/.env\n\
then restart Absotui.";

/// What a saved token is stored as: `v2:` + base64 of
/// `[argon2 memory KiB: u32 BE][iterations: u8][lanes: u8][salt: 16][nonce: 12][AES-256-GCM sealed]`.
/// A fresh random salt gives every token its own key, and the header (cost and salt) is
/// authenticated along with the token. Anything without the prefix is the older format.
const CURRENT_PREFIX: &str = "v2:";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
const HEADER_LEN: usize = 4 + 1 + 1 + SALT_LEN;

#[derive(Clone, Copy)]
struct KdfCost {
    memory_kib: u32,
    iterations: u8,
    lanes: u8,
}

// Argon2id at OWASP's minimum profile (19 MiB, 2 passes, 1 lane).
const DEFAULT_COST: KdfCost = KdfCost { memory_kib: 19 * 1024, iterations: 2, lanes: 1 };

// A stored header asking for more than this is refused, so a tampered database cannot
// make startup allocate gigabytes.
const MAX_MEMORY_KIB: u32 = 256 * 1024;
const MAX_ITERATIONS: u8 = 10;
const MAX_LANES: u8 = 4;

fn derive_key(secret: &str, salt: &[u8], cost: KdfCost) -> Option<[u8; 32]> {
    let params = Params::new(cost.memory_kib, u32::from(cost.iterations), u32::from(cost.lanes), Some(32)).ok()?;
    let mut key = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(secret.as_bytes(), salt, &mut key)
        .ok()?;
    Some(key)
}

fn encrypt_with(secret: &str, plaintext: &str, cost: KdfCost) -> Option<String> {
    let mut header = [0u8; HEADER_LEN];
    header[..4].copy_from_slice(&cost.memory_kib.to_be_bytes());
    header[4] = cost.iterations;
    header[5] = cost.lanes;
    getrandom::fill(&mut header[6..]).ok()?;
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).ok()?;

    let key = derive_key(secret, &header[6..], cost)?;
    let cipher = Aes256Gcm::new_from_slice(&key).ok()?;
    let sealed = cipher
        .encrypt(&Nonce::try_from(&nonce[..]).ok()?, Payload { msg: plaintext.as_bytes(), aad: &header })
        .ok()?;

    let mut out = Vec::with_capacity(HEADER_LEN + NONCE_LEN + sealed.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Some(format!("{CURRENT_PREFIX}{}", STANDARD.encode(out)))
}

fn decrypt_current(secret: &str, stored: &str) -> Option<String> {
    let raw = STANDARD.decode(stored.strip_prefix(CURRENT_PREFIX)?).ok()?;
    if raw.len() < HEADER_LEN + NONCE_LEN + TAG_LEN {
        return None;
    }
    let (header, rest) = raw.split_at(HEADER_LEN);
    let (nonce, sealed) = rest.split_at(NONCE_LEN);
    let cost = KdfCost {
        memory_kib: u32::from_be_bytes(header[..4].try_into().ok()?),
        iterations: header[4],
        lanes: header[5],
    };
    if cost.memory_kib > MAX_MEMORY_KIB || cost.iterations > MAX_ITERATIONS || cost.lanes > MAX_LANES {
        return None;
    }

    let key = derive_key(secret, &header[6..], cost)?;
    let cipher = Aes256Gcm::new_from_slice(&key).ok()?;
    let plain = cipher
        .decrypt(&Nonce::try_from(nonce).ok()?, Payload { msg: sealed, aad: header })
        .ok()?;
    String::from_utf8(plain).ok()
}

/// The format this app wrote until the move to `v2:` (the `magic-crypt` crate, which has an
/// unpatched advisory): AES-256-CBC with PKCS#7 padding, key = SHA-256 of the secret, an
/// all-zero IV, standard base64. Only ever read, so saved logins survive the change.
fn decrypt_legacy(secret: &str, stored: &str) -> Option<String> {
    let key = Sha256::digest(secret.as_bytes());
    let iv = [0u8; 16];
    let ciphertext = STANDARD.decode(stored).ok()?;
    let plain = cbc::Decryptor::<aes::Aes256>::new_from_slices(&key, &iv)
        .ok()?
        .decrypt_padded_vec::<Pkcs7>(&ciphertext)
        .ok()?;
    String::from_utf8(plain).ok()
}

/// True for a saved value still in the older format (an empty value is "nothing saved").
pub fn is_legacy_format(stored: &str) -> bool {
    !stored.is_empty() && !stored.starts_with(CURRENT_PREFIX)
}

pub fn encrypt_token(token_to_encrypt: &str) -> Result<String, String> {
    // `.env` is loaded into the environment at startup (see `main.rs`).
    let Ok(key) = env::var("ABSOTUI_SECRET_KEY") else {
        error!("{MISSING_SECRET_MESSAGE}");
        return Err(MISSING_SECRET_MESSAGE.to_string());
    };
    encrypt_with(&key, token_to_encrypt, DEFAULT_COST).ok_or_else(|| {
        error!("Failed to encrypt the token.");
        "Failed to encrypt the token.".to_string()
    })
}

pub fn decrypt_token(encrypted_token: &str) -> Result<String, String> {
    let Ok(key) = env::var("ABSOTUI_SECRET_KEY") else {
        error!("{MISSING_SECRET_MESSAGE}");
        return Err(MISSING_SECRET_MESSAGE.to_string());
    };
    let decrypted = if is_legacy_format(encrypted_token) || encrypted_token.is_empty() {
        decrypt_legacy(&key, encrypted_token)
    } else {
        decrypt_current(&key, encrypted_token)
    };
    decrypted.ok_or_else(|| {
        error!("Failed to decrypt the token.");
        "Failed to decrypt the token.".to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Cheap settings so the suite stays fast; the stored header carries them, so the same
    // decrypt path runs as for the real cost.
    const TEST_COST: KdfCost = KdfCost { memory_kib: 8, iterations: 1, lanes: 1 };

    #[test]
    fn missing_secret_message_has_no_stray_indentation_and_says_to_restart() {
        assert!(!MISSING_SECRET_MESSAGE.contains("  "));
        assert_eq!(MISSING_SECRET_MESSAGE.lines().count(), 5);
        assert!(MISSING_SECRET_MESSAGE.lines().all(|line| line.len() <= 78), "a longer line wraps on an 80-column terminal");
        assert!(MISSING_SECRET_MESSAGE.ends_with("then restart Absotui."));
    }

    #[test]
    fn missing_secret_message_generates_a_random_key_not_a_guessable_one() {
        assert!(MISSING_SECRET_MESSAGE.contains("KEY=$(od -An -tx1 -N32 /dev/urandom"));
        assert!(MISSING_SECRET_MESSAGE.contains("ABSOTUI_SECRET_KEY=$KEY"));
        assert!(!MISSING_SECRET_MESSAGE.contains("=secret"));
    }

    #[test]
    fn a_token_roundtrips_and_is_not_stored_in_the_clear() {
        let stored = encrypt_with("a-secret", "some-fake-audiobookshelf-token-abc123", TEST_COST).unwrap();
        assert!(stored.starts_with(CURRENT_PREFIX));
        assert!(!stored.contains("audiobookshelf"));
        assert_eq!(decrypt_current("a-secret", &stored).unwrap(), "some-fake-audiobookshelf-token-abc123");
    }

    #[test]
    fn an_empty_token_roundtrips() {
        let stored = encrypt_with("a-secret", "", TEST_COST).unwrap();
        assert_eq!(decrypt_current("a-secret", &stored).unwrap(), "");
    }

    #[test]
    fn the_same_token_is_stored_differently_each_time() {
        let a = encrypt_with("a-secret", "same-token", TEST_COST).unwrap();
        let b = encrypt_with("a-secret", "same-token", TEST_COST).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn the_wrong_secret_is_rejected() {
        let stored = encrypt_with("a-secret", "token", TEST_COST).unwrap();
        assert!(decrypt_current("another-secret", &stored).is_none());
    }

    fn flip_byte(stored: &str, index: usize) -> String {
        let mut raw = STANDARD.decode(stored.strip_prefix(CURRENT_PREFIX).unwrap()).unwrap();
        raw[index] ^= 0x01;
        format!("{CURRENT_PREFIX}{}", STANDARD.encode(raw))
    }

    #[test]
    fn tampering_with_any_part_of_a_stored_token_is_detected() {
        let stored = encrypt_with("a-secret", "token", TEST_COST).unwrap();
        let length = STANDARD.decode(stored.strip_prefix(CURRENT_PREFIX).unwrap()).unwrap().len();
        // the cost bytes, the salt, the nonce, the ciphertext and the tag
        for index in [4, 5, 6, HEADER_LEN - 1, HEADER_LEN, HEADER_LEN + NONCE_LEN, length - 1] {
            assert!(decrypt_current("a-secret", &flip_byte(&stored, index)).is_none(), "byte {index}");
        }
    }

    #[test]
    fn a_header_asking_for_a_huge_key_derivation_is_refused_without_trying_it() {
        let stored = encrypt_with("a-secret", "token", TEST_COST).unwrap();
        let mut raw = STANDARD.decode(stored.strip_prefix(CURRENT_PREFIX).unwrap()).unwrap();
        raw[..4].copy_from_slice(&u32::MAX.to_be_bytes());
        let hostile = format!("{CURRENT_PREFIX}{}", STANDARD.encode(raw));
        assert!(decrypt_current("a-secret", &hostile).is_none());
    }

    #[test]
    fn garbage_and_truncated_values_are_rejected() {
        for bad in ["v2:", "v2:AAAA", "v2:not base64 at all!!", "v2:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"] {
            assert!(decrypt_current("a-secret", bad).is_none(), "{bad}");
        }
    }

    // Produced by the previous implementation (magic-crypt 5.0.1, `new_magic_crypt!(key, 256)`).
    const LEGACY_VECTORS: &[(&str, &str, &str)] = &[
        // (secret, plaintext, stored)
        ("test-secret-key-for-roundtrip", "some-fake-audiobookshelf-token-abc123", "3YbeZ71POyHB3dMzvWTjA33lg0ElUbFB2maW2IRjyY/U9ygBQDypzMBMIKuRJz1x"),
        ("test-secret-key-for-roundtrip", "", "JcyfOOEmbyvSrOMnW9+MTQ=="),
        ("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff", "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1c2VySWQiOiJhYmMiLCJ0eXBlIjoiYWNjZXNzIiwiZXhwIjoxODAwMDAwMDAwfQ.c2lnbmF0dXJlLW5vdC1yZWFs", "4pINuLrhj5QVBZX/XJ0HXHFK6icG+56VoZy8dV0NmCoWjHKEQDd0TXxfE/rnpw5cWCM+3t5hHDij/9xdiv2ZysgneqZh6HrTk07uiAX7WXpYW+1RINVdFt2/zfxZ12sq8maZ7/2U9SzJrgSgDVo7aXAffjerTFk7eCw/pSE72ZhKEitRHuswSGrUGy/dY0tw"),
        ("secret", "rt-refresh-token-0123456789", "FJ0DbOC0cO5avFwEt0q6AE+hMFWlSq2qmzWNLacjixw="),
    ];

    #[test]
    fn values_saved_by_the_previous_format_still_decrypt() {
        for (secret, plaintext, stored) in LEGACY_VECTORS {
            assert!(is_legacy_format(stored));
            assert_eq!(&decrypt_legacy(secret, stored).unwrap(), plaintext, "{plaintext}");
            assert!(decrypt_legacy("not-the-secret", stored).is_none() || plaintext.is_empty(), "{plaintext}");
        }
    }

    #[test]
    fn only_old_looking_values_count_as_legacy() {
        assert!(!is_legacy_format(""));
        assert!(!is_legacy_format("v2:AAAA"));
        assert!(is_legacy_format("p0ZmVuz9a4Xo3sKj1w=="));
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        // Audited 2026-07-20: this is the only test in the codebase that touches an
        // environment variable at all (checked every #[test] and every env::var/
        // set_var/remove_var call site) - cargo test's default concurrent test
        // threads have nothing else to race with here.
        unsafe { env::set_var("ABSOTUI_SECRET_KEY", "test-secret-key-for-roundtrip") };
        let original = "some-fake-audiobookshelf-token-abc123";

        let encrypted = encrypt_token(original).expect("encryption should succeed");
        assert_ne!(encrypted, original);
        assert!(encrypted.starts_with(CURRENT_PREFIX));

        let decrypted = decrypt_token(&encrypted).expect("decryption should succeed");
        assert_eq!(decrypted, original);

        // A value saved by the previous format goes through the same public entry point.
        for (secret, plaintext, stored) in LEGACY_VECTORS {
            if *secret == "test-secret-key-for-roundtrip" {
                assert_eq!(&decrypt_token(stored).expect("legacy decryption should succeed"), plaintext);
            }
        }
    }
}
