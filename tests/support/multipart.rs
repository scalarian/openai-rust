use std::collections::BTreeMap;

/// Parsed multipart body.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParsedMultipart {
    pub parts: Vec<MultipartPart>,
}

/// One multipart part.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MultipartPart {
    pub name: Option<String>,
    pub filename: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

/// Parses a multipart body using a known boundary.
pub fn parse_multipart(body: &[u8], boundary: &str) -> Result<ParsedMultipart, String> {
    let marker = format!("--{boundary}");
    let text = String::from_utf8(body.to_vec()).map_err(|err| err.to_string())?;
    let mut parts = Vec::new();

    for raw_section in text.split(&marker).skip(1) {
        let section = raw_section.trim_start_matches("\r\n");
        if section.is_empty() || section.starts_with("--") {
            continue;
        }

        let (raw_headers, raw_body) = section
            .split_once("\r\n\r\n")
            .ok_or_else(|| String::from("multipart section missing header/body delimiter"))?;
        let raw_body = raw_body
            .trim_end_matches("\r\n")
            .trim_end_matches("--")
            .trim_end_matches("\r\n");

        let mut headers = BTreeMap::new();
        let mut name = None;
        let mut filename = None;
        for line in raw_headers.lines() {
            let (header_name, header_value) = line
                .split_once(':')
                .ok_or_else(|| format!("invalid multipart header: {line}"))?;
            let header_name = header_name.trim().to_ascii_lowercase();
            let header_value = header_value.trim().to_string();
            if header_name == "content-disposition" {
                for token in header_value.split(';').skip(1) {
                    let token = token.trim();
                    if let Some(value) = token.strip_prefix("name=") {
                        name = Some(value.trim_matches('"').to_string());
                    } else if let Some(value) = token.strip_prefix("filename=") {
                        filename = Some(value.trim_matches('"').to_string());
                    }
                }
            }
            headers.insert(header_name, header_value);
        }

        parts.push(MultipartPart {
            name,
            filename,
            headers,
            body: raw_body.as_bytes().to_vec(),
        });
    }

    Ok(ParsedMultipart { parts })
}
