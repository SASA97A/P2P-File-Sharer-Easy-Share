use easy_share_lib::pairing::{pin, qr};

#[test]
fn test_generate_pin_format_and_randomness() {
    let pin1 = pin::generate_pin();
    let pin2 = pin::generate_pin();

    assert_eq!(pin1.len(), 6, "PIN must be exactly 6 characters");
    assert!(pin1.chars().all(|c| c.is_ascii_digit()), "PIN must only contain digits");

    assert_eq!(pin2.len(), 6);
    assert!(pin2.chars().all(|c| c.is_ascii_digit()));

    // Generate several PINs and verify variation and valid range
    let mut pins = std::collections::HashSet::new();
    for _ in 0..50 {
        let p = pin::generate_pin();
        assert_eq!(p.len(), 6);
        assert!(p.chars().all(|c| c.is_ascii_digit()));
        let val: u32 = p.parse().expect("should parse as u32");
        assert!(val <= 999_999);
        pins.insert(p);
    }
    assert!(pins.len() > 1, "PIN generator should produce different random values");
}

#[test]
fn test_format_pin() {
    assert_eq!(pin::format_pin("123456"), "123 - 456");
    assert_eq!(pin::format_pin("000000"), "000 - 000");
    assert_eq!(pin::format_pin("839421"), "839 - 421");
    // Non-6-digit fallback
    assert_eq!(pin::format_pin("123"), "123");
    assert_eq!(pin::format_pin("1234567"), "1234567");
    assert_eq!(pin::format_pin(""), "");
}

#[test]
fn test_normalize_pin() {
    // Exact 6 digits
    assert_eq!(pin::normalize_pin("123456"), Some("123456".to_string()));
    assert_eq!(pin::normalize_pin("000123"), Some("000123".to_string()));

    // With hyphen and spaces
    assert_eq!(pin::normalize_pin("123 - 456"), Some("123456".to_string()));
    assert_eq!(pin::normalize_pin("123-456"), Some("123456".to_string()));
    assert_eq!(pin::normalize_pin(" 123 456 "), Some("123456".to_string()));
    assert_eq!(pin::normalize_pin(" 1 2 3 - 4 5 6 "), Some("123456".to_string()));
    assert_eq!(pin::normalize_pin("\t123\n-\r456 "), Some("123456".to_string()));

    // Invalid inputs
    assert_eq!(pin::normalize_pin("12345"), None); // 5 digits
    assert_eq!(pin::normalize_pin("1234567"), None); // 7 digits
    assert_eq!(pin::normalize_pin(""), None); // empty
    assert_eq!(pin::normalize_pin("123-45a"), None); // contains non-digit
    assert_eq!(pin::normalize_pin("abcdef"), None); // all non-digits
    assert_eq!(pin::normalize_pin("123.456"), None); // period not ignored
    assert_eq!(pin::normalize_pin("123_456"), None); // underscore not ignored
}

#[test]
fn test_build_pairing_url() {
    let url = qr::build_pairing_url("192.168.1.50", 5050, "839421", "Alice Device");
    assert!(url.starts_with("easyshare://pair?"));
    assert!(url.contains("ip=192.168.1.50"));
    assert!(url.contains("port=5050"));
    assert!(url.contains("pin=839421"));
    assert!(url.contains("name=Alice%20Device") || url.contains("name=Alice+Device"));

    // Special characters in name
    let special_url = qr::build_pairing_url("10.0.0.1", 8080, "001122", "Bob's MacBook & Phone");
    assert!(special_url.starts_with("easyshare://pair?ip=10.0.0.1&port=8080&pin=001122"));
    assert!(special_url.contains("name="));
}

#[test]
fn test_generate_qr_svg() {
    let url = "easyshare://pair?ip=192.168.1.50&port=5050&pin=839421&name=Alice";
    let svg_result = qr::generate_qr_svg(url);

    assert!(svg_result.is_ok(), "QR SVG generation should succeed");
    let svg = svg_result.unwrap();

    assert!(svg.contains("<svg"), "SVG output should contain <svg tag");
    assert!(svg.contains("</svg>"), "SVG output should contain </svg> closing tag");
    assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\"") || svg.contains("<svg"));
}
