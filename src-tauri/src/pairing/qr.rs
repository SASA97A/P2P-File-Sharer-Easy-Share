use qrcode::render::svg;
use qrcode::QrCode;
use url::form_urlencoded;

/// Builds a standard pairing URL for EasyShare:
/// `easyshare://pair?ip={ip}&port={port}&pin={pin}&name={name}`
pub fn build_pairing_url(ip: &str, port: u16, pin: &str, name: &str) -> String {
    let encoded_name: String = form_urlencoded::byte_serialize(name.as_bytes()).collect();
    format!(
        "easyshare://pair?ip={}&port={}&pin={}&name={}",
        ip, port, pin, encoded_name
    )
}

/// Generates a scalable inline SVG string for the given URL or content.
pub fn generate_qr_svg(url: &str) -> Result<String, String> {
    let code = QrCode::new(url.as_bytes()).map_err(|e| e.to_string())?;
    let image = code
        .render()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    Ok(image)
}
