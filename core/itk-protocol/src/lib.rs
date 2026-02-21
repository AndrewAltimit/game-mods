//! # ITK Protocol
//!
//! Wire protocol definitions for the Injection Toolkit.
//!
//! This crate defines the message format used for IPC communication between:
//! - Injected DLL/SO and daemon
//! - Daemon and overlay
//! - Daemon and MCP server
//!
//! ## Wire Format
//!
//! ```text
//! ┌─────────┬─────────┬──────────┬─────────────┬─────────┬───────────┐
//! │ Magic   │ Version │ MsgType  │ PayloadLen  │ CRC32   │ Payload   │
//! │ 4 bytes │ 4 bytes │ 4 bytes  │ 4 bytes     │ 4 bytes │ N bytes   │
//! │ "ITKP"  │ 1       │ enum     │ ≤ 1MB       │ crc32   │ bincode   │
//! └─────────┴─────────┴──────────┴─────────────┴─────────┴───────────┘
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::warn;

/// Protocol magic bytes: "ITKP" (Injection Toolkit Protocol)
pub const MAGIC: [u8; 4] = *b"ITKP";

/// Current protocol version (bumped to 2 for bitcode migration)
pub const VERSION: u32 = 2;

/// Maximum payload size (1 MB)
pub const MAX_PAYLOAD_SIZE: usize = 1024 * 1024;

/// Maximum string field length (256 bytes) per CLAUDE.md constraints
pub const MAX_STRING_LENGTH: usize = 256;

/// Maximum data/payload field length (64 KB) per CLAUDE.md constraints
pub const MAX_DATA_LENGTH: usize = 64 * 1024;

/// Header size in bytes
pub const HEADER_SIZE: usize = 20; // 4 + 4 + 4 + 4 + 4

/// Protocol errors
#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("invalid magic bytes: expected {expected:?}, got {got:?}")]
    InvalidMagic { expected: [u8; 4], got: [u8; 4] },

    #[error("unsupported protocol version: {0}")]
    UnsupportedVersion(u32),

    #[error("payload too large: {size} bytes (max {max})")]
    PayloadTooLarge { size: usize, max: usize },

    #[error("CRC mismatch: expected {expected:#x}, got {got:#x}")]
    CrcMismatch { expected: u32, got: u32 },

    #[error("unknown message type: {0}")]
    UnknownMessageType(u32),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("incomplete header: need {need} bytes, have {have}")]
    IncompleteHeader { need: usize, have: usize },

    #[error("incomplete payload: need {need} bytes, have {have}")]
    IncompletePayload { need: usize, have: usize },

    #[error("validation failed: field `{field}` - {reason}")]
    ValidationFailed { field: String, reason: String },
}

// =============================================================================
// Validation helpers
// =============================================================================

/// Validate that a string field does not exceed MAX_STRING_LENGTH.
fn validate_string(field: &str, value: &str) -> Result<(), ProtocolError> {
    if value.len() > MAX_STRING_LENGTH {
        return Err(ProtocolError::ValidationFailed {
            field: field.to_string(),
            reason: format!(
                "string length {} exceeds maximum {}",
                value.len(),
                MAX_STRING_LENGTH
            ),
        });
    }
    Ok(())
}

/// Validate that an optional string field does not exceed MAX_STRING_LENGTH.
fn validate_optional_string(field: &str, value: &Option<String>) -> Result<(), ProtocolError> {
    if let Some(s) = value {
        validate_string(field, s)?;
    }
    Ok(())
}

/// Validate that a data field does not exceed MAX_DATA_LENGTH.
fn validate_data(field: &str, value: &str) -> Result<(), ProtocolError> {
    if value.len() > MAX_DATA_LENGTH {
        return Err(ProtocolError::ValidationFailed {
            field: field.to_string(),
            reason: format!(
                "data length {} exceeds maximum {}",
                value.len(),
                MAX_DATA_LENGTH
            ),
        });
    }
    Ok(())
}

/// Validate that a float is finite (not NaN or Inf).
fn validate_finite_f32(field: &str, value: f32) -> Result<(), ProtocolError> {
    if !value.is_finite() {
        return Err(ProtocolError::ValidationFailed {
            field: field.to_string(),
            reason: format!("non-finite float value: {value}"),
        });
    }
    Ok(())
}

/// Validate that a float is finite (not NaN or Inf).
fn validate_finite_f64(field: &str, value: f64) -> Result<(), ProtocolError> {
    if !value.is_finite() {
        return Err(ProtocolError::ValidationFailed {
            field: field.to_string(),
            reason: format!("non-finite float value: {value}"),
        });
    }
    Ok(())
}

/// Message type identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum MessageType {
    /// Heartbeat/keepalive
    Ping = 0,
    /// Response to ping
    Pong = 1,

    /// Screen rectangle update (from injector to daemon/overlay)
    ScreenRect = 10,
    /// Window state update
    WindowState = 11,
    /// Combined overlay update
    OverlayUpdate = 12,

    /// Application state snapshot
    StateSnapshot = 20,
    /// State change event
    StateEvent = 21,
    /// State query request
    StateQuery = 22,
    /// State query response
    StateResponse = 23,

    /// Multiplayer sync state
    SyncState = 30,
    /// Clock synchronization ping
    ClockPing = 31,
    /// Clock synchronization pong
    ClockPong = 32,

    // Video playback messages (40-49)
    /// Load a video from URL or file path
    VideoLoad = 40,
    /// Start/resume video playback
    VideoPlay = 41,
    /// Pause video playback
    VideoPause = 42,
    /// Seek to a position in the video
    VideoSeek = 43,
    /// Video state update (position, duration, playing status)
    VideoState = 44,
    /// Video metadata (dimensions, duration, codec info)
    VideoMetadata = 45,
    /// Video playback error
    VideoError = 46,

    /// Error response
    Error = 255,
}

impl TryFrom<u32> for MessageType {
    type Error = ProtocolError;

    fn try_from(value: u32) -> Result<Self, ProtocolError> {
        match value {
            0 => Ok(Self::Ping),
            1 => Ok(Self::Pong),
            10 => Ok(Self::ScreenRect),
            11 => Ok(Self::WindowState),
            12 => Ok(Self::OverlayUpdate),
            20 => Ok(Self::StateSnapshot),
            21 => Ok(Self::StateEvent),
            22 => Ok(Self::StateQuery),
            23 => Ok(Self::StateResponse),
            30 => Ok(Self::SyncState),
            31 => Ok(Self::ClockPing),
            32 => Ok(Self::ClockPong),
            40 => Ok(Self::VideoLoad),
            41 => Ok(Self::VideoPlay),
            42 => Ok(Self::VideoPause),
            43 => Ok(Self::VideoSeek),
            44 => Ok(Self::VideoState),
            45 => Ok(Self::VideoMetadata),
            46 => Ok(Self::VideoError),
            255 => Ok(Self::Error),
            _ => Err(ProtocolError::UnknownMessageType(value)),
        }
    }
}

/// Message header
#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub magic: [u8; 4],
    pub version: u32,
    pub msg_type: MessageType,
    pub payload_len: u32,
    pub crc32: u32,
}

impl Header {
    /// Parse header from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() < HEADER_SIZE {
            return Err(ProtocolError::IncompleteHeader {
                need: HEADER_SIZE,
                have: bytes.len(),
            });
        }

        let magic: [u8; 4] = bytes[0..4].try_into().unwrap();
        if magic != MAGIC {
            return Err(ProtocolError::InvalidMagic {
                expected: MAGIC,
                got: magic,
            });
        }

        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != VERSION {
            return Err(ProtocolError::UnsupportedVersion(version));
        }

        let msg_type_raw = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let msg_type = MessageType::try_from(msg_type_raw)?;

        let payload_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        if payload_len as usize > MAX_PAYLOAD_SIZE {
            return Err(ProtocolError::PayloadTooLarge {
                size: payload_len as usize,
                max: MAX_PAYLOAD_SIZE,
            });
        }

        let crc32 = u32::from_le_bytes(bytes[16..20].try_into().unwrap());

        Ok(Self {
            magic,
            version,
            msg_type,
            payload_len,
            crc32,
        })
    }

    /// Serialize header to bytes
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut bytes = [0u8; HEADER_SIZE];
        bytes[0..4].copy_from_slice(&self.magic);
        bytes[4..8].copy_from_slice(&self.version.to_le_bytes());
        bytes[8..12].copy_from_slice(&(self.msg_type as u32).to_le_bytes());
        bytes[12..16].copy_from_slice(&self.payload_len.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.crc32.to_le_bytes());
        bytes
    }
}

/// Trait for validating deserialized protocol messages.
pub trait Validate {
    /// Validate that all fields satisfy protocol constraints.
    fn validate(&self) -> Result<(), ProtocolError>;
}

/// Screen rectangle message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenRect {
    /// X coordinate in window pixels
    pub x: f32,
    /// Y coordinate in window pixels
    pub y: f32,
    /// Width in window pixels
    pub width: f32,
    /// Height in window pixels
    pub height: f32,
    /// Rotation in radians (for perspective correction)
    pub rotation: f32,
    /// Whether the rect is valid/visible
    pub visible: bool,
}

/// Window state message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    /// Window X position on screen
    pub x: i32,
    /// Window Y position on screen
    pub y: i32,
    /// Window width
    pub width: u32,
    /// Window height
    pub height: u32,
    /// DPI scaling factor
    pub dpi_scale: f32,
    /// Whether fullscreen (overlay may not work)
    pub is_fullscreen: bool,
    /// Whether borderless windowed
    pub is_borderless: bool,
    /// Whether window is focused
    pub is_focused: bool,
}

/// Combined overlay update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayUpdate {
    /// Screen rectangle for rendering
    pub rect: ScreenRect,
    /// Window state
    pub window: WindowState,
    /// Timestamp (monotonic ms)
    pub timestamp_ms: u64,
}

/// Application state snapshot (generic, app-specific fields in `data`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Application identifier (e.g., "vrchat", "nms")
    pub app_id: String,
    /// Snapshot timestamp
    pub timestamp_ms: u64,
    /// Application-specific state as JSON
    pub data: String,
}

/// State change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEvent {
    /// Application identifier
    pub app_id: String,
    /// Event type (app-specific)
    pub event_type: String,
    /// Event timestamp
    pub timestamp_ms: u64,
    /// Event data as JSON
    pub data: String,
}

/// State query request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateQuery {
    /// Application identifier
    pub app_id: String,
    /// Query type (app-specific)
    pub query_type: String,
    /// Query parameters as JSON
    pub params: String,
}

/// State query response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateResponse {
    /// Whether query succeeded
    pub success: bool,
    /// Response data as JSON (if success)
    pub data: Option<String>,
    /// Error message (if !success)
    pub error: Option<String>,
}

/// Multiplayer sync state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    /// Content identifier (URL, file hash, etc.)
    pub content_id: String,
    /// Position at reference time (milliseconds)
    pub position_at_ref_ms: u64,
    /// Reference wallclock time (milliseconds since epoch)
    pub ref_wallclock_ms: u64,
    /// Whether currently playing
    pub is_playing: bool,
    /// Playback rate (1.0 = normal)
    pub playback_rate: f64,
}

/// Clock synchronization ping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockPing {
    /// Sender's local time (milliseconds)
    pub sender_time_ms: u64,
}

/// Clock synchronization pong
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockPong {
    /// Original sender's time from ping
    pub sender_time_ms: u64,
    /// Receiver's local time when ping was received
    pub receiver_time_ms: u64,
}

/// Error message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    /// Error code
    pub code: u32,
    /// Human-readable message
    pub message: String,
}

// =============================================================================
// Video Playback Messages
// =============================================================================

/// Load a video from URL or file path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoLoad {
    /// Video source (file path or URL)
    pub source: String,
    /// Start position in milliseconds (0 = beginning)
    pub start_position_ms: u64,
    /// Whether to start playing immediately
    pub autoplay: bool,
}

/// Start or resume video playback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoPlay {
    /// Optional position to start from (None = current position)
    pub from_position_ms: Option<u64>,
}

/// Pause video playback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoPause {
    // Empty struct - just a command
}

/// Seek to a position in the video
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoSeek {
    /// Target position in milliseconds
    pub position_ms: u64,
}

/// Video state update (broadcast periodically and on state changes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoState {
    /// Content identifier (URL hash or file path)
    pub content_id: String,
    /// Current playback position in milliseconds
    pub position_ms: u64,
    /// Total duration in milliseconds (0 if unknown/live)
    pub duration_ms: u64,
    /// Whether currently playing
    pub is_playing: bool,
    /// Whether currently buffering
    pub is_buffering: bool,
    /// Playback rate (1.0 = normal)
    pub playback_rate: f64,
    /// Volume (0.0 - 1.0)
    pub volume: f32,
}

/// Video metadata (sent once when video is loaded)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMetadata {
    /// Content identifier (URL hash or file path)
    pub content_id: String,
    /// Video width in pixels
    pub width: u32,
    /// Video height in pixels
    pub height: u32,
    /// Duration in milliseconds (0 if unknown/live)
    pub duration_ms: u64,
    /// Frames per second (0 if unknown)
    pub fps: f32,
    /// Codec name (e.g., "h264", "vp9")
    pub codec: String,
    /// Whether this is a live stream
    pub is_live: bool,
    /// Human-readable title (if available from metadata)
    pub title: Option<String>,
}

/// Video playback error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoError {
    /// Error code
    pub code: VideoErrorCode,
    /// Human-readable error message
    pub message: String,
    /// Whether playback can be retried
    pub is_recoverable: bool,
}

/// Video error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum VideoErrorCode {
    /// Unknown error
    Unknown = 0,
    /// Failed to open the source (file not found, network error, etc.)
    OpenFailed = 1,
    /// No video stream found in the source
    NoVideoStream = 2,
    /// Codec not supported
    UnsupportedCodec = 3,
    /// Decode error (corrupted data)
    DecodeError = 4,
    /// Network error during streaming
    NetworkError = 5,
    /// Source requires authentication
    AuthenticationRequired = 6,
    /// Geographic restriction
    GeoRestricted = 7,
    /// YouTube extraction failed (yt-dlp error)
    YoutubeExtractionFailed = 8,
    /// YouTube support not enabled
    YoutubeNotEnabled = 9,
}

// =============================================================================
// Validate implementations
// =============================================================================

impl Validate for ScreenRect {
    fn validate(&self) -> Result<(), ProtocolError> {
        validate_finite_f32("x", self.x)?;
        validate_finite_f32("y", self.y)?;
        validate_finite_f32("width", self.width)?;
        validate_finite_f32("height", self.height)?;
        validate_finite_f32("rotation", self.rotation)?;
        Ok(())
    }
}

impl Validate for WindowState {
    fn validate(&self) -> Result<(), ProtocolError> {
        validate_finite_f32("dpi_scale", self.dpi_scale)?;
        Ok(())
    }
}

impl Validate for OverlayUpdate {
    fn validate(&self) -> Result<(), ProtocolError> {
        self.rect.validate()?;
        self.window.validate()?;
        Ok(())
    }
}

impl Validate for StateSnapshot {
    fn validate(&self) -> Result<(), ProtocolError> {
        validate_string("app_id", &self.app_id)?;
        validate_data("data", &self.data)?;
        Ok(())
    }
}

impl Validate for StateEvent {
    fn validate(&self) -> Result<(), ProtocolError> {
        validate_string("app_id", &self.app_id)?;
        validate_string("event_type", &self.event_type)?;
        validate_data("data", &self.data)?;
        Ok(())
    }
}

impl Validate for StateQuery {
    fn validate(&self) -> Result<(), ProtocolError> {
        validate_string("app_id", &self.app_id)?;
        validate_string("query_type", &self.query_type)?;
        validate_data("params", &self.params)?;
        Ok(())
    }
}

impl Validate for StateResponse {
    fn validate(&self) -> Result<(), ProtocolError> {
        if let Some(ref data) = self.data {
            validate_data("data", data)?;
        }
        validate_optional_string("error", &self.error)?;
        Ok(())
    }
}

impl Validate for SyncState {
    fn validate(&self) -> Result<(), ProtocolError> {
        validate_string("content_id", &self.content_id)?;
        validate_finite_f64("playback_rate", self.playback_rate)?;
        Ok(())
    }
}

impl Validate for ErrorMessage {
    fn validate(&self) -> Result<(), ProtocolError> {
        validate_string("message", &self.message)?;
        Ok(())
    }
}

impl Validate for VideoLoad {
    fn validate(&self) -> Result<(), ProtocolError> {
        // Video source URLs can be longer than 256 bytes, use data limit
        validate_data("source", &self.source)?;
        Ok(())
    }
}

impl Validate for VideoState {
    fn validate(&self) -> Result<(), ProtocolError> {
        validate_string("content_id", &self.content_id)?;
        validate_finite_f64("playback_rate", self.playback_rate)?;
        validate_finite_f32("volume", self.volume)?;
        Ok(())
    }
}

impl Validate for VideoMetadata {
    fn validate(&self) -> Result<(), ProtocolError> {
        validate_string("content_id", &self.content_id)?;
        validate_finite_f32("fps", self.fps)?;
        validate_string("codec", &self.codec)?;
        validate_optional_string("title", &self.title)?;
        Ok(())
    }
}

impl Validate for VideoError {
    fn validate(&self) -> Result<(), ProtocolError> {
        validate_string("message", &self.message)?;
        Ok(())
    }
}

// Unit-like messages that need no validation
impl Validate for ClockPing {
    fn validate(&self) -> Result<(), ProtocolError> {
        Ok(())
    }
}

impl Validate for ClockPong {
    fn validate(&self) -> Result<(), ProtocolError> {
        Ok(())
    }
}

impl Validate for VideoPlay {
    fn validate(&self) -> Result<(), ProtocolError> {
        Ok(())
    }
}

impl Validate for VideoPause {
    fn validate(&self) -> Result<(), ProtocolError> {
        Ok(())
    }
}

impl Validate for VideoSeek {
    fn validate(&self) -> Result<(), ProtocolError> {
        Ok(())
    }
}

/// Decode a message from wire format with validation.
///
/// Like `decode()`, but also calls `validate()` on the deserialized payload.
pub fn decode_validated<T: for<'de> Deserialize<'de> + Validate>(
    bytes: &[u8],
) -> Result<(MessageType, T), ProtocolError> {
    let (msg_type, payload): (MessageType, T) = decode(bytes)?;
    if let Err(e) = payload.validate() {
        warn!(msg_type = ?msg_type, error = %e, "protocol validation failed");
        return Err(e);
    }
    Ok((msg_type, payload))
}

/// Encode a message to wire format using bitcode serialization.
pub fn encode<T: Serialize>(msg_type: MessageType, payload: &T) -> Result<Vec<u8>, ProtocolError> {
    let payload_bytes =
        bitcode::serialize(payload).map_err(|e| ProtocolError::Serialization(e.to_string()))?;

    if payload_bytes.len() > MAX_PAYLOAD_SIZE {
        return Err(ProtocolError::PayloadTooLarge {
            size: payload_bytes.len(),
            max: MAX_PAYLOAD_SIZE,
        });
    }

    let crc = crc32fast::hash(&payload_bytes);

    let header = Header {
        magic: MAGIC,
        version: VERSION,
        msg_type,
        payload_len: payload_bytes.len() as u32,
        crc32: crc,
    };

    let mut result = Vec::with_capacity(HEADER_SIZE + payload_bytes.len());
    result.extend_from_slice(&header.to_bytes());
    result.extend_from_slice(&payload_bytes);

    Ok(result)
}

/// Decode a message from wire format using bitcode deserialization.
///
/// Returns the message type and deserialized payload.
pub fn decode<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
) -> Result<(MessageType, T), ProtocolError> {
    let header = Header::from_bytes(bytes)?;

    let payload_start = HEADER_SIZE;
    let payload_end = payload_start + header.payload_len as usize;

    if bytes.len() < payload_end {
        return Err(ProtocolError::IncompletePayload {
            need: header.payload_len as usize,
            have: bytes.len() - HEADER_SIZE,
        });
    }

    let payload_bytes = &bytes[payload_start..payload_end];

    // Verify CRC
    let computed_crc = crc32fast::hash(payload_bytes);
    if computed_crc != header.crc32 {
        return Err(ProtocolError::CrcMismatch {
            expected: header.crc32,
            got: computed_crc,
        });
    }

    let payload: T = bitcode::deserialize(payload_bytes)
        .map_err(|e| ProtocolError::Serialization(e.to_string()))?;

    Ok((header.msg_type, payload))
}

/// Decode only the header (useful for routing without deserializing)
pub fn decode_header(bytes: &[u8]) -> Result<Header, ProtocolError> {
    Header::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_screen_rect() {
        let rect = ScreenRect {
            x: 100.0,
            y: 200.0,
            width: 640.0,
            height: 480.0,
            rotation: 0.0,
            visible: true,
        };

        let encoded = encode(MessageType::ScreenRect, &rect).unwrap();
        let (msg_type, decoded): (_, ScreenRect) = decode(&encoded).unwrap();

        assert_eq!(msg_type, MessageType::ScreenRect);
        assert_eq!(decoded.x, rect.x);
        assert_eq!(decoded.y, rect.y);
        assert_eq!(decoded.width, rect.width);
        assert_eq!(decoded.height, rect.height);
        assert_eq!(decoded.visible, rect.visible);
    }

    #[test]
    fn test_header_roundtrip() {
        let header = Header {
            magic: MAGIC,
            version: VERSION,
            msg_type: MessageType::StateSnapshot,
            payload_len: 1234,
            crc32: 0xDEADBEEF,
        };

        let bytes = header.to_bytes();
        let parsed = Header::from_bytes(&bytes).unwrap();

        assert_eq!(parsed.magic, header.magic);
        assert_eq!(parsed.version, header.version);
        assert_eq!(parsed.msg_type, header.msg_type);
        assert_eq!(parsed.payload_len, header.payload_len);
        assert_eq!(parsed.crc32, header.crc32);
    }

    #[test]
    fn test_invalid_magic() {
        let mut bytes = [0u8; HEADER_SIZE];
        bytes[0..4].copy_from_slice(b"NOPE");

        let result = Header::from_bytes(&bytes);
        assert!(matches!(result, Err(ProtocolError::InvalidMagic { .. })));
    }

    #[test]
    fn test_encode_decode_video_load() {
        let load = VideoLoad {
            source: "https://example.com/video.mp4".to_string(),
            start_position_ms: 5000,
            autoplay: true,
        };

        let encoded = encode(MessageType::VideoLoad, &load).unwrap();
        let (msg_type, decoded): (_, VideoLoad) = decode(&encoded).unwrap();

        assert_eq!(msg_type, MessageType::VideoLoad);
        assert_eq!(decoded.source, load.source);
        assert_eq!(decoded.start_position_ms, 5000);
        assert!(decoded.autoplay);
    }

    #[test]
    fn test_encode_decode_sync_state() {
        let sync = SyncState {
            content_id: "abc123".to_string(),
            position_at_ref_ms: 42000,
            ref_wallclock_ms: 1700000000000,
            is_playing: true,
            playback_rate: 1.0,
        };

        let encoded = encode(MessageType::SyncState, &sync).unwrap();
        let (msg_type, decoded): (_, SyncState) = decode(&encoded).unwrap();

        assert_eq!(msg_type, MessageType::SyncState);
        assert_eq!(decoded.content_id, "abc123");
        assert_eq!(decoded.position_at_ref_ms, 42000);
        assert!(decoded.is_playing);
    }

    // =========================================================================
    // Validation tests
    // =========================================================================

    #[test]
    fn test_validate_screen_rect_nan_rejected() {
        let rect = ScreenRect {
            x: f32::NAN,
            y: 200.0,
            width: 640.0,
            height: 480.0,
            rotation: 0.0,
            visible: true,
        };
        let encoded = encode(MessageType::ScreenRect, &rect).unwrap();
        let result: Result<(_, ScreenRect), _> = decode_validated(&encoded);
        assert!(matches!(
            result,
            Err(ProtocolError::ValidationFailed { .. })
        ));
    }

    #[test]
    fn test_validate_screen_rect_inf_rejected() {
        let rect = ScreenRect {
            x: 100.0,
            y: 200.0,
            width: f32::INFINITY,
            height: 480.0,
            rotation: 0.0,
            visible: true,
        };
        let encoded = encode(MessageType::ScreenRect, &rect).unwrap();
        let result: Result<(_, ScreenRect), _> = decode_validated(&encoded);
        assert!(matches!(
            result,
            Err(ProtocolError::ValidationFailed { .. })
        ));
    }

    #[test]
    fn test_validate_screen_rect_neg_inf_rejected() {
        let rect = ScreenRect {
            x: 100.0,
            y: f32::NEG_INFINITY,
            width: 640.0,
            height: 480.0,
            rotation: 0.0,
            visible: true,
        };
        let encoded = encode(MessageType::ScreenRect, &rect).unwrap();
        let result: Result<(_, ScreenRect), _> = decode_validated(&encoded);
        assert!(matches!(
            result,
            Err(ProtocolError::ValidationFailed { .. })
        ));
    }

    #[test]
    fn test_validate_screen_rect_valid() {
        let rect = ScreenRect {
            x: 100.0,
            y: 200.0,
            width: 640.0,
            height: 480.0,
            rotation: 0.0,
            visible: true,
        };
        let encoded = encode(MessageType::ScreenRect, &rect).unwrap();
        let result: Result<(_, ScreenRect), _> = decode_validated(&encoded);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_oversized_string_rejected() {
        let snapshot = StateSnapshot {
            app_id: "x".repeat(MAX_STRING_LENGTH + 1),
            timestamp_ms: 0,
            data: "{}".to_string(),
        };
        let encoded = encode(MessageType::StateSnapshot, &snapshot).unwrap();
        let result: Result<(_, StateSnapshot), _> = decode_validated(&encoded);
        assert!(matches!(
            result,
            Err(ProtocolError::ValidationFailed { .. })
        ));
    }

    #[test]
    fn test_validate_max_length_string_accepted() {
        let snapshot = StateSnapshot {
            app_id: "x".repeat(MAX_STRING_LENGTH),
            timestamp_ms: 0,
            data: "{}".to_string(),
        };
        let encoded = encode(MessageType::StateSnapshot, &snapshot).unwrap();
        let result: Result<(_, StateSnapshot), _> = decode_validated(&encoded);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_empty_string_accepted() {
        let snapshot = StateSnapshot {
            app_id: String::new(),
            timestamp_ms: 0,
            data: String::new(),
        };
        let encoded = encode(MessageType::StateSnapshot, &snapshot).unwrap();
        let result: Result<(_, StateSnapshot), _> = decode_validated(&encoded);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_oversized_data_rejected() {
        let snapshot = StateSnapshot {
            app_id: "test".to_string(),
            timestamp_ms: 0,
            data: "x".repeat(MAX_DATA_LENGTH + 1),
        };
        let encoded = encode(MessageType::StateSnapshot, &snapshot).unwrap();
        let result: Result<(_, StateSnapshot), _> = decode_validated(&encoded);
        assert!(matches!(
            result,
            Err(ProtocolError::ValidationFailed { .. })
        ));
    }

    #[test]
    fn test_validate_data_at_boundary_accepted() {
        let snapshot = StateSnapshot {
            app_id: "test".to_string(),
            timestamp_ms: 0,
            data: "x".repeat(MAX_DATA_LENGTH),
        };
        let encoded = encode(MessageType::StateSnapshot, &snapshot).unwrap();
        let result: Result<(_, StateSnapshot), _> = decode_validated(&encoded);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_video_state_nan_playback_rate() {
        let state = VideoState {
            content_id: "abc".to_string(),
            position_ms: 0,
            duration_ms: 0,
            is_playing: false,
            is_buffering: false,
            playback_rate: f64::NAN,
            volume: 0.5,
        };
        let encoded = encode(MessageType::VideoState, &state).unwrap();
        let result: Result<(_, VideoState), _> = decode_validated(&encoded);
        assert!(matches!(
            result,
            Err(ProtocolError::ValidationFailed { .. })
        ));
    }

    #[test]
    fn test_validate_sync_state_inf_rate() {
        let sync = SyncState {
            content_id: "abc".to_string(),
            position_at_ref_ms: 0,
            ref_wallclock_ms: 0,
            is_playing: true,
            playback_rate: f64::INFINITY,
        };
        let encoded = encode(MessageType::SyncState, &sync).unwrap();
        let result: Result<(_, SyncState), _> = decode_validated(&encoded);
        assert!(matches!(
            result,
            Err(ProtocolError::ValidationFailed { .. })
        ));
    }

    #[test]
    fn test_crc_validation() {
        let rect = ScreenRect {
            x: 100.0,
            y: 200.0,
            width: 640.0,
            height: 480.0,
            rotation: 0.0,
            visible: true,
        };

        let mut encoded = encode(MessageType::ScreenRect, &rect).unwrap();

        // Corrupt the payload
        if let Some(last) = encoded.last_mut() {
            *last ^= 0xFF;
        }

        let result: Result<(_, ScreenRect), _> = decode(&encoded);
        assert!(matches!(result, Err(ProtocolError::CrcMismatch { .. })));
    }
}
