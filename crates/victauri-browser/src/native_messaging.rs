use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MAX_INPUT_SIZE: usize = 1_048_576;
const MAX_OUTPUT_SIZE: usize = 67_108_864;

/// Read one native messaging frame from stdin.
///
/// Chrome sends: 4-byte little-endian length prefix, then UTF-8 JSON.
/// Maximum input size is 1 MB per Chrome's spec.
///
/// # Errors
///
/// Returns an error if the message exceeds the 1 MB limit, if the read
/// fails, or if the bytes are not valid JSON.
pub async fn read_message(
    reader: &mut (impl AsyncReadExt + Unpin),
) -> Result<serde_json::Value, NativeMessageError> {
    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes).await.map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            NativeMessageError::Disconnected
        } else {
            NativeMessageError::Io(e)
        }
    })?;

    let len = u32::from_le_bytes(len_bytes) as usize;
    if len > MAX_INPUT_SIZE {
        return Err(NativeMessageError::TooLarge {
            size: len,
            max: MAX_INPUT_SIZE,
        });
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await.map_err(NativeMessageError::Io)?;

    serde_json::from_slice(&buf).map_err(NativeMessageError::Json)
}

/// Write one native messaging frame to stdout.
///
/// Format: 4-byte little-endian length prefix, then UTF-8 JSON.
/// Maximum output size is 64 MB per Chrome's spec (host → extension direction).
///
/// # Errors
///
/// Returns an error if the serialized message exceeds 64 MB or if the write fails.
pub async fn write_message(
    writer: &mut (impl AsyncWriteExt + Unpin),
    msg: &serde_json::Value,
) -> Result<(), NativeMessageError> {
    let bytes = serde_json::to_vec(msg).map_err(NativeMessageError::Json)?;
    if bytes.len() > MAX_OUTPUT_SIZE {
        return Err(NativeMessageError::TooLarge {
            size: bytes.len(),
            max: MAX_OUTPUT_SIZE,
        });
    }

    let len_bytes = (bytes.len() as u32).to_le_bytes();
    writer.write_all(&len_bytes).await.map_err(NativeMessageError::Io)?;
    writer.write_all(&bytes).await.map_err(NativeMessageError::Io)?;
    writer.flush().await.map_err(NativeMessageError::Io)?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum NativeMessageError {
    #[error("native messaging peer disconnected")]
    Disconnected,

    #[error("message size {size} exceeds limit {max}")]
    TooLarge { size: usize, max: usize },

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn roundtrip_message() {
        let msg = serde_json::json!({"type": "execute", "id": "abc123", "method": "snapshot"});

        let mut buf = Vec::new();
        write_message(&mut buf, &msg).await.unwrap();

        let mut reader = BufReader::new(buf.as_slice());
        let decoded = read_message(&mut reader).await.unwrap();

        assert_eq!(msg, decoded);
    }

    #[tokio::test]
    async fn rejects_oversized_input() {
        let len = (MAX_INPUT_SIZE + 1) as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend(vec![0u8; MAX_INPUT_SIZE + 1]);

        let mut reader = BufReader::new(buf.as_slice());
        let result = read_message(&mut reader).await;
        assert!(matches!(result, Err(NativeMessageError::TooLarge { .. })));
    }

    #[tokio::test]
    async fn detects_disconnect() {
        let buf: &[u8] = &[];
        let mut reader = BufReader::new(buf);
        let result = read_message(&mut reader).await;
        assert!(matches!(result, Err(NativeMessageError::Disconnected)));
    }

    #[tokio::test]
    async fn handles_empty_json() {
        let msg = serde_json::json!({});

        let mut buf = Vec::new();
        write_message(&mut buf, &msg).await.unwrap();

        let mut reader = BufReader::new(buf.as_slice());
        let decoded = read_message(&mut reader).await.unwrap();
        assert_eq!(decoded, serde_json::json!({}));
    }

    #[tokio::test]
    async fn handles_multiple_messages() {
        let msgs = vec![
            serde_json::json!({"id": "1"}),
            serde_json::json!({"id": "2", "data": "hello"}),
            serde_json::json!({"id": "3", "nested": {"key": "value"}}),
        ];

        let mut buf = Vec::new();
        for msg in &msgs {
            write_message(&mut buf, msg).await.unwrap();
        }

        let mut reader = BufReader::new(buf.as_slice());
        for expected in &msgs {
            let decoded = read_message(&mut reader).await.unwrap();
            assert_eq!(&decoded, expected);
        }
    }

    #[tokio::test]
    async fn partial_length_prefix_is_disconnect() {
        let buf: &[u8] = &[0x02, 0x00];
        let mut reader = BufReader::new(buf);
        let result = read_message(&mut reader).await;
        assert!(matches!(result, Err(NativeMessageError::Disconnected)));
    }

    #[tokio::test]
    async fn invalid_json_returns_error() {
        let invalid = b"not json at all";
        let len = invalid.len() as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(invalid);

        let mut reader = BufReader::new(buf.as_slice());
        let result = read_message(&mut reader).await;
        assert!(matches!(result, Err(NativeMessageError::Json(_))));
    }

    #[tokio::test]
    async fn zero_length_message() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0u32.to_le_bytes());

        let mut reader = BufReader::new(buf.as_slice());
        let result = read_message(&mut reader).await;
        assert!(matches!(result, Err(NativeMessageError::Json(_))));
    }

    #[tokio::test]
    async fn unicode_message_roundtrip() {
        let msg = serde_json::json!({"emoji": "🔥🚀", "cjk": "日本語テスト", "mixed": "hello 世界"});

        let mut buf = Vec::new();
        write_message(&mut buf, &msg).await.unwrap();

        let mut reader = BufReader::new(buf.as_slice());
        let decoded = read_message(&mut reader).await.unwrap();
        assert_eq!(decoded["emoji"], "🔥🚀");
        assert_eq!(decoded["cjk"], "日本語テスト");
    }

    #[tokio::test]
    async fn large_message_near_limit() {
        let big_string = "x".repeat(500_000);
        let msg = serde_json::json!({"data": big_string});

        let mut buf = Vec::new();
        write_message(&mut buf, &msg).await.unwrap();

        let mut reader = BufReader::new(buf.as_slice());
        let decoded = read_message(&mut reader).await.unwrap();
        assert_eq!(decoded["data"].as_str().unwrap().len(), 500_000);
    }

    #[tokio::test]
    async fn write_message_length_prefix_correct() {
        let msg = serde_json::json!({"a": 1});
        let expected_json = serde_json::to_vec(&msg).unwrap();

        let mut buf = Vec::new();
        write_message(&mut buf, &msg).await.unwrap();

        let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        assert_eq!(len, expected_json.len());
        assert_eq!(&buf[4..], &expected_json);
    }

    #[tokio::test]
    async fn truncated_body_is_io_error() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&100u32.to_le_bytes());
        buf.extend_from_slice(b"short");

        let mut reader = BufReader::new(buf.as_slice());
        let result = read_message(&mut reader).await;
        assert!(matches!(result, Err(NativeMessageError::Io(_))));
    }
}
