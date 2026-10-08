use easy_share_lib::models::peer::{DeviceInfo, PeerInfo};
use easy_share_lib::models::transfer::{
    ChunkHeader, FileMetadata, FinishRequest, FinishResponse, TransferRequest, TransferResponse,
    TransferStatus, TransferStatusQueryResponse,
};
use std::collections::HashMap;

#[test]
fn test_peer_info_serialization_roundtrip() {
    let peer = PeerInfo {
        device_name: "Alice-PC".to_string(),
        device_type: "desktop".to_string(),
        os: "windows".to_string(),
        room_id: Some("Engineering".to_string()),
        ip: "192.168.1.50".to_string(),
        port: 5050,
        pairing_pin: None,
    };

    let json_str = serde_json::to_string(&peer).expect("failed to serialize PeerInfo");
    assert!(json_str.contains("\"device_name\":\"Alice-PC\""));
    assert!(json_str.contains("\"room_id\":\"Engineering\""));
    assert!(json_str.contains("\"port\":5050"));
    assert!(!json_str.contains("pairing_pin"));

    let deserialized: PeerInfo =
        serde_json::from_str(&json_str).expect("failed to deserialize PeerInfo");
    assert_eq!(peer, deserialized);

    // Test with room_id = None
    let peer_no_room = PeerInfo {
        device_name: "Bob-Phone".to_string(),
        device_type: "mobile".to_string(),
        os: "android".to_string(),
        room_id: None,
        ip: "192.168.1.51".to_string(),
        port: 5050,
        pairing_pin: None,
    };
    let json_no_room =
        serde_json::to_string(&peer_no_room).expect("failed to serialize PeerInfo without room");
    assert_eq!(peer_no_room, serde_json::from_str::<PeerInfo>(&json_no_room).unwrap());

    // Test with pairing_pin
    let peer_with_pin = PeerInfo::new(
        "Charlie-Laptop",
        "desktop",
        "linux",
        None,
        "192.168.1.52",
        5050,
    )
    .with_pairing_pin("123456");

    let json_pin = serde_json::to_string(&peer_with_pin).expect("failed to serialize PeerInfo with PIN");
    assert!(json_pin.contains("\"pairing_pin\":\"123456\""));
    let deserialized_pin: PeerInfo =
        serde_json::from_str(&json_pin).expect("failed to deserialize PeerInfo with PIN");
    assert_eq!(peer_with_pin, deserialized_pin);
    assert_eq!(deserialized_pin.pairing_pin, Some("123456".to_string()));
}

#[test]
fn test_device_info_serialization_roundtrip() {
    let device = DeviceInfo {
        device_name: "Alice-PC".to_string(),
        device_type: "desktop".to_string(),
        os: "windows".to_string(),
        version: "2.0.0".to_string(),
        room_id: Some("Engineering".to_string()),
        port: 5050,
        pairing_pin: None,
    };

    let json_str = serde_json::to_string(&device).expect("failed to serialize DeviceInfo");
    assert!(json_str.contains("\"version\":\"2.0.0\""));
    assert!(!json_str.contains("pairing_pin"));
    let deserialized: DeviceInfo =
        serde_json::from_str(&json_str).expect("failed to deserialize DeviceInfo");
    assert_eq!(device, deserialized);

    // Test DeviceInfo with pairing_pin
    let device_with_pin = DeviceInfo::new(
        "Alice-PC",
        "desktop",
        "windows",
        "2.0.0",
        Some("Engineering".to_string()),
        5050,
    )
    .with_pairing_pin("654321");

    let json_device_pin =
        serde_json::to_string(&device_with_pin).expect("failed to serialize DeviceInfo with PIN");
    assert!(json_device_pin.contains("\"pairing_pin\":\"654321\""));
    let deserialized_device_pin: DeviceInfo =
        serde_json::from_str(&json_device_pin).expect("failed to deserialize DeviceInfo with PIN");
    assert_eq!(device_with_pin, deserialized_device_pin);
    assert_eq!(deserialized_device_pin.pairing_pin, Some("654321".to_string()));
}

#[test]
fn test_transfer_request_with_multiple_files_roundtrip() {
    let files = vec![
        FileMetadata {
            file_id: "f1-abc".to_string(),
            name: "archive.zip".to_string(),
            size: 1073741824,
            blake3_hash: Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string()),
        },
        FileMetadata {
            file_id: "f2-def".to_string(),
            name: "image.png".to_string(),
            size: 2048576,
            blake3_hash: None,
        },
    ];

    let request = TransferRequest {
        request_id: "req-12345".to_string(),
        sender_name: "Alice-PC".to_string(),
        sender_os: "windows".to_string(),
        room_id: Some("Engineering".to_string()),
        total_bytes: 1075790400,
        files,
    };

    let json_str = serde_json::to_string(&request).expect("failed to serialize TransferRequest");
    assert!(json_str.contains("\"request_id\":\"req-12345\""));
    assert!(json_str.contains("\"archive.zip\""));
    assert!(json_str.contains("\"image.png\""));

    let deserialized: TransferRequest =
        serde_json::from_str(&json_str).expect("failed to deserialize TransferRequest");
    assert_eq!(request, deserialized);
}

#[test]
fn test_transfer_response_accepted_and_declined() {
    let mut offsets = HashMap::new();
    offsets.insert("f1-abc".to_string(), 524288000);

    let accepted = TransferResponse::accepted("sess-99887766", offsets.clone());
    let json_accepted =
        serde_json::to_string(&accepted).expect("failed to serialize accepted TransferResponse");
    assert!(json_accepted.contains("\"status\":\"accepted\""));
    assert!(json_accepted.contains("\"session_token\":\"sess-99887766\""));

    let des_accepted: TransferResponse =
        serde_json::from_str(&json_accepted).expect("failed to deserialize accepted TransferResponse");
    assert_eq!(des_accepted.status, TransferStatus::Accepted);
    assert_eq!(des_accepted.session_token.as_deref(), Some("sess-99887766"));
    assert_eq!(des_accepted.existing_offsets.get("f1-abc"), Some(&524288000));

    let declined = TransferResponse::declined("User rejected the transfer request.");
    let json_declined =
        serde_json::to_string(&declined).expect("failed to serialize declined TransferResponse");
    assert!(json_declined.contains("\"status\":\"declined\""));
    assert!(json_declined.contains("\"reason\":\"User rejected the transfer request.\""));

    let des_declined: TransferResponse =
        serde_json::from_str(&json_declined).expect("failed to deserialize declined TransferResponse");
    assert_eq!(des_declined.status, TransferStatus::Declined);
    assert_eq!(
        des_declined.reason.as_deref(),
        Some("User rejected the transfer request.")
    );
}

#[test]
fn test_finish_request_and_response_roundtrip() {
    let finish_req = FinishRequest {
        session_token: "sess-99887766-5544-3322".to_string(),
        file_id: "f1-abc".to_string(),
    };

    let json_req = serde_json::to_string(&finish_req).expect("failed to serialize FinishRequest");
    let des_req: FinishRequest =
        serde_json::from_str(&json_req).expect("failed to deserialize FinishRequest");
    assert_eq!(finish_req, des_req);

    let finish_res = FinishResponse {
        status: TransferStatus::Completed,
        file_id: "f1-abc".to_string(),
        saved_path: "C:\\Users\\User\\Downloads\\archive.zip".to_string(),
    };

    let json_res = serde_json::to_string(&finish_res).expect("failed to serialize FinishResponse");
    assert!(json_res.contains("\"status\":\"completed\""));
    let des_res: FinishResponse =
        serde_json::from_str(&json_res).expect("failed to deserialize FinishResponse");
    assert_eq!(finish_res, des_res);
}

#[test]
fn test_transfer_status_query_response() {
    let status_resp = TransferStatusQueryResponse {
        file_id: "f1-abc".to_string(),
        bytes_received: 524288000,
        status: TransferStatus::Partial,
    };

    let json_str = serde_json::to_string(&status_resp)
        .expect("failed to serialize TransferStatusQueryResponse");
    assert!(json_str.contains("\"status\":\"partial\""));
    let des_resp: TransferStatusQueryResponse =
        serde_json::from_str(&json_str).expect("failed to deserialize TransferStatusQueryResponse");
    assert_eq!(status_resp, des_resp);
}

#[test]
fn test_chunk_header_roundtrip() {
    let chunk = ChunkHeader {
        session_token: "sess-12345".to_string(),
        file_id: "f1-abc".to_string(),
        offset: 1048576,
    };

    let json_str = serde_json::to_string(&chunk).expect("failed to serialize ChunkHeader");
    let des_chunk: ChunkHeader =
        serde_json::from_str(&json_str).expect("failed to deserialize ChunkHeader");
    assert_eq!(chunk, des_chunk);
}
