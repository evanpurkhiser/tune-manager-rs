use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

pub mod state;
pub mod runner;
pub mod stages;

/// Represents the different stages of the processing pipeline
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize)]
pub enum ProcessingStage {
    /// Converts media to supported format (AIFF/MP3), ensures ID3v2.4 tags, and computes media hash
    PrepareMedia,

    /// Detects the musical key of the audio using keyfinder-cli
    Keyfinder,

    /// Extracts Beatport URL from WOAF tags and fetches track metadata from Beatport API
    Beatport,

    /// Uses AI to clean up and normalize track metadata
    Ai,
}

