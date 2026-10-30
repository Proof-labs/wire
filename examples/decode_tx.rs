//! Decode each hex vector file by its kind; exits non-zero naming every file
//! that fails. The kind is the file's directory: `events/<event type>.hex` is
//! an `Event` that must render as that ABCI event type,
//! `responses/<wire type>.hex` is that read-model response, and any other file
//! is a signed transaction.

use std::path::Path;
use std::process::ExitCode;

use proof_wire::abci_event::AbciEventWriter;
use proof_wire::query::{AccountInfo, PositionBrief};
use proof_wire::types::{Event, EventInfo, MarketConfig};
use serde::de::DeserializeOwned;

fn decode_as<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    rmp_serde::from_slice(bytes).map_err(|e| e.to_string())
}

fn decode_event(bytes: &[u8], event_type: &str) -> Result<(), String> {
    let event: Event = decode_as(bytes)?;
    let mut writer = AbciEventWriter::new();
    event.encode_abci(&mut writer);
    let rendered = writer.into_vec();
    let length = rendered
        .get(..2)
        .map(|prefix| usize::from(u16::from_le_bytes([prefix[0], prefix[1]])))
        .ok_or("event rendered no type")?;
    let rendered_type = rendered
        .get(2..)
        .and_then(|rest| rest.get(..length))
        .ok_or("event type truncated")?;
    if rendered_type == event_type.as_bytes() {
        Ok(())
    } else {
        Err(format!(
            "decodes as {}, not {event_type}",
            String::from_utf8_lossy(rendered_type)
        ))
    }
}

fn decode_response(bytes: &[u8], wire_type: &str) -> Result<(), String> {
    match wire_type {
        "account_info" => decode_as::<AccountInfo>(bytes).map(|_| ()),
        "event_info" => decode_as::<EventInfo>(bytes).map(|_| ()),
        "market_config" => decode_as::<MarketConfig>(bytes).map(|_| ()),
        "position_brief" => decode_as::<PositionBrief>(bytes).map(|_| ()),
        other => Err(format!("no response type {other}")),
    }
}

fn decode(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let kind = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str());
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or("vector name is not UTF-8")?;
    match kind {
        Some("events") => decode_event(bytes, name),
        Some("responses") => decode_response(bytes, name),
        _ => proof_wire::codec::decode_tx(bytes)
            .map(|_| ())
            .map_err(|e| format!("{e:?}")),
    }
}

fn main() -> ExitCode {
    let mut failed = false;
    for path in std::env::args().skip(1) {
        let decoded = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| hex::decode(text.trim()).map_err(|e| e.to_string()))
            .and_then(|bytes| decode(Path::new(&path), &bytes));
        if let Err(error) = decoded {
            println!("{path}: {error}");
            failed = true;
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
