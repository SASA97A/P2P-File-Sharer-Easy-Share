use easy_share_lib::crypto::{
    decrypt_chunk, derive_session_key, encrypt_chunk, generate_ephemeral_keypair,
    public_key_to_hex, public_key_from_hex,
};

#[test]
fn test_ephemeral_key_hex_serialization() {
    let (_, public_key) = generate_ephemeral_keypair();
    let hex_str = public_key_to_hex(&public_key);
    assert_eq!(hex_str.len(), 64); // 32 bytes in hex = 64 characters

    let parsed = public_key_from_hex(&hex_str).expect("should parse valid hex public key");
    assert_eq!(public_key.as_bytes(), parsed.as_bytes());

    assert!(public_key_from_hex("invalid-hex").is_err());
    assert!(public_key_from_hex("001122").is_err()); // Too short
}

#[test]
fn test_key_exchange_and_chunk_encryption_roundtrip() {
    let pin = "491823";
    let session_token = "session-test-token-uuid-123";

    let (sender_secret, sender_pub) = generate_ephemeral_keypair();
    let (receiver_secret, receiver_pub) = generate_ephemeral_keypair();

    let sender_key = derive_session_key(sender_secret, &receiver_pub, pin, session_token);
    let receiver_key = derive_session_key(receiver_secret, &sender_pub, pin, session_token);

    assert_eq!(sender_key.as_bytes(), receiver_key.as_bytes());

    let original_data = b"Hello, secure P2P Easy Share encrypted world!";
    let offset = 0u64;

    let encrypted = encrypt_chunk(&sender_key, "file-1", offset, original_data)
        .expect("encryption should succeed");

    // Ciphertext must differ from plaintext and include 16-byte Poly1305 tag
    assert_ne!(encrypted.as_slice(), original_data.as_slice());
    assert_eq!(encrypted.len(), original_data.len() + 16);

    let decrypted = decrypt_chunk(&receiver_key, "file-1", offset, &encrypted)
        .expect("decryption should succeed");

    assert_eq!(decrypted, original_data);
}

#[test]
fn test_wrong_pin_fails_decryption() {
    let correct_pin = "112233";
    let wrong_pin = "998877";
    let session_token = "sess-token-456";

    let (sender_secret, sender_pub) = generate_ephemeral_keypair();
    let (receiver_secret, receiver_pub) = generate_ephemeral_keypair();

    let sender_key = derive_session_key(sender_secret, &receiver_pub, correct_pin, session_token);
    let receiver_key = derive_session_key(receiver_secret, &sender_pub, wrong_pin, session_token);

    let original_data = b"Sensitive payload protected by PIN";
    let encrypted = encrypt_chunk(&sender_key, "file-1", 0, original_data).unwrap();

    let res = decrypt_chunk(&receiver_key, "file-1", 0, &encrypted);
    assert!(res.is_err(), "decryption with wrong PIN must fail authentication");
}

#[test]
fn test_tampered_ciphertext_fails_decryption() {
    let pin = "554433";
    let session_token = "sess-tamper-check";

    let (sender_secret, sender_pub) = generate_ephemeral_keypair();
    let (receiver_secret, receiver_pub) = generate_ephemeral_keypair();

    let sender_key = derive_session_key(sender_secret, &receiver_pub, pin, session_token);
    let receiver_key = derive_session_key(receiver_secret, &sender_pub, pin, session_token);

    let original_data = b"Important tamper-evident chunk";
    let mut encrypted = encrypt_chunk(&sender_key, "file-1", 5, original_data).unwrap();

    // Flip a bit in the ciphertext
    encrypted[0] ^= 0x01;

    let res = decrypt_chunk(&receiver_key, "file-1", 5, &encrypted);
    assert!(res.is_err(), "tampered ciphertext must fail authentication");
}

#[test]
fn test_chunk_index_replay_or_reorder_fails_decryption() {
    let pin = "123789";
    let session_token = "sess-replay-check";

    let (sender_secret, sender_pub) = generate_ephemeral_keypair();
    let (receiver_secret, receiver_pub) = generate_ephemeral_keypair();

    let sender_key = derive_session_key(sender_secret, &receiver_pub, pin, session_token);
    let receiver_key = derive_session_key(receiver_secret, &sender_pub, pin, session_token);

    let original_data = b"Sequential chunk payload";
    // Encrypted at offset 0
    let encrypted = encrypt_chunk(&sender_key, "file-1", 0, original_data).unwrap();

    // Attempting to decrypt at a different offset must fail due to nonce mismatch
    let res = decrypt_chunk(&receiver_key, "file-1", 4096, &encrypted);
    assert!(res.is_err(), "chunk offset mismatch must fail decryption");

    // Moving a chunk into another file of the same session must fail too
    let res = decrypt_chunk(&receiver_key, "file-2", 0, &encrypted);
    assert!(res.is_err(), "chunk moved to another file must fail decryption");
}

#[test]
fn test_same_offset_in_different_files_uses_different_keystream() {
    let pin = "246810";
    let session_token = "sess-multi-file";

    let (sender_secret, _) = generate_ephemeral_keypair();
    let (_, receiver_pub) = generate_ephemeral_keypair();
    let key = derive_session_key(sender_secret, &receiver_pub, pin, session_token);

    // Identical plaintext at offset 0 of two files must not produce identical ciphertext,
    // otherwise the session key and nonce are being reused across files.
    let data = [0u8; 64];
    let a = encrypt_chunk(&key, "file-a", 0, &data).unwrap();
    let b = encrypt_chunk(&key, "file-b", 0, &data).unwrap();
    assert_ne!(a, b);

    // Unaligned resumed chunks at distinct offsets must not reuse a nonce either
    let c = encrypt_chunk(&key, "file-a", 3, &data).unwrap();
    assert_ne!(a, c);
}
