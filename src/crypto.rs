use ring::aead::{Aad, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey, AES_256_GCM};
use ring::error::Unspecified;
use crate::error::{AppError, Result};
use base64::{Engine as _, engine::general_purpose};

struct CounterNonceSequence(u32);

impl NonceSequence for CounterNonceSequence {
    fn advance(&mut self) -> std::result::Result<Nonce, Unspecified> {
        let mut nonce_bytes = vec![0u8; 12];
        let bytes = self.0.to_be_bytes();
        nonce_bytes[8..].copy_from_slice(&bytes);
        self.0 = self.0.wrapping_add(1);
        Nonce::try_assume_unique_for_key(&nonce_bytes)
    }
}

pub fn encrypt(plaintext: &str) -> Result<String> {
    let key = get_encryption_key()?;
    let unbound_key = UnboundKey::new(&AES_256_GCM, &key)
        .map_err(|_| AppError::Crypto("Failed to create key".into()))?;
    let nonce_sequence = CounterNonceSequence(0);
    let mut sealing_key = SealingKey::new(unbound_key, nonce_sequence);
    
    let mut in_out = plaintext.as_bytes().to_vec();
    sealing_key.seal_in_place_append_tag(Aad::empty(), &mut in_out)
        .map_err(|_| AppError::Crypto("Encryption failed".into()))?;
    
    Ok(general_purpose::STANDARD.encode(&in_out))
}

pub fn decrypt(ciphertext: &str) -> Result<String> {
    let key = get_encryption_key()?;
    let unbound_key = UnboundKey::new(&AES_256_GCM, &key)
        .map_err(|_| AppError::Crypto("Failed to create key".into()))?;
    let nonce_sequence = CounterNonceSequence(0);
    let mut opening_key = OpeningKey::new(unbound_key, nonce_sequence);
    
    let mut in_out = general_purpose::STANDARD.decode(ciphertext)
        .map_err(|_| AppError::Crypto("Invalid base64".into()))?;
    
    let plaintext = opening_key.open_in_place(Aad::empty(), &mut in_out)
        .map_err(|_| AppError::Crypto("Decryption failed".into()))?;
    
    String::from_utf8(plaintext.to_vec())
        .map_err(|_| AppError::Crypto("Invalid UTF-8".into()))
}

pub fn hash_password(password: &str) -> Result<String> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|_| AppError::Crypto("Password hashing failed".into()))
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    bcrypt::verify(password, hash)
        .map_err(|_| AppError::Crypto("Password verification failed".into()))
}

pub fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        return "****".to_string();
    }
    let prefix = &token[..4];
    let suffix = &token[token.len()-4..];
    format!("{}****{}", prefix, suffix)
}

fn get_encryption_key() -> Result<Vec<u8>> {
    let key_str = std::env::var("TOKEN_ENCRYPTION_KEY")
        .map_err(|_| AppError::Crypto("TOKEN_ENCRYPTION_KEY not set".into()))?;
    
    general_purpose::STANDARD.decode(&key_str)
        .map_err(|_| AppError::Crypto("Invalid encryption key".into()))
}
