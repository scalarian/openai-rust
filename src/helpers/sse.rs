use crate::{OpenAIError, error::ErrorKind};

/// One logical SSE frame.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SseFrame {
    /// Optional event name.
    pub event: Option<String>,
    /// Event data payload.
    pub data: String,
}

/// Incremental SSE parser that tolerates fragmented frames and mixed newlines.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SseParser {
    pending: Vec<u8>,
    event: Option<String>,
    data_lines: Vec<String>,
}

impl SseParser {
    /// Pushes one raw byte chunk into the parser and returns any completed frames.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<SseFrame>, OpenAIError> {
        self.pending.extend_from_slice(chunk);
        self.drain_lines(false)
    }

    /// Finishes parsing at EOF, flushing any final line/frame.
    pub fn finish(&mut self) -> Result<Vec<SseFrame>, OpenAIError> {
        let mut frames = self.drain_lines(true)?;
        if !self.pending.is_empty() {
            let line = String::from_utf8(self.pending.split_off(0)).map_err(|error| {
                OpenAIError::new(
                    ErrorKind::Parse,
                    format!("invalid UTF-8 in SSE line buffer: {error}"),
                )
                .with_source(error)
            })?;
            if !line.is_empty() {
                frames.extend(self.handle_line(&line));
            }
        }
        if let Some(frame) = self.flush_event() {
            frames.push(frame);
        }
        Ok(frames)
    }

    fn drain_lines(&mut self, eof: bool) -> Result<Vec<SseFrame>, OpenAIError> {
        let mut frames = Vec::new();
        loop {
            let Some((line, consumed)) = next_line(&self.pending, eof)? else {
                break;
            };
            self.pending.drain(..consumed);
            frames.extend(self.handle_line(&line));
        }
        Ok(frames)
    }

    fn handle_line(&mut self, line: &str) -> Vec<SseFrame> {
        if line.is_empty() {
            return self.flush_event().into_iter().collect();
        }

        if line.starts_with(':') {
            return Vec::new();
        }

        let (field, value) = line
            .split_once(':')
            .map_or((line, ""), |(field, value)| (field, value.trim_start()));

        match field {
            "event" => {
                self.event = Some(value.to_string());
            }
            "data" => {
                self.data_lines.push(value.to_string());
            }
            _ => {}
        }

        Vec::new()
    }

    fn flush_event(&mut self) -> Option<SseFrame> {
        if self.event.is_none() && self.data_lines.is_empty() {
            return None;
        }

        Some(SseFrame {
            event: self.event.take(),
            data: self.data_lines.drain(..).collect::<Vec<_>>().join("\n"),
        })
    }
}

fn next_line(buffer: &[u8], eof: bool) -> Result<Option<(String, usize)>, OpenAIError> {
    let mut index = 0usize;
    while index < buffer.len() {
        match buffer[index] {
            b'\n' => {
                let line = String::from_utf8(buffer[..index].to_vec()).map_err(|error| {
                    OpenAIError::new(
                        ErrorKind::Parse,
                        format!("invalid UTF-8 in SSE frame: {error}"),
                    )
                    .with_source(error)
                })?;
                let consumed = index + 1;
                let line = line.strip_suffix('\r').unwrap_or(&line).to_string();
                return Ok(Some((line, consumed)));
            }
            b'\r' => {
                if index + 1 < buffer.len() && buffer[index + 1] == b'\n' {
                    let line = String::from_utf8(buffer[..index].to_vec()).map_err(|error| {
                        OpenAIError::new(
                            ErrorKind::Parse,
                            format!("invalid UTF-8 in SSE frame: {error}"),
                        )
                        .with_source(error)
                    })?;
                    return Ok(Some((line, index + 2)));
                }
                if eof {
                    let line = String::from_utf8(buffer[..index].to_vec()).map_err(|error| {
                        OpenAIError::new(
                            ErrorKind::Parse,
                            format!("invalid UTF-8 in SSE frame: {error}"),
                        )
                        .with_source(error)
                    })?;
                    return Ok(Some((line, index + 1)));
                }
                if index + 1 == buffer.len() {
                    return Ok(None);
                }
            }
            _ => {}
        }
        index += 1;
    }

    if eof && !buffer.is_empty() {
        let line = String::from_utf8(buffer.to_vec()).map_err(|error| {
            OpenAIError::new(
                ErrorKind::Parse,
                format!("invalid UTF-8 in SSE frame: {error}"),
            )
            .with_source(error)
        })?;
        return Ok(Some((line, buffer.len())));
    }

    Ok(None)
}
