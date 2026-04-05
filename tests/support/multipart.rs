#![allow(dead_code)]

use std::collections::BTreeMap;
use std::str;

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
    let marker = format!("--{boundary}").into_bytes();
    let mut parts = Vec::new();

    for raw_section in split_bytes(body, &marker).into_iter().skip(1) {
        let section = raw_section.strip_prefix(b"\r\n").unwrap_or(raw_section);
        if section.is_empty() || section.starts_with(b"--") {
            continue;
        }

        let (raw_headers, raw_body) = section
            .split_once_bytes(b"\r\n\r\n")
            .ok_or_else(|| String::from("multipart section missing header/body delimiter"))?;
        let raw_body = raw_body.strip_suffix(b"\r\n").unwrap_or(raw_body);

        let mut headers = BTreeMap::new();
        let mut name = None;
        let mut filename = None;
        let raw_headers = str::from_utf8(raw_headers).map_err(|err| err.to_string())?;
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
            body: raw_body.to_vec(),
        });
    }

    Ok(ParsedMultipart { parts })
}

trait ByteSliceExt {
    fn split_once_bytes<'a>(&'a self, delimiter: &[u8]) -> Option<(&'a [u8], &'a [u8])>;
}

impl ByteSliceExt for [u8] {
    fn split_once_bytes<'a>(&'a self, delimiter: &[u8]) -> Option<(&'a [u8], &'a [u8])> {
        find_subslice(self, delimiter).map(|index| {
            let next = index + delimiter.len();
            (&self[..index], &self[next..])
        })
    }
}

fn split_bytes<'a>(bytes: &'a [u8], delimiter: &[u8]) -> Vec<&'a [u8]> {
    let mut segments = Vec::new();
    let mut start = 0;
    while let Some(relative_index) = find_subslice(&bytes[start..], delimiter) {
        let index = start + relative_index;
        segments.push(&bytes[start..index]);
        start = index + delimiter.len();
    }
    segments.push(&bytes[start..]);
    segments
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }

    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
