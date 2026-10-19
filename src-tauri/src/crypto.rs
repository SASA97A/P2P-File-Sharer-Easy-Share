use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use hkdf::Hkdf;
use sha2::Sha256;
pub use x25519_dalek::{EphemeralSecret, PublicKey};

pub const KEY_SIZE: usize = 32;
pub const NONCE_SIZE: usize = 12;

/// Authenticated symmetric encryption session key.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionKey([u8; KEY_SIZE]);

impl SessionKey {
    pub fn new(bytes: [u8; KEY_SIZE]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; KEY_SIZE] {
        &self.0
    }
}

impl std::fmt::Debug for SessionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SessionKey([REDACTED])")
    }
}

/// Generates a new ephemeral X25519 keypair for key exchange.
pub fn generate_ephemeral_keypair() -> (EphemeralSecret, PublicKey) {
    let mut rng = rand::rngs::OsRng;
    let secret = EphemeralSecret::random_from_rng(&mut rng);
    let public = PublicKey::from(&secret);
    (secret, public)
}

/// Serializes an X25519 public key to a lowercase hex string.
pub fn public_key_to_hex(key: &PublicKey) -> String {
    hex::encode(key.as_bytes())
}

/// Parses an X25519 public key from a hex string.
pub fn public_key_from_hex(hex_str: &str) -> Result<PublicKey, String> {
    let bytes = hex::decode(hex_str).map_err(|e| format!("Invalid hex: {}", e))?;
    if bytes.len() != 32 {
        return Err(format!(
            "Expected 32 bytes for public key, got {}",
            bytes.len()
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(PublicKey::from(arr))
}

/// Derives a 32-byte authenticated session key from an ephemeral Diffie-Hellman shared secret,
/// salted with the pairing PIN and expanded with the session token.
pub fn derive_session_key(
    our_secret: EphemeralSecret,
    peer_public: &PublicKey,
    pin: &str,
    session_token: &str,
) -> SessionKey {
    let shared_secret = our_secret.diffie_hellman(peer_public);

    // HKDF-Extract using the PIN as salt and Diffie-Hellman output as IKM
    let hk = Hkdf::<Sha256>::new(Some(pin.as_bytes()), shared_secret.as_bytes());

    let mut key_bytes = [0u8; KEY_SIZE];
    // HKDF-Expand using session_token as info context
    hk.expand(session_token.as_bytes(), &mut key_bytes)
        .expect("32 bytes is valid length for HKDF-SHA256 output");

    SessionKey::new(key_bytes)
}

/// Derives a deterministic 12-byte nonce for a specific chunk index in the transfer session.
fn derive_chunk_nonce(chunk_index: u64) -> Nonce {
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    // 4-byte domain separator prefix
    nonce_bytes[0..4].copy_from_slice(b"EASY");
    // 8-byte big-endian chunk counter
    nonce_bytes[4..12].copy_from_slice(&chunk_index.to_be_bytes());
    *Nonce::from_slice(&nonce_bytes)
}

/// Encrypts a chunk payload with ChaCha20-Poly1305 using the session key and chunk index.
pub fn encrypt_chunk(
    key: &SessionKey,
    chunk_index: u64,
    plaintext: &[u8],
) -> Result<Vec<u8>, String> {
    let cipher = ChaCha20Poly1305::new_from_slice(key.as_bytes())
        .map_err(|e| format!("Cipher init error: {}", e))?;
    let nonce = derive_chunk_nonce(chunk_index);

    cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| format!("Encryption error: {}", e))
}

/// Decrypts a chunk payload with ChaCha20-Poly1305 and verifies its Poly1305 MAC tag.
pub fn decrypt_chunk(
    key: &SessionKey,
    chunk_index: u64,
    ciphertext: &[u8],
) -> Result<Vec<u8>, String> {
    let cipher = ChaCha20Poly1305::new_from_slice(key.as_bytes())
        .map_err(|e| format!("Cipher init error: {}", e))?;
    let nonce = derive_chunk_nonce(chunk_index);

    cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|e| format!("Decryption authentication failed: {}", e))
}
