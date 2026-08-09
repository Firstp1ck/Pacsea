//! Strict bounded LF-delimited JSONL framing and RPC command correlation.
//!
//! The Pi RPC contract is line-delimited JSON over stdin/stdout. Generic Unicode
//! line readers are forbidden: records are separated by `\n` only, one optional
//! trailing `\r` is stripped for tolerance, and every other separator-looking
//! character stays inside the record where the JSON parser must account for it.
//!
//! Every record is additionally length-bounded, must decode as strict UTF-8, must
//! be a single JSON object, must not contain duplicate object keys, and must not
//! exceed the compiled nesting bound.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;

use serde_json::Value;

use super::limits;

/// What: Failure modes of the strict Pi RPC framing and JSON contract.
///
/// Inputs: Produced by [`LineFramer`], [`decode_record`], and [`parse_strict_json`].
///
/// Output: Implements `Display`/`Error` with the exact rejected condition.
///
/// Details:
/// - Every variant is terminal for the affected record; the caller must not retry
///   parsing the same bytes with a laxer reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// A pending or complete record exceeded the compiled byte bound.
    RecordTooLarge {
        /// Observed byte length (or the bound at which scanning stopped).
        observed: usize,
        /// Compiled maximum record length.
        limit: usize,
    },
    /// The record was not valid UTF-8.
    NotUtf8 {
        /// Byte offset of the first invalid sequence.
        offset: usize,
    },
    /// The record was empty or contained only the optional trailing carriage return.
    EmptyRecord,
    /// The record contained a bare carriage return, which is not a Pi record separator.
    EmbeddedCarriageReturn,
    /// The record was not syntactically valid JSON.
    InvalidJson {
        /// Serde diagnostic for the failing record.
        reason: String,
    },
    /// The record parsed but was not a JSON object.
    NotAnObject,
    /// The record contained a duplicate object key.
    DuplicateKey {
        /// Key observed more than once in the same object.
        key: String,
    },
    /// The record exceeded the compiled JSON nesting bound.
    TooDeep {
        /// Compiled maximum depth.
        limit: usize,
    },
    /// Trailing content followed the first complete JSON value.
    TrailingContent,
    /// A response referenced a command id that was never issued or already settled.
    UnknownCommandId {
        /// Correlation id carried by the offending response.
        id: String,
    },
    /// A command id was reused while the previous command was still pending.
    DuplicateCommandId {
        /// Correlation id that collided.
        id: String,
    },
    /// A response omitted the mandatory correlation id.
    MissingCommandId,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecordTooLarge { observed, limit } => write!(
                f,
                "Pi RPC record of at least {observed} bytes exceeds the {limit}-byte limit; \
                 the scan was stopped instead of buffering unbounded agent output"
            ),
            Self::NotUtf8 { offset } => write!(
                f,
                "Pi RPC record is not valid UTF-8 at byte {offset}; the record was discarded"
            ),
            Self::EmptyRecord => write!(f, "Pi RPC record was empty"),
            Self::EmbeddedCarriageReturn => write!(
                f,
                "Pi RPC record contains a bare carriage return; only a single trailing \
                 carriage return before the line feed is tolerated"
            ),
            Self::InvalidJson { reason } => write!(f, "Pi RPC record is not valid JSON: {reason}"),
            Self::NotAnObject => write!(f, "Pi RPC record is not a JSON object"),
            Self::DuplicateKey { key } => {
                write!(f, "Pi RPC record repeats the JSON key {key:?}")
            }
            Self::TooDeep { limit } => {
                write!(f, "Pi RPC record nests deeper than the {limit}-level limit")
            }
            Self::TrailingContent => write!(
                f,
                "Pi RPC record contains trailing content after the first JSON value"
            ),
            Self::UnknownCommandId { id } => write!(
                f,
                "Pi RPC response references unknown or already settled command id {id:?}"
            ),
            Self::DuplicateCommandId { id } => {
                write!(f, "Pi RPC command id {id:?} is already pending")
            }
            Self::MissingCommandId => {
                write!(f, "Pi RPC response is missing its correlation id")
            }
        }
    }
}

impl std::error::Error for ProtocolError {}

/// What: Incremental strict LF record framer for the Pi stdout stream.
///
/// Inputs: Byte chunks appended with [`LineFramer::push`].
///
/// Output: Complete records via [`LineFramer::next_record`].
///
/// Details:
/// - Records are split on `\n` only. A single trailing `\r` immediately before the
///   line feed is stripped; any other carriage return keeps the record and is
///   rejected by [`decode_record`].
/// - The pending buffer is hard-bounded, so a Pi process that never emits a line
///   feed cannot exhaust memory. Once the bound trips the framer is poisoned and
///   returns the same error for every later call.
#[derive(Debug)]
pub struct LineFramer {
    /// Bytes received since the last complete record.
    pending: Vec<u8>,
    /// Compiled maximum record length in bytes, excluding the separator.
    limit: usize,
    /// Set once the bound was exceeded; the stream can no longer be resynchronized.
    poisoned: bool,
}

impl Default for LineFramer {
    fn default() -> Self {
        Self::new(limits::MAX_RPC_RECORD_BYTES)
    }
}

impl LineFramer {
    /// What: Create a framer with an explicit byte bound.
    ///
    /// Inputs:
    /// - `limit`: Maximum record length in bytes, excluding the `\n` separator.
    ///
    /// Output:
    /// - An empty framer.
    ///
    /// Details:
    /// - Tests use small bounds; production uses [`limits::MAX_RPC_RECORD_BYTES`].
    #[must_use]
    pub const fn new(limit: usize) -> Self {
        Self {
            pending: Vec::new(),
            limit,
            poisoned: false,
        }
    }

    /// What: Append received bytes to the pending record buffer.
    ///
    /// Inputs:
    /// - `chunk`: Bytes read from the Pi stdout pipe.
    ///
    /// Output:
    /// - `Ok(())`, or [`ProtocolError::RecordTooLarge`] once the bound is exceeded.
    ///
    /// Details:
    /// - The bound is checked against the longest incomplete record in the buffer so
    ///   a single oversized record is rejected before it is fully received.
    ///
    /// # Errors
    /// - Returns `Err` when an incomplete record already exceeds the byte bound.
    pub fn push(&mut self, chunk: &[u8]) -> Result<(), ProtocolError> {
        if self.poisoned {
            return Err(ProtocolError::RecordTooLarge {
                observed: self.pending.len(),
                limit: self.limit,
            });
        }
        self.pending.extend_from_slice(chunk);
        let mut segment_start = 0usize;
        for index in self
            .pending
            .iter()
            .enumerate()
            .filter_map(|(index, byte)| (*byte == b'\n').then_some(index))
            .chain(std::iter::once(self.pending.len()))
        {
            let observed = index.saturating_sub(segment_start);
            if observed > self.limit {
                self.poisoned = true;
                self.pending.clear();
                return Err(ProtocolError::RecordTooLarge {
                    observed,
                    limit: self.limit,
                });
            }
            segment_start = index.saturating_add(1);
        }
        Ok(())
    }

    /// What: Take the next complete record, if one is buffered.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - `Some(bytes)` without the `\n` separator and without one optional trailing `\r`.
    /// - `None` when no complete record is buffered.
    ///
    /// Details:
    /// - Empty records are returned as empty slices so [`decode_record`] can reject
    ///   them explicitly rather than letting a caller skip them silently.
    pub fn next_record(&mut self) -> Option<Vec<u8>> {
        let index = self.pending.iter().position(|byte| *byte == b'\n')?;
        let mut record: Vec<u8> = self.pending.drain(..=index).collect();
        record.pop();
        if record.last() == Some(&b'\r') {
            record.pop();
        }
        Some(record)
    }

    /// What: Report bytes buffered for the current incomplete record.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Pending byte count.
    ///
    /// Details:
    /// - Used by shutdown handling to report truncated trailing output.
    #[must_use]
    pub const fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

/// What: Decode one framed record into a strict JSON object.
///
/// Inputs:
/// - `record`: Record bytes without the separator and without one trailing `\r`.
///
/// Output:
/// - The parsed object, or the exact rejection reason.
///
/// Details:
/// - Rejects non-UTF-8 bytes, empty records, bare carriage returns, non-object
///   values, duplicate keys, excessive nesting, and trailing content.
///
/// # Errors
/// - Returns `Err` for every condition listed above; there is no lenient fallback.
pub fn decode_record(record: &[u8]) -> Result<serde_json::Map<String, Value>, ProtocolError> {
    if record.len() > limits::MAX_RPC_RECORD_BYTES {
        return Err(ProtocolError::RecordTooLarge {
            observed: record.len(),
            limit: limits::MAX_RPC_RECORD_BYTES,
        });
    }
    if record.is_empty() {
        return Err(ProtocolError::EmptyRecord);
    }
    let text = std::str::from_utf8(record).map_err(|error| ProtocolError::NotUtf8 {
        offset: error.valid_up_to(),
    })?;
    if text.contains('\r') {
        return Err(ProtocolError::EmbeddedCarriageReturn);
    }
    match parse_strict_json(text, limits::MAX_JSON_DEPTH)? {
        Value::Object(map) => Ok(map),
        _ => Err(ProtocolError::NotAnObject),
    }
}

/// What: Parse exactly one JSON value with duplicate-key, depth, and trailing-content checks.
///
/// Inputs:
/// - `text`: Candidate JSON document.
/// - `max_depth`: Maximum accepted container nesting.
///
/// Output:
/// - The parsed value, or the exact rejection reason.
///
/// Details:
/// - `serde_json` keeps the last value for duplicate keys by default, which would let
///   hostile output hide a second assessment behind an innocuous first one. This parser
///   walks the raw token stream and rejects the document instead.
/// - Trailing content after the first value is rejected, so "multiple final answers"
///   can never be silently reduced to the first object.
///
/// # Errors
/// - Returns `Err` for syntax errors, duplicate keys, excessive depth, or trailing content.
pub fn parse_strict_json(text: &str, max_depth: usize) -> Result<Value, ProtocolError> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = StrictValue::deserialize_with_depth(&mut deserializer, max_depth)?;
    deserializer.end().map_err(|error| {
        if error.is_eof() || error.is_syntax() {
            ProtocolError::TrailingContent
        } else {
            ProtocolError::InvalidJson {
                reason: error.to_string(),
            }
        }
    })?;
    Ok(value)
}

/// What: Depth- and duplicate-aware JSON value deserializer.
///
/// Inputs: A `serde_json` deserializer plus the remaining depth budget.
///
/// Output: A plain [`Value`] once every strictness check passed.
///
/// Details:
/// - Implemented as a private helper type so the visitor can carry the depth budget
///   without leaking a public generic API.
struct StrictValue;

impl StrictValue {
    /// What: Deserialize one value while enforcing depth and duplicate-key limits.
    ///
    /// Inputs:
    /// - `deserializer`: Positioned `serde_json` deserializer.
    /// - `max_depth`: Remaining container nesting budget.
    ///
    /// Output:
    /// - The parsed value or the exact rejection reason.
    ///
    /// Details:
    /// - Errors raised inside the visitor are re-mapped from serde's custom-error
    ///   channel back into typed [`ProtocolError`] values by [`classify_serde_error`].
    fn deserialize_with_depth<'de, D>(
        deserializer: D,
        max_depth: usize,
    ) -> Result<Value, ProtocolError>
    where
        D: serde::Deserializer<'de>,
        D::Error: fmt::Display,
    {
        serde::Deserializer::deserialize_any(deserializer, StrictVisitor { depth: max_depth })
            .map_err(|error| classify_serde_error(&error.to_string(), max_depth))
    }
}

/// Sentinel prefix used to smuggle typed rejections through serde's error channel.
const STRICT_DUPLICATE_PREFIX: &str = "pacsea-strict-duplicate-key:";

/// Sentinel marker used to smuggle depth rejections through serde's error channel.
const STRICT_DEPTH_MARKER: &str = "pacsea-strict-depth-exceeded";

/// What: Map a serde error string back to a typed protocol rejection.
///
/// Inputs:
/// - `message`: Serde error rendering.
///
/// Output:
/// - The matching [`ProtocolError`].
///
/// Details:
/// - Serde visitors can only raise `Error::custom`, so the strict visitor encodes its
///   typed rejections with unique sentinels that are decoded here.
fn classify_serde_error(message: &str, max_depth: usize) -> ProtocolError {
    if let Some(start) = message.find(STRICT_DUPLICATE_PREFIX) {
        let rest = &message[start + STRICT_DUPLICATE_PREFIX.len()..];
        if let Some((length, payload)) = rest.split_once(':')
            && let Ok(length) = length.parse::<usize>()
            && let Some(key) = payload.get(..length)
        {
            return ProtocolError::DuplicateKey {
                key: key.to_string(),
            };
        }
    }
    if message.contains(STRICT_DEPTH_MARKER) {
        return ProtocolError::TooDeep { limit: max_depth };
    }
    ProtocolError::InvalidJson {
        reason: message.to_string(),
    }
}

/// What: Serde visitor that enforces nesting depth and rejects duplicate object keys.
///
/// Inputs: Carries the remaining depth budget.
///
/// Output: Plain [`Value`] trees.
///
/// Details:
/// - Depth is decremented per container; reaching zero inside a container rejects the
///   document rather than truncating it.
struct StrictVisitor {
    /// Remaining container nesting budget.
    depth: usize,
}

impl<'de> serde::de::Visitor<'de> for StrictVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a strict JSON value without duplicate keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
        Ok(serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number))
    }

    fn visit_str<E>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_string()))
    }

    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        serde::Deserializer::deserialize_any(deserializer, self)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let Some(depth) = self.depth.checked_sub(1) else {
            return Err(serde::de::Error::custom(STRICT_DEPTH_MARKER));
        };
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(StrictSeed { depth })? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let Some(depth) = self.depth.checked_sub(1) else {
            return Err(serde::de::Error::custom(STRICT_DEPTH_MARKER));
        };
        let mut ordered: BTreeMap<String, Value> = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            let value = map.next_value_seed(StrictSeed { depth })?;
            match ordered.entry(key) {
                Entry::Occupied(existing) => {
                    return Err(serde::de::Error::custom(format!(
                        "{STRICT_DUPLICATE_PREFIX}{}:{}",
                        existing.key().len(),
                        existing.key()
                    )));
                }
                Entry::Vacant(slot) => {
                    slot.insert(value);
                }
            }
        }
        Ok(Value::Object(ordered.into_iter().collect()))
    }
}

/// What: Seed that threads the remaining depth budget into nested values.
///
/// Inputs: Remaining container nesting budget.
///
/// Output: Nested [`Value`] trees under the same strictness rules.
///
/// Details:
/// - Required because `serde` seeds, not visitors, carry state into nested elements.
struct StrictSeed {
    /// Remaining container nesting budget for the nested value.
    depth: usize,
}

impl<'de> serde::de::DeserializeSeed<'de> for StrictSeed {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        serde::Deserializer::deserialize_any(deserializer, StrictVisitor { depth: self.depth })
    }
}

/// What: Pi RPC command names the scanner requires before enabling behavior.
///
/// Inputs: None; this is the compiled contract.
///
/// Output: Sorted command name list.
///
/// Details:
/// - `capabilities` verifies each name is advertised before the corresponding runtime
///   behavior (fallback, accounting, cancellation) is allowed to run.
pub const REQUIRED_RPC_COMMANDS: [&str; 10] = [
    "abort",
    "abort_retry",
    "agent_settled",
    "get_available_models",
    "get_commands",
    "get_last_assistant_text",
    "get_session_stats",
    "get_state",
    "set_auto_retry",
    "set_model",
];

/// What: Correlates outbound Pi RPC commands with inbound responses.
///
/// Inputs: Command names registered through [`CommandCorrelator::issue`].
///
/// Output: Settled command names from [`CommandCorrelator::settle`].
///
/// Details:
/// - Ids are `pacsea-<sequence>` and are never reused inside one session, so a late
///   response from an aborted command cannot be mistaken for a fresh answer.
/// - Unknown, missing, and already-settled ids are hard errors rather than warnings.
#[derive(Debug, Default)]
pub struct CommandCorrelator {
    /// Monotonic id sequence for this session.
    sequence: u64,
    /// Currently outstanding command ids mapped to their command names.
    pending: BTreeMap<String, String>,
}

impl CommandCorrelator {
    /// What: Create an empty correlator.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - A correlator whose first issued id is `pacsea-1`.
    ///
    /// Details:
    /// - One correlator belongs to exactly one Pi process.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sequence: 0,
            pending: BTreeMap::new(),
        }
    }

    /// What: Allocate a fresh correlation id and record the outstanding command.
    ///
    /// Inputs:
    /// - `command`: Pi RPC command name being sent.
    ///
    /// Output:
    /// - The allocated id.
    ///
    /// Details:
    /// - Ids are strictly increasing, so replayed responses always fail correlation.
    ///
    /// # Errors
    /// - Returns `Err` only if the id sequence somehow collides with a pending id.
    pub fn issue(&mut self, command: &str) -> Result<String, ProtocolError> {
        self.sequence = self.sequence.saturating_add(1);
        let id = format!("pacsea-{}", self.sequence);
        match self.pending.entry(id.clone()) {
            Entry::Occupied(_) => Err(ProtocolError::DuplicateCommandId { id }),
            Entry::Vacant(slot) => {
                slot.insert(command.to_string());
                Ok(id)
            }
        }
    }

    /// What: Settle an inbound response against its outstanding command.
    ///
    /// Inputs:
    /// - `record`: Decoded RPC object.
    ///
    /// Output:
    /// - The command name the response belongs to.
    ///
    /// Details:
    /// - Records without an `id` are rejected. Callers must route unsolicited event
    ///   records through [`CommandCorrelator::is_event`] before settling.
    ///
    /// # Errors
    /// - Returns `Err` when the id is missing, not a string, unknown, or already settled.
    pub fn settle(
        &mut self,
        record: &serde_json::Map<String, Value>,
    ) -> Result<String, ProtocolError> {
        let id = record
            .get("id")
            .and_then(Value::as_str)
            .ok_or(ProtocolError::MissingCommandId)?;
        self.pending
            .remove(id)
            .ok_or_else(|| ProtocolError::UnknownCommandId { id: id.to_string() })
    }

    /// What: Report whether a decoded record is an unsolicited Pi event.
    ///
    /// Inputs:
    /// - `record`: Decoded RPC object.
    ///
    /// Output:
    /// - `true` when the record carries no correlation id.
    ///
    /// Details:
    /// - Pi emits notifications such as `agent_settled` without a command id; those
    ///   are handled by the scan loop rather than by correlation.
    #[must_use]
    pub fn is_event(record: &serde_json::Map<String, Value>) -> bool {
        !record.contains_key("id")
    }

    /// What: Report the number of outstanding commands.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Pending command count.
    ///
    /// Details:
    /// - Cancellation drops every pending command without settling it.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// What: Drop every outstanding command after an abort.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - No return value; later responses for the dropped ids fail correlation.
    ///
    /// Details:
    /// - Ids are never recycled, so clearing cannot make a stale response valid again.
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

/// What: Serialize an RPC command to a single strict LF-terminated record.
///
/// Inputs:
/// - `id`: Correlation id from [`CommandCorrelator::issue`].
/// - `command`: Pi RPC command name.
/// - `fields`: Additional command fields.
///
/// Output:
/// - Encoded bytes ending in exactly one `\n`.
///
/// Details:
/// - Rejects any encoding whose serialization would contain an embedded control
///   character outside a JSON escape, and enforces the record byte bound before send.
///
/// # Errors
/// - Returns `Err` when the encoded record would exceed the compiled byte bound.
pub fn encode_command(
    id: &str,
    command: &str,
    fields: &serde_json::Map<String, Value>,
) -> Result<Vec<u8>, ProtocolError> {
    let mut object = serde_json::Map::new();
    object.insert("id".to_string(), Value::String(id.to_string()));
    object.insert("type".to_string(), Value::String(command.to_string()));
    for (key, value) in fields {
        if key == "id" || key == "type" {
            continue;
        }
        object.insert(key.clone(), value.clone());
    }
    let mut encoded =
        serde_json::to_vec(&Value::Object(object)).map_err(|error| ProtocolError::InvalidJson {
            reason: error.to_string(),
        })?;
    if encoded.len() >= limits::MAX_RPC_RECORD_BYTES {
        return Err(ProtocolError::RecordTooLarge {
            observed: encoded.len(),
            limit: limits::MAX_RPC_RECORD_BYTES,
        });
    }
    encoded.push(b'\n');
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::{
        CommandCorrelator, LineFramer, ProtocolError, REQUIRED_RPC_COMMANDS, decode_record,
        encode_command, parse_strict_json,
    };
    use serde_json::Value;

    /// Drain every currently complete record from a framer.
    fn drain(framer: &mut LineFramer) -> Vec<Vec<u8>> {
        let mut records = Vec::new();
        while let Some(record) = framer.next_record() {
            records.push(record);
        }
        records
    }

    /// Verify LF splitting, single trailing CR stripping, and partial buffering.
    #[test]
    fn framer_splits_on_line_feed_only() {
        let mut framer = LineFramer::new(1024);
        framer
            .push(b"{\"a\":1}\r\n{\"b\":2}\n{\"c\"")
            .expect("bounded");
        let records = drain(&mut framer);
        assert_eq!(records, vec![b"{\"a\":1}".to_vec(), b"{\"b\":2}".to_vec()]);
        assert_eq!(framer.pending_len(), 4);
    }

    /// Verify a bare CR does not terminate a record and is rejected on decode.
    #[test]
    fn bare_carriage_return_is_not_a_separator() {
        let mut framer = LineFramer::new(1024);
        framer.push(b"{\"a\":1}\r{\"b\":2}\n").expect("bounded");
        let records = drain(&mut framer);
        assert_eq!(records.len(), 1);
        assert_eq!(
            decode_record(&records[0]),
            Err(ProtocolError::EmbeddedCarriageReturn)
        );
    }

    /// Verify Unicode separators never split records.
    #[test]
    fn unicode_separators_do_not_split_records() {
        let mut framer = LineFramer::new(1024);
        framer
            .push("{\"a\":\"x\u{2028}y\u{2029}z\"}\n".as_bytes())
            .expect("bounded");
        let records = drain(&mut framer);
        assert_eq!(records.len(), 1);
        let decoded = decode_record(&records[0]).expect("separators stay inside the JSON string");
        assert_eq!(decoded["a"], Value::String("x\u{2028}y\u{2029}z".into()));
    }

    /// Verify the framer refuses to buffer an unbounded record and stays poisoned.
    #[test]
    fn oversized_pending_record_is_rejected_and_poisons_the_framer() {
        let mut framer = LineFramer::new(8);
        let error = framer.push(&[b'x'; 9]).expect_err("must exceed the bound");
        assert_eq!(
            error,
            ProtocolError::RecordTooLarge {
                observed: 9,
                limit: 8
            }
        );
        assert!(framer.push(b"\n").is_err(), "framer must stay poisoned");
        assert!(framer.next_record().is_none());
    }

    /// Verify an oversized record that arrives in small chunks is still rejected.
    #[test]
    fn oversized_record_is_rejected_across_chunks() {
        let mut framer = LineFramer::new(8);
        framer.push(b"xxxx").expect("under bound");
        assert!(framer.push(b"yyyyy").is_err(), "combined length exceeds 8");
    }

    /// Verify a complete oversized line cannot hide before a short trailing record.
    #[test]
    fn oversized_complete_record_is_rejected_before_buffering() {
        let mut framer = LineFramer::new(8);
        let error = framer
            .push(b"123456789\n{}\n")
            .expect_err("the first record exceeds the bound");
        assert_eq!(
            error,
            ProtocolError::RecordTooLarge {
                observed: 9,
                limit: 8
            }
        );
        assert!(framer.next_record().is_none());
    }

    /// Verify invalid UTF-8, empty records, and non-objects are rejected.
    #[test]
    fn malformed_records_are_rejected() {
        assert_eq!(decode_record(b""), Err(ProtocolError::EmptyRecord));
        assert_eq!(
            decode_record(&[b'{', 0xff, b'}']),
            Err(ProtocolError::NotUtf8 { offset: 1 })
        );
        assert_eq!(decode_record(b"[1,2]"), Err(ProtocolError::NotAnObject));
        assert!(matches!(
            decode_record(b"{\"a\":}"),
            Err(ProtocolError::InvalidJson { .. })
        ));
    }

    /// Verify duplicate keys are rejected instead of last-write-wins.
    #[test]
    fn duplicate_keys_are_rejected() {
        let error = decode_record(b"{\"severity\":\"low\",\"severity\":\"critical\"}")
            .expect_err("duplicate keys must fail");
        assert_eq!(
            error,
            ProtocolError::DuplicateKey {
                key: "severity".to_string()
            }
        );
    }

    /// Verify duplicate keys nested inside arrays and objects are also rejected.
    #[test]
    fn nested_duplicate_keys_are_rejected() {
        let error = decode_record(b"{\"a\":{\"b\":[{\"c\":1,\"c\":2}]}}")
            .expect_err("nested duplicate keys must fail");
        assert_eq!(
            error,
            ProtocolError::DuplicateKey {
                key: "c".to_string()
            }
        );
    }

    /// Verify trailing objects after the first value are rejected.
    #[test]
    fn trailing_objects_are_rejected() {
        assert_eq!(
            decode_record(b"{\"a\":1}{\"b\":2}"),
            Err(ProtocolError::TrailingContent)
        );
        assert_eq!(
            decode_record(b"{\"a\":1} garbage"),
            Err(ProtocolError::TrailingContent)
        );
    }

    /// Verify nesting beyond the configured budget is rejected.
    #[test]
    fn excessive_nesting_is_rejected() {
        let deep = format!("{}1{}", "[".repeat(40), "]".repeat(40));
        assert!(matches!(
            parse_strict_json(&deep, 32),
            Err(ProtocolError::TooDeep { .. })
        ));
        let shallow = format!("{}1{}", "[".repeat(4), "]".repeat(4));
        assert!(parse_strict_json(&shallow, 32).is_ok());
        assert_eq!(
            parse_strict_json("[[1]]", 1),
            Err(ProtocolError::TooDeep { limit: 1 })
        );
    }

    /// Verify correlation rejects unknown, missing, and replayed ids.
    #[test]
    fn correlation_rejects_unknown_and_replayed_ids() {
        let mut correlator = CommandCorrelator::new();
        let id = correlator.issue("get_state").expect("id");
        assert_eq!(id, "pacsea-1");
        assert_eq!(correlator.pending_len(), 1);

        let record = decode_record(b"{\"id\":\"pacsea-1\",\"data\":{}}").expect("valid");
        assert_eq!(correlator.settle(&record).expect("settles"), "get_state");
        assert_eq!(
            correlator.settle(&record),
            Err(ProtocolError::UnknownCommandId {
                id: "pacsea-1".to_string()
            })
        );

        let foreign = decode_record(b"{\"id\":\"attacker\",\"data\":{}}").expect("valid");
        assert!(correlator.settle(&foreign).is_err());

        let event = decode_record(b"{\"type\":\"agent_settled\"}").expect("valid");
        assert!(CommandCorrelator::is_event(&event));
        assert_eq!(
            correlator.settle(&event),
            Err(ProtocolError::MissingCommandId)
        );
    }

    /// Verify cleared commands can never be settled by a late response.
    #[test]
    fn cleared_commands_cannot_be_settled_later() {
        let mut correlator = CommandCorrelator::new();
        let id = correlator.issue("prompt").expect("id");
        correlator.clear();
        let record =
            decode_record(format!("{{\"id\":\"{id}\",\"data\":{{}}}}").as_bytes()).expect("valid");
        assert!(correlator.settle(&record).is_err());
        // Ids are never recycled, so a second command cannot reuse the aborted id.
        assert_eq!(correlator.issue("prompt").expect("id"), "pacsea-2");
    }

    /// Verify encoding produces exactly one LF-terminated record with fixed control fields.
    #[test]
    fn encoding_produces_one_lf_terminated_record() {
        let mut fields = serde_json::Map::new();
        fields.insert("message".to_string(), Value::String("hello".into()));
        fields.insert("type".to_string(), Value::String("spoofed".into()));
        let encoded = encode_command("pacsea-7", "prompt", &fields).expect("encodes");
        assert_eq!(encoded.last(), Some(&b'\n'));
        assert!(!encoded[..encoded.len() - 1].contains(&b'\n'));
        let decoded = decode_record(&encoded[..encoded.len() - 1]).expect("round trips");
        assert_eq!(decoded["type"], Value::String("prompt".into()));
        assert_eq!(decoded["id"], Value::String("pacsea-7".into()));
    }

    /// Verify newlines inside payloads are escaped and never re-frame the record.
    #[test]
    fn encoded_payload_newlines_are_escaped() {
        let mut fields = serde_json::Map::new();
        fields.insert(
            "message".to_string(),
            Value::String("line1\n{\"id\":\"pacsea-1\"}".into()),
        );
        let encoded = encode_command("pacsea-1", "prompt", &fields).expect("encodes");
        assert!(!encoded[..encoded.len() - 1].contains(&b'\n'));
    }

    /// Verify the required RPC command inventory stays sorted and unique.
    #[test]
    fn required_rpc_commands_are_sorted_and_unique() {
        let mut sorted = REQUIRED_RPC_COMMANDS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, REQUIRED_RPC_COMMANDS.to_vec());
    }
}
