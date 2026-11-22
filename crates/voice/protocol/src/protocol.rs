// Derived from openai/codex 943a21829d (codex-rs/protocol/src/protocol.rs), Apache-2.0.
// Modified for Hex: only the realtime types below, without JsonSchema or TS derives.
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

/// A caller-owned realtime sideband. Dropping either direction seals input and ends
/// the attachment after accepted messages drain. The caller supplies bounded channels
/// and one complete JSON message per item.
#[derive(Debug)]
pub struct RealtimeSidebandTransport {
    pub incoming: tokio::sync::mpsc::Receiver<String>,
    pub outgoing: tokio::sync::mpsc::Sender<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RealtimeOutputModality {
    Text,
    Audio,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum RealtimeVoice {
    Alloy,
    Arbor,
    Ash,
    Aube,
    Ballad,
    Breeze,
    Bubbie,
    Cedar,
    Coral,
    Cove,
    Echo,
    Ember,
    Haetsal,
    Hanul,
    Himari,
    Juniper,
    Leher,
    Maple,
    Marin,
    Miko,
    Neer,
    Porto,
    Rindo,
    Rio,
    Rivage,
    Sage,
    Selva,
    Shimmer,
    Sol,
    Sonna,
    Spruce,
    Tinta,
    Vale,
    Verse,
    Viola,
}

impl RealtimeVoice {
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::Alloy => "alloy",
            Self::Arbor => "arbor",
            Self::Ash => "ash",
            Self::Aube => "aube",
            Self::Ballad => "ballad",
            Self::Breeze => "breeze",
            Self::Bubbie => "bubbie",
            Self::Cedar => "cedar",
            Self::Coral => "coral",
            Self::Cove => "cove",
            Self::Echo => "echo",
            Self::Ember => "ember",
            Self::Haetsal => "haetsal",
            Self::Hanul => "hanul",
            Self::Himari => "himari",
            Self::Juniper => "juniper",
            Self::Leher => "leher",
            Self::Maple => "maple",
            Self::Marin => "marin",
            Self::Miko => "miko",
            Self::Neer => "neer",
            Self::Porto => "porto",
            Self::Rindo => "rindo",
            Self::Rio => "rio",
            Self::Rivage => "rivage",
            Self::Sage => "sage",
            Self::Selva => "selva",
            Self::Shimmer => "shimmer",
            Self::Sol => "sol",
            Self::Sonna => "sonna",
            Self::Spruce => "spruce",
            Self::Tinta => "tinta",
            Self::Vale => "vale",
            Self::Verse => "verse",
            Self::Viola => "viola",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RealtimeAudioFrame {
    pub data: String,
    pub sample_rate: u32,
    pub num_channels: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub samples_per_channel: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RealtimeTranscriptDelta {
    pub delta: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RealtimeTranscriptDone {
    pub text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RealtimeTranscriptEntry {
    pub role: String,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RealtimeHandoffRequested {
    pub handoff_id: String,
    pub item_id: String,
    pub input_transcript: String,
    pub active_transcript: Vec<RealtimeTranscriptEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RealtimeNoopRequested {
    pub call_id: String,
    pub item_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RealtimeInputAudioSpeechStarted {
    pub item_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RealtimeResponseCancelled {
    pub response_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RealtimeResponseCreated {
    pub response_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RealtimeResponseDone {
    pub response_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum RealtimeEvent {
    SessionUpdated {
        realtime_session_id: String,
        instructions: Option<String>,
    },
    InputAudioSpeechStarted(RealtimeInputAudioSpeechStarted),
    InputTranscriptDelta(RealtimeTranscriptDelta),
    InputTranscriptDone(RealtimeTranscriptDone),
    OutputTranscriptDelta(RealtimeTranscriptDelta),
    OutputTranscriptDone(RealtimeTranscriptDone),
    AudioOut(RealtimeAudioFrame),
    ResponseCreated(RealtimeResponseCreated),
    ResponseCancelled(RealtimeResponseCancelled),
    ResponseDone(RealtimeResponseDone),
    ConversationItemAdded(Value),
    ConversationItemDone {
        item_id: String,
    },
    /// Canonical display history produced by Core, separate from provider events.
    HistoryItemStarted(crate::realtime::RealtimeItem),
    HistoryTranscriptDelta {
        item_id: String,
        delta: String,
    },
    HistoryItemCompleted(crate::realtime::RealtimeItem),
    HandoffRequested(RealtimeHandoffRequested),
    NoopRequested(RealtimeNoopRequested),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationTextParams {
    pub text: String,
    pub role: ConversationTextRole,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationTextRole {
    #[default]
    User,
    Developer,
    Assistant,
}
