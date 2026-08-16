use std::ffi::OsString;
use std::process::Command;

use thiserror::Error;
use zellij_agent_shared::{Event, Transport};

const PIPE_NAME: &str = "zja.events";

#[derive(Debug)]
pub(crate) struct ZellijPipeTransport {
    executable: OsString,
}

impl ZellijPipeTransport {
    pub(crate) fn new(executable: OsString) -> Self {
        Self { executable }
    }
}

#[derive(Debug, Error)]
pub(crate) enum PipeTransportError {
    #[error("could not serialize the event: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("could not start Zellij: {0}")]
    Start(#[source] std::io::Error),
    #[error("Zellij rejected the event ({status}): {message}")]
    Rejected { status: String, message: String },
}

impl Transport for ZellijPipeTransport {
    type Error = PipeTransportError;

    fn send(&mut self, event: &Event) -> Result<(), Self::Error> {
        let payload = serde_json::to_string(event)?;
        let output = Command::new(&self.executable)
            .arg("pipe")
            .arg("--name")
            .arg(PIPE_NAME)
            .arg("--")
            .arg(payload)
            .output()
            .map_err(PipeTransportError::Start)?;

        if output.status.success() {
            return Ok(());
        }

        Err(PipeTransportError::Rejected {
            status: output.status.to_string(),
            message: String::from_utf8_lossy(&output.stderr)
                .trim()
                .chars()
                .map(|character| {
                    if character.is_control() {
                        ' '
                    } else {
                        character
                    }
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zellij_agent_shared::{AgentStatus, Generation, PaneId, Sequence};

    #[test]
    fn a_successful_process_accepts_serialized_events() {
        let mut transport = ZellijPipeTransport::new(OsString::from("/usr/bin/true"));
        let event = Event::status_changed(
            PaneId::from(1_u32),
            AgentStatus::Idle,
            Generation::from(1_u64),
            Sequence::from(0_u64),
        );

        assert!(transport.send(&event).is_ok());
    }

    #[test]
    fn a_failed_process_is_reported() {
        let mut transport = ZellijPipeTransport::new(OsString::from("/usr/bin/false"));
        let event = Event::status_changed(
            PaneId::from(1_u32),
            AgentStatus::Error,
            Generation::from(1_u64),
            Sequence::from(0_u64),
        );

        assert!(matches!(
            transport.send(&event),
            Err(PipeTransportError::Rejected { .. })
        ));
    }
}
