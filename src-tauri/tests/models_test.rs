use easy_share_lib::models::peer::{DeviceInfo, PeerInfo};
use easy_share_lib::models::transfer::{
    CancelRequest, ChunkHeader, ChunkResponse, FileMetadata, FinishRequest, FinishResponse,
    TransferRequest, TransferResponse, TransferStatus, TransferStatusQueryResponse,
};
use std::collections::HashMap;

#[test]
fn test_peer_info_serialization_roundtrip() {
    let peer = PeerInfo {
        device_name: "Alice-PC".to_string(),
        device_type: "desktop".to_string(),
        os: "windows".to_string(),
        ip: "192.168.1.50".to_string(),
        port: 5050,
        pairing_pin: None,
    };

    let json_str = serde_json::to_string(&peer).expect("failed to serialize PeerInfo");
    assert!(json_str.contains("\"device_name\":\"Alice-PC\""));
    assert!(json_str.contains("\"port\":5050"));
    assert!(!json_str.contains("pairing_pin"));

    let deserialized: PeerInfo =
        serde_json::from_str(&json_str).expect("failed to deserialize PeerInfo");
    assert_eq!(peer, deserialized);

    // Test with pairing_pin
    let peer_with_pin = PeerInfo::new(
        "Charlie-Laptop",
        "desktop",
        "linux",
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
    assert_eq!(accepted, des_accepted);

    let declined = TransferResponse::declined("User rejected the transfer request");
    let json_declined =
        serde_json::to_string(&declined).expect("failed to serialize declined TransferResponse");
    assert!(json_declined.contains("\"status\":\"declined\""));
    assert!(json_declined.contains("User rejected"));

    let des_declined: TransferResponse =
        serde_json::from_str(&json_declined).expect("failed to deserialize declined TransferResponse");
    assert_eq!(declined, des_declined);
}

#[test]
fn test_chunk_header_and_response_roundtrip() {
    let header = ChunkHeader::new("sess-123", "file-456", 1048576);
    let json_header = serde_json::to_string(&header).unwrap();
    assert!(json_header.contains("\"session_token\":\"sess-123\""));
    assert!(json_header.contains("\"offset\":1048576"));
    let des_header: ChunkHeader = serde_json::from_str(&json_header).unwrap();
    assert_eq!(header, des_header);

    let chunk_res = ChunkResponse {
        file_id: "file-456".to_string(),
        bytes_written: 524288,
        current_total: 1572864,
    };
    let json_res = serde_json::to_string(&chunk_res).unwrap();
    assert!(json_res.contains("\"bytes_written\":524288"));
    let des_res: ChunkResponse = serde_json::from_str(&json_res).unwrap();
    assert_eq!(chunk_res, des_res);
}

#[test]
fn test_finish_request_and_response_roundtrip() {
    let finish_req = FinishRequest::new("sess-finish-abc", "f-final");
    let json_req =
        serde_json::to_string(&finish_req).expect("failed to serialize FinishRequest");
    assert!(json_req.contains("\"session_token\":\"sess-finish-abc\""));
    assert!(json_req.contains("\"file_id\":\"f-final\""));

    let des_req: FinishRequest =
        serde_json::from_str(&json_req).expect("failed to deserialize FinishRequest");
    assert_eq!(finish_req, des_req);

    let finish_res = FinishResponse::completed("f-final", "C:\\Downloads\\greeting.txt");
    let json_res =
        serde_json::to_string(&finish_res).expect("failed to serialize FinishResponse");
    assert!(json_res.contains("\"status\":\"completed\""));
    assert!(json_res.contains("\"file_id\":\"f-final\""));
    let des_res: FinishResponse =
        serde_json::from_str(&json_res).expect("failed to deserialize FinishResponse");
    assert_eq!(finish_res, des_res);
}

#[test]
fn test_transfer_status_query_response() {
    let query_res = TransferStatusQueryResponse {
        file_id: "f-123".to_string(),
        bytes_received: 1536,
        status: TransferStatus::InProgress,
    };

    let json_str =
        serde_json::to_string(&query_res).expect("failed to serialize query response");
    assert!(json_str.contains("\"status\":\"in_progress\""));
    assert!(json_str.contains("\"bytes_received\":1536"));
    assert!(json_str.contains("\"file_id\":\"f-123\""));

    let des: TransferStatusQueryResponse =
        serde_json::from_str(&json_str).expect("failed to deserialize query response");
    assert_eq!(query_res, des);
}

#[test]
fn test_cancel_request_roundtrip() {
    let cancel_req = CancelRequest::new("sess-cancel-123", Some("Transfer timed out".to_string()));
    let json_str = serde_json::to_string(&cancel_req).unwrap();
    assert!(json_str.contains("\"session_token\":\"sess-cancel-123\""));
    assert!(json_str.contains("Transfer timed out"));

    let des: CancelRequest = serde_json::from_str(&json_str).unwrap();
    assert_eq!(cancel_req, des);
}
