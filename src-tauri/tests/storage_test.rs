use easy_share_lib::storage::file_manager::{FileManager, StorageError};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

#[test]
fn test_sanitize_filename_path_traversal() {
    assert_eq!(FileManager::sanitize_filename("../../test.exe"), "test.exe");
    assert_eq!(
        FileManager::sanitize_filename("C:\\Windows\\System32\\cmd.exe"),
        "cmd.exe"
    );
    assert_eq!(FileManager::sanitize_filename("/etc/passwd"), "passwd");
    assert_eq!(FileManager::sanitize_filename("foo/bar/baz.zip"), "baz.zip");
    assert_eq!(FileManager::sanitize_filename("foo\\bar\\baz.zip"), "baz.zip");
    assert_eq!(
        FileManager::sanitize_filename("....//..\\nested/my_file.pdf"),
        "my_file.pdf"
    );
}

#[test]
fn test_sanitize_filename_windows_reserved_names() {
    assert_eq!(FileManager::sanitize_filename("CON"), "_CON");
    assert_eq!(FileManager::sanitize_filename("con"), "_con");
    assert_eq!(FileManager::sanitize_filename("PRN"), "_PRN");
    assert_eq!(FileManager::sanitize_filename("AUX"), "_AUX");
    assert_eq!(FileManager::sanitize_filename("NUL"), "_NUL");
    assert_eq!(FileManager::sanitize_filename("COM1.txt"), "_COM1.txt");
    assert_eq!(FileManager::sanitize_filename("com9.zip"), "_com9.zip");
    assert_eq!(FileManager::sanitize_filename("LPT2.pdf"), "_LPT2.pdf");
    assert_eq!(FileManager::sanitize_filename("lpt1.tar.gz"), "_lpt1.tar.gz");
}

#[test]
fn test_sanitize_filename_invalid_filesystem_chars() {
    assert_eq!(
        FileManager::sanitize_filename("test:file*name?.txt"),
        "test_file_name_.txt"
    );
    assert_eq!(
        FileManager::sanitize_filename("file<name>with|quotes\".ext"),
        "file_name_with_quotes_.ext"
    );
    assert_eq!(
        FileManager::sanitize_filename("hello\x00world\x1f.txt"),
        "hello_world_.txt"
    );
}

#[test]
fn test_sanitize_filename_edge_cases() {
    assert_eq!(FileManager::sanitize_filename(""), "unnamed_file");
    assert_eq!(FileManager::sanitize_filename("..."), "unnamed_file");
    assert_eq!(FileManager::sanitize_filename("   "), "unnamed_file");
    assert_eq!(FileManager::sanitize_filename("my file.txt "), "my file.txt");
    assert_eq!(FileManager::sanitize_filename("my file.txt."), "my file.txt");
}

#[test]
fn test_sanitize_filename_unicode_bidi_and_hidden_chars() {
    // Right-to-Left Override spoofing: "report\u{202E}cod.exe"
    // Without sanitization, displayed as "reportexe.doc"
    let sanitized_rlo = FileManager::sanitize_filename("report\u{202E}cod.exe");
    assert!(!sanitized_rlo.contains('\u{202E}'), "Must strip RLO character");
    assert_eq!(sanitized_rlo, "report_cod.exe");

    // Zero-width space and BOM
    let sanitized_zw = FileManager::sanitize_filename("hidden\u{200B}\u{FEFF}file.txt");
    assert!(!sanitized_zw.contains('\u{200B}'));
    assert!(!sanitized_zw.contains('\u{FEFF}'));
    assert_eq!(sanitized_zw, "hidden__file.txt");
}

#[test]
fn test_get_part_path_derivation() {
    let downloads_dir = PathBuf::from("C:\\Downloads");
    let part_path = FileManager::get_part_path(&downloads_dir, "sample.iso", "sess-12345");
    assert_eq!(
        part_path,
        downloads_dir.join("sample.iso.sess-12345.part")
    );
}

#[test]
fn test_get_unique_dest_path_collision_resolution() {
    let temp_dir = std::env::temp_dir().join("easy_share_test_unique_dest");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("failed to create temp dir");

    // Case 1: Initial file does not exist
    let dest1 = FileManager::get_unique_dest_path(&temp_dir, "file.txt");
    assert_eq!(dest1, temp_dir.join("file.txt"));

    // Create file.txt
    File::create(&dest1).expect("failed to create file.txt");

    // Case 2: file.txt exists -> file(1).txt
    let dest2 = FileManager::get_unique_dest_path(&temp_dir, "file.txt");
    assert_eq!(dest2, temp_dir.join("file(1).txt"));

    // Create file(1).txt
    File::create(&dest2).expect("failed to create file(1).txt");

    // Case 3: file(1).txt exists -> file(2).txt
    let dest3 = FileManager::get_unique_dest_path(&temp_dir, "file.txt");
    assert_eq!(dest3, temp_dir.join("file(2).txt"));

    // Case 4: File without extension
    let dest_no_ext1 = FileManager::get_unique_dest_path(&temp_dir, "binary_data");
    assert_eq!(dest_no_ext1, temp_dir.join("binary_data"));
    File::create(&dest_no_ext1).expect("failed to create binary_data");

    let dest_no_ext2 = FileManager::get_unique_dest_path(&temp_dir, "binary_data");
    assert_eq!(dest_no_ext2, temp_dir.join("binary_data(1)"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_commit_part_file_atomic_rename() {
    let temp_dir = std::env::temp_dir().join("easy_share_test_commit");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("failed to create temp dir");

    let part_path = FileManager::get_part_path(&temp_dir, "test_doc.pdf", "sess-abc");
    let payload = b"Hello, Easy Share File Storage!";

    let mut part_file = File::create(&part_path).expect("failed to create part file");
    part_file.write_all(payload).expect("failed to write part file");
    drop(part_file);

    let dest_path = FileManager::get_unique_dest_path(&temp_dir, "test_doc.pdf");
    assert_eq!(dest_path, temp_dir.join("test_doc.pdf"));

    let result = FileManager::commit_part_file(&part_path, &dest_path);
    assert!(result.is_ok(), "commit_part_file failed: {:?}", result.err());

    assert!(!part_path.exists(), "part file should not exist after commit");
    assert!(dest_path.exists(), "dest file should exist after commit");

    let contents = fs::read(&dest_path).expect("failed to read dest file");
    assert_eq!(contents, payload);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_commit_part_file_existing_dest_replaced() {
    let temp_dir = std::env::temp_dir().join("easy_share_test_commit_replace");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("failed to create temp dir");

    let part_path = temp_dir.join("test.bin.part");
    let dest_path = temp_dir.join("test.bin");

    // Pre-create destination file with stale content and set read-only attribute
    fs::write(&dest_path, b"stale data").expect("failed to write stale dest");
    let mut perms = fs::metadata(&dest_path).expect("get perms").permissions();
    perms.set_readonly(true);
    fs::set_permissions(&dest_path, perms).expect("set readonly");

    // Create part file with fresh content
    fs::write(&part_path, b"fresh replacement data").expect("failed to write part file");

    let result = FileManager::commit_part_file(&part_path, &dest_path);
    assert!(
        result.is_ok(),
        "commit_part_file should replace existing dest without failing: {:?}",
        result.err()
    );

    assert!(!part_path.exists(), "part file should not exist after commit");
    assert!(dest_path.exists(), "dest file should exist after commit");
    assert_eq!(
        fs::read(&dest_path).expect("failed to read dest"),
        b"fresh replacement data"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_commit_part_file_missing_source_error() {
    let non_existent_part = PathBuf::from("non_existent_file.12345.part");
    let dest_path = PathBuf::from("final_output.txt");

    let result = FileManager::commit_part_file(&non_existent_part, &dest_path);
    match result {
        Err(StorageError::FileNotFound(p)) => assert_eq!(p, non_existent_part),
        other => panic!("expected StorageError::FileNotFound, got {:?}", other),
    }
}

#[test]
fn test_calculate_and_verify_blake3_hash() {
    let temp_dir = std::env::temp_dir().join("easy_share_test_blake3");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("failed to create temp dir");

    let payload = b"Cryptographic verification with BLAKE3 in Easy Share.";
    let expected_hash = blake3::hash(payload).to_hex().to_string();

    let bytes_hash = FileManager::calculate_blake3_bytes(payload);
    assert_eq!(bytes_hash, expected_hash);

    let file_path = temp_dir.join("blake3_sample.bin");
    fs::write(&file_path, payload).expect("failed to write payload");

    let file_hash = FileManager::calculate_blake3_file(&file_path).expect("failed to calculate file hash");
    assert_eq!(file_hash, expected_hash);

    let file_size = FileManager::get_file_size(&file_path).expect("failed to get file size");
    assert_eq!(file_size, payload.len() as u64);

    let verified = FileManager::verify_blake3_file(&file_path, &expected_hash)
        .expect("failed to verify file hash");
    assert!(verified);

    let bad_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let bad_verified = FileManager::verify_blake3_file(&file_path, bad_hash)
        .expect("failed to verify bad file hash");
    assert!(!bad_verified);

    let non_existent = temp_dir.join("does_not_exist.bin");
    let missing_res = FileManager::calculate_blake3_file(&non_existent);
    assert!(matches!(missing_res, Err(StorageError::FileNotFound(_))));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_calculate_and_verify_blake3_hash_async() {
    let temp_dir = std::env::temp_dir().join("easy_share_test_blake3_async");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("failed to create temp dir");

    let payload = b"Async cryptographic verification with BLAKE3 in Easy Share.";
    let expected_hash = blake3::hash(payload).to_hex().to_string();

    let file_path = temp_dir.join("blake3_async_sample.bin");
    fs::write(&file_path, payload).expect("failed to write payload");

    let file_hash = FileManager::calculate_blake3_file_async(file_path.clone())
        .await
        .expect("failed to calculate async file hash");
    assert_eq!(file_hash, expected_hash);

    let verified = FileManager::verify_blake3_file_async(file_path.clone(), expected_hash)
        .await
        .expect("failed to verify async file hash");
    assert!(verified);

    let bad_hash = "0000000000000000000000000000000000000000000000000000000000000000".to_string();
    let bad_verified = FileManager::verify_blake3_file_async(file_path.clone(), bad_hash)
        .await
        .expect("failed to verify bad async file hash");
    assert!(!bad_verified);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_get_downloads_dir() {
    let downloads_dir = FileManager::get_downloads_dir();
    assert!(!downloads_dir.as_os_str().is_empty());
}

#[test]
fn test_resolve_destination_path_sanitization_and_hierarchy() {
    let temp_dir = std::env::temp_dir().join("easy_share_test_dest_hierarchy");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // 1. Valid nested relative path
    let p1 = FileManager::resolve_destination_path(&temp_dir, "main.rs", Some("MyProject/src/main.rs"));
    assert_eq!(p1, temp_dir.join("MyProject").join("src").join("main.rs"));

    // 2. Traversal attack neutralization
    let p2 = FileManager::resolve_destination_path(&temp_dir, "passwd", Some("../../etc/passwd"));
    assert_eq!(p2, temp_dir.join("etc").join("passwd"));

    // 3. Drive letter and absolute prefix neutralization
    let p3 = FileManager::resolve_destination_path(&temp_dir, "boot.ini", Some("C:\\Windows\\boot.ini"));
    assert_eq!(p3, temp_dir.join("Windows").join("boot.ini"));

    // 4. Fallback to filename when None or empty
    let p4 = FileManager::resolve_destination_path(&temp_dir, "notes.txt", None);
    assert_eq!(p4, temp_dir.join("notes.txt"));

    let p5 = FileManager::resolve_destination_path(&temp_dir, "notes.txt", Some(""));
    assert_eq!(p5, temp_dir.join("notes.txt"));

    // 5. Collision handling when file already exists
    fs::create_dir_all(temp_dir.join("MyProject").join("src")).unwrap();
    File::create(&p1).unwrap();

    let p1_col = FileManager::resolve_destination_path(&temp_dir, "main.rs", Some("MyProject/src/main.rs"));
    assert_eq!(p1_col, temp_dir.join("MyProject").join("src").join("main(1).rs"));

    let _ = fs::remove_dir_all(&temp_dir);
}
