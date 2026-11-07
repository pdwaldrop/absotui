use magic_crypt::{new_magic_crypt, MagicCryptTrait};
use std::env;
use log::error;

// `.env` is read once at startup, hence the restart.
const MISSING_SECRET_MESSAGE: &str = "No secret found in .env. Do this:\n\
mkdir -p ~/.config/absotui\n\
KEY=$(od -An -tx1 -N32 /dev/urandom | tr -d ' \\n')\n\
echo \"ABSOTUI_SECRET_KEY=$KEY\" >> ~/.config/absotui/.env\n\
then restart Absotui.";

pub fn encrypt_token(token_to_encrypt: &str) -> Result<String, String> {


    // Load .env variables (`env::var` will read ~.config/absotui/.env)
    // check `main.rs` to see the init process for dotenv
    // Retrieve secret key from .env
    if let Ok(key) = env::var("ABSOTUI_SECRET_KEY") {

        // Create magic crypt object
        let mc = new_magic_crypt!(key, 256);

        // Token encryption
        let encrypted_token = mc.encrypt_str_to_base64(token_to_encrypt);

        Ok(encrypted_token)
    } else {
        error!("{MISSING_SECRET_MESSAGE}");
        Err(MISSING_SECRET_MESSAGE.to_string())
    }
}


pub fn decrypt_token(encrypted_token: &str) -> Result<String, String> {
    // Load .env variables (`env::var` will read ~.config/absotui/.env)
    // check `main.rs` to see the init process for dotenv
    // Retrieve secret key from .env
    

    if let Ok(key) = env::var("ABSOTUI_SECRET_KEY") {
        // Create magic crypt object
        let mc = new_magic_crypt!(key, 256);

        // Token decryption
        if let Ok(decrypted_token) = mc.decrypt_base64_to_string(encrypted_token) { Ok(decrypted_token) } else {
            error!("Failed to decrypt the token.");
            Err("Failed to decrypt the token.".to_string())
        }
    } else {
        error!("{MISSING_SECRET_MESSAGE}");
        Err(MISSING_SECRET_MESSAGE.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn encrypt_decrypt_roundtrip() {
        // Audited 2026-07-20: this is the only test in the codebase that touches an
        // environment variable at all (checked every #[test] and every env::var/
        // set_var/remove_var call site) - cargo test's default concurrent test
        // threads have nothing else to race with here.
        unsafe { env::set_var("ABSOTUI_SECRET_KEY", "test-secret-key-for-roundtrip") };
        let original = "some-fake-audiobookshelf-token-abc123";

        let encrypted = encrypt_token(original).expect("encryption should succeed");
        assert_ne!(encrypted, original);

        let decrypted = decrypt_token(&encrypted).expect("decryption should succeed");
        assert_eq!(decrypted, original);
    }
}
