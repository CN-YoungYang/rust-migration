use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};
use crate::error::{AppError, Result};
use base64::{Engine as _, engine::general_purpose};
use std::sync::OnceLock;

static ENCRYPTION_KEY: OnceLock<Vec<u8>> = OnceLock::new();

fn get_encryption_key() -> Result<&'static Vec<u8>> {
    ENCRYPTION_KEY.get_or_init(|| {
        let key_str = std::env::var("TOKEN_ENCRYPTION_KEY")
            .expect("TOKEN_ENCRYPTION_KEY not set");
        let key = general_purpose::STANDARD.decode(&key_str)
            .expect("Invalid TOKEN_ENCRYPTION_KEY: not valid base64");
        assert_eq!(key.len(), 32, "TOKEN_ENCRYPTION_KEY must decode to 32 bytes");
        key
    });
    // get_or_init always succeeds here; errors above panic with clear messages
    Ok(ENCRYPTION_KEY.get().unwrap())
}

pub fn encrypt(plaintext: &str) -> Result<String> {
    let key = get_encryption_key()?;
    let unbound_key = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|_| AppError::Crypto("Failed to create key".into()))?;
    let sealing_key = LessSafeKey::new(unbound_key);

    let rng = SystemRandom::new();
    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes)
        .map_err(|_| AppError::Crypto("Failed to generate nonce".into()))?;
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    let mut in_out = plaintext.as_bytes().to_vec();
    sealing_key.seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)
        .map_err(|_| AppError::Crypto("Encryption failed".into()))?;

    let mut output = nonce_bytes.to_vec();
    output.extend_from_slice(&in_out);
    Ok(general_purpose::STANDARD.encode(output))
}

pub fn decrypt(ciphertext: &str) -> Result<String> {
    let key = get_encryption_key()?;
    let unbound_key = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|_| AppError::Crypto("Failed to create key".into()))?;
    let opening_key = LessSafeKey::new(unbound_key);

    let decoded = general_purpose::STANDARD.decode(ciphertext)
        .map_err(|_| AppError::Crypto("Invalid base64".into()))?;
    if decoded.len() < 12 + AES_256_GCM.tag_len() {
        return Err(AppError::Crypto("Invalid ciphertext".into()));
    }

    let nonce_bytes: [u8; 12] = decoded[..12]
        .try_into()
        .map_err(|_| AppError::Crypto("Invalid nonce".into()))?;
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let mut in_out = decoded[12..].to_vec();

    let plaintext = opening_key.open_in_place(nonce, Aad::empty(), &mut in_out)
        .map_err(|_| AppError::Crypto("Decryption failed".into()))?;

    String::from_utf8(plaintext.to_vec())
        .map_err(|_| AppError::Crypto("Invalid UTF-8".into()))
}

pub fn hash_password(password: &str) -> Result<String> {
    bcrypt::hash(password, 10)
        .map_err(|_| AppError::Crypto("Password hashing failed".into()))
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    bcrypt::verify(password, hash)
        .map_err(|_| AppError::Crypto("Password verification failed".into()))
}

pub fn decrypt_secret(encrypted: &str) -> Result<String> {
    decrypt(encrypted)
}
