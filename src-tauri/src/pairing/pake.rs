//! PIN pairing over SPAKE2 (a password-authenticated key exchange).
//!
//! The 6-digit PIN never leaves either device. Each pairing run lets a party who does not know
//! the PIN test exactly one guess, and a passive observer learns nothing that can be brute-forced
//! offline. A successful run leaves both devices with a 32-byte pairing key, which later
//! authenticates transfer handshakes and salts the session key.
//!
//! Flow (client = the device typing the PIN, server = the device showing it):
//! 1. client: `client_start(pin)` -> sends `msg_a`
//! 2. server: `server_respond(pin, msg_a)` -> returns `msg_b` and `confirm_b`
//! 3. client: `client_finish(..)` checks `confirm_b` (wrong PIN fails here), sends `confirm_a`
//! 4. server: `server_verify(..)` checks `confirm_a` and stores the pairing key

use spake2::{Ed25519Group, Identity, Password, Spake2};

pub const PAIRING_KEY_SIZE: usize = 32;

const ID_CLIENT: &[u8] = b"easy-share/pair/client";
const ID_SERVER: &[u8] = b"easy-share/pair/server";
const LABEL_CONFIRM_A: &[u8] = b"EASY_SHARE_PAKE_CONFIRM_A:";
const LABEL_CONFIRM_B: &[u8] = b"EASY_SHARE_PAKE_CONFIRM_B:";
const LABEL_PAIRING_KEY: &[u8] = b"EASY_SHARE_PAIRING_KEY:";
const LABEL_REQUEST_AUTH: &[u8] = b"EASY_SHARE_HANDSHAKE_REQUEST:";
const LABEL_RESPONSE_AUTH: &[u8] = b"EASY_SHARE_HANDSHAKE_RESPONSE:";

/// Long-lived key shared by two devices after a successful PIN pairing.
#[derive(Clone, PartialEq, Eq)]
pub struct PairingKey([u8; PAIRING_KEY_SIZE]);

impl PairingKey {
    pub fn as_bytes(&self) -> &[u8; PAIRING_KEY_SIZE] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(hex_str: &str) -> Option<Self> {
        hex::decode(hex_str).ok()?.try_into().ok().map(PairingKey)
    }
}

impl std::fmt::Debug for PairingKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PairingKey([REDACTED])")
    }
}

/// Client-side state kept between sending `msg_a` and receiving the server's reply.
pub struct ClientPake {
    spake: Spake2<Ed25519Group>,
    msg_a: Vec<u8>,
}

/// Server-side state kept between replying to `/pair/start` and receiving `/pair/finish`.
#[derive(Clone)]
pub struct ServerPending {
    expected_confirm_a: blake3::Hash,
    pairing_key: PairingKey,
}

impl std::fmt::Debug for ServerPending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ServerPending([REDACTED])")
    }
}

fn tag(key: &[u8; 32], label: &[u8], parts: &[&[u8]]) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new_keyed(key);
    hasher.update(label);
    for part in parts {
        // Length-prefix every field so different splits of the same bytes never collide
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize()
}

fn spake_secret(raw: Vec<u8>) -> Result<[u8; 32], String> {
    raw.try_into()
        .map_err(|_| "Unexpected SPAKE2 key length".to_string())
}

fn parse_tag(hex_str: &str) -> Option<blake3::Hash> {
    blake3::Hash::from_hex(hex_str).ok()
}

/// Starts a pairing run on the device where the user typed the PIN.
pub fn client_start(pin: &str) -> (ClientPake, Vec<u8>) {
    let (spake, msg_a) = Spake2::<Ed25519Group>::start_a(
        &Password::new(pin.as_bytes()),
        &Identity::new(ID_CLIENT),
        &Identity::new(ID_SERVER),
    );
    (
        ClientPake {
            spake,
            msg_a: msg_a.clone(),
        },
        msg_a,
    )
}

/// Answers a pairing run on the device that shows the PIN.
/// Returns the pending state, `msg_b`, and the hex `confirm_b` tag.
pub fn server_respond(pin: &str, msg_a: &[u8]) -> Result<(ServerPending, Vec<u8>, String), String> {
    let (spake, msg_b) = Spake2::<Ed25519Group>::start_b(
        &Password::new(pin.as_bytes()),
        &Identity::new(ID_CLIENT),
        &Identity::new(ID_SERVER),
    );
    let secret = spake_secret(
        spake
            .finish(msg_a)
            .map_err(|e| format!("Invalid pairing message: {:?}", e))?,
    )?;

    let transcript: [&[u8]; 2] = [msg_a, &msg_b];
    let confirm_b = tag(&secret, LABEL_CONFIRM_B, &transcript).to_hex().to_string();
    let pending = ServerPending {
        expected_confirm_a: tag(&secret, LABEL_CONFIRM_A, &transcript),
        pairing_key: PairingKey(*tag(&secret, LABEL_PAIRING_KEY, &transcript).as_bytes()),
    };
    Ok((pending, msg_b, confirm_b))
}

/// Completes the client side. Fails when the server used a different PIN.
/// Returns the pairing key and the hex `confirm_a` tag to send back.
pub fn client_finish(
    state: ClientPake,
    msg_b: &[u8],
    confirm_b_hex: &str,
) -> Result<(PairingKey, String), String> {
    let ClientPake { spake, msg_a } = state;
    let secret = spake_secret(
        spake
            .finish(msg_b)
            .map_err(|e| format!("Invalid pairing message: {:?}", e))?,
    )?;

    let transcript: [&[u8]; 2] = [&msg_a, msg_b];
    let expected_b = tag(&secret, LABEL_CONFIRM_B, &transcript);
    // blake3::Hash equality is constant-time
    if parse_tag(confirm_b_hex) != Some(expected_b) {
        return Err("Pairing PIN does not match".to_string());
    }

    let confirm_a = tag(&secret, LABEL_CONFIRM_A, &transcript).to_hex().to_string();
    let key = PairingKey(*tag(&secret, LABEL_PAIRING_KEY, &transcript).as_bytes());
    Ok((key, confirm_a))
}

/// Completes the server side once the client proves it derived the same key.
pub fn server_verify(pending: &ServerPending, confirm_a_hex: &str) -> Result<PairingKey, String> {
    if parse_tag(confirm_a_hex) == Some(pending.expected_confirm_a) {
        Ok(pending.pairing_key.clone())
    } else {
        Err("Pairing confirmation failed".to_string())
    }
}

/// Authenticates the sender's handshake: binds the request id to its ephemeral public key.
pub fn request_auth(key: &PairingKey, request_id: &str, sender_public_hex: &str) -> String {
    tag(
        key.as_bytes(),
        LABEL_REQUEST_AUTH,
        &[request_id.as_bytes(), sender_public_hex.as_bytes()],
    )
    .to_hex()
    .to_string()
}

/// Checks a sender's handshake tag in constant time.
pub fn verify_request_auth(
    key: &PairingKey,
    request_id: &str,
    sender_public_hex: &str,
    auth_hex: &str,
) -> bool {
    parse_tag(auth_hex)
        == Some(tag(
            key.as_bytes(),
            LABEL_REQUEST_AUTH,
            &[request_id.as_bytes(), sender_public_hex.as_bytes()],
        ))
}

/// Authenticates the receiver's handshake reply: binds the session token to its ephemeral key.
pub fn response_auth(
    key: &PairingKey,
    request_id: &str,
    session_token: &str,
    receiver_public_hex: &str,
) -> String {
    tag(
        key.as_bytes(),
        LABEL_RESPONSE_AUTH,
        &[
            request_id.as_bytes(),
            session_token.as_bytes(),
            receiver_public_hex.as_bytes(),
        ],
    )
    .to_hex()
    .to_string()
}

/// Checks a receiver's handshake reply tag in constant time.
pub fn verify_response_auth(
    key: &PairingKey,
    request_id: &str,
    session_token: &str,
    receiver_public_hex: &str,
    auth_hex: &str,
) -> bool {
    parse_tag(auth_hex)
        == Some(tag(
            key.as_bytes(),
            LABEL_RESPONSE_AUTH,
            &[
                request_id.as_bytes(),
                session_token.as_bytes(),
                receiver_public_hex.as_bytes(),
            ],
        ))
}
