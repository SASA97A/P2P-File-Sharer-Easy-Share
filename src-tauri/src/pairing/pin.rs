use rand::Rng;

/// Generates a random 6-digit numeric PIN string (e.g. "839421").
pub fn generate_pin() -> String {
    let mut rng = rand::thread_rng();
    let num: u32 = rng.gen_range(0..=999_999);
    format!("{:06}", num)
}

/// Formats a 6-digit PIN with a visual hyphen separator (e.g. "839 - 421").
/// If the input is not exactly 6 characters, returns the input string unchanged.
pub fn format_pin(pin: &str) -> String {
    if pin.len() == 6 && pin.is_ascii() {
        format!("{} - {}", &pin[..3], &pin[3..])
    } else {
        pin.to_string()
    }
}

/// Normalizes PIN input by stripping whitespace and hyphens,
/// and validates that the result is exactly 6 numeric digits ('0'..='9').
/// Returns Some(pin) if valid, or None if invalid.
pub fn normalize_pin(input: &str) -> Option<String> {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();

    if cleaned.len() == 6 && cleaned.chars().all(|c| c.is_ascii_digit()) {
        Some(cleaned)
    } else {
        None
    }
}
