#![cfg_attr(
    not(test),
    deny(clippy::expect_used, clippy::panic, clippy::unwrap_used)
)]

mod transport;

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, ExitCode, ExitStatus};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand, ValueEnum};
use thiserror::Error;
use transport::ZellijPipeTransport;
use zellij_agent_shared::{
    AgentId, AgentStatus, Event, Generation, PaneId, Sequence, TabId, Transport,
};

const DEFAULT_AGENT_ID: &str = "default";

#[derive(Debug, Parser)]
#[command(name = "zja", version, about)]
struct Cli {
    #[arg(long, env = "ZELLIJ_PANE_ID", value_parser = parse_pane_id)]
    pane_id: Option<u32>,

    #[arg(long, env = "ZJA_TAB_ID")]
    tab_id: Option<u64>,

    #[arg(long, env = "ZJA_ZELLIJ_BIN", default_value = "zellij")]
    zellij_bin: OsString,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Status {
        #[arg(value_enum)]
        state: StatusArg,

        #[arg(long, env = "ZJA_AGENT_ID", default_value = DEFAULT_AGENT_ID)]
        agent_id: String,

        #[arg(long, env = "ZJA_GENERATION")]
        generation: Option<u64>,

        #[arg(long, env = "ZJA_SEQUENCE", default_value_t = 0)]
        sequence: u64,
    },
    Run {
        #[arg(long, env = "ZJA_AGENT_ID")]
        agent_id: Option<String>,

        #[arg(last = true, required = true, num_args = 1..)]
        command: Vec<OsString>,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum StatusArg {
    Idle,
    Running,
    Complete,
    Error,
}

impl From<StatusArg> for AgentStatus {
    fn from(value: StatusArg) -> Self {
        match value {
            StatusArg::Idle => Self::Idle,
            StatusArg::Running => Self::Running,
            StatusArg::Complete => Self::Complete,
            StatusArg::Error => Self::Error,
        }
    }
}

#[derive(Debug, Error)]
enum CliError {
    #[error("no target pane; run inside Zellij or pass --pane-id")]
    MissingPane,
    #[error("failed to report status: {0}")]
    Transport(String),
    #[error("failed to launch {program}: {source}")]
    Launch {
        program: String,
        #[source]
        source: std::io::Error,
    },
}

enum CommandOutcome {
    Reported,
    Child(ExitStatus),
}

pub fn main_entry() -> ExitCode {
    let cli = Cli::parse();
    let mut transport = ZellijPipeTransport::new(cli.zellij_bin.clone());

    match execute(cli, &mut transport) {
        Ok(CommandOutcome::Reported) => ExitCode::SUCCESS,
        Ok(CommandOutcome::Child(status)) => exit_code(status),
        Err(error) => {
            eprintln!("zja: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute<T>(cli: Cli, transport: &mut T) -> Result<CommandOutcome, CliError>
where
    T: Transport,
    T::Error: std::fmt::Display,
{
    let tab_id = cli.tab_id.map(TabId::from);
    match cli.command {
        Commands::Status {
            state,
            agent_id,
            generation,
            sequence,
        } => {
            let pane_id = cli.pane_id.map(PaneId::from).ok_or(CliError::MissingPane)?;
            let event = Event::status_changed_for(
                tab_id,
                pane_id,
                AgentId::from(agent_id),
                state.into(),
                Generation::from(generation.unwrap_or_else(generation_seed)),
                Sequence::from(sequence),
            );
            transport
                .send(&event)
                .map_err(|error| CliError::Transport(error.to_string()))?;
            Ok(CommandOutcome::Reported)
        }
        Commands::Run { agent_id, command } => run_child(
            cli.pane_id.map(PaneId::from),
            tab_id,
            agent_id,
            command,
            transport,
        ),
    }
}

fn run_child<T>(
    pane_id: Option<PaneId>,
    tab_id: Option<TabId>,
    agent_id: Option<String>,
    command: Vec<OsString>,
    transport: &mut T,
) -> Result<CommandOutcome, CliError>
where
    T: Transport,
    T::Error: std::fmt::Display,
{
    let Some(program) = command.first() else {
        return Err(CliError::Launch {
            program: "<missing command>".to_owned(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty command"),
        });
    };
    let generation = generation_seed();
    let agent_id = agent_id.unwrap_or_else(|| command_agent_id(program));

    if let Some(target_pane) = pane_id {
        let running = Event::status_changed_for(
            tab_id,
            target_pane,
            AgentId::from(agent_id.clone()),
            AgentStatus::Running,
            Generation::from(generation),
            Sequence::from(0_u64),
        );
        report_best_effort(transport, &running);
    } else {
        eprintln!("zja: warning: running outside Zellij; lifecycle status is not reported");
    }

    let mut child = Command::new(program);
    child
        .args(command.iter().skip(1))
        .env("ZJA_AGENT_ID", &agent_id)
        .env("ZJA_GENERATION", generation.to_string())
        .env("ZJA_SEQUENCE", "1");
    let status = match child.status() {
        Ok(status) => status,
        Err(source) => {
            if let Some(target_pane) = pane_id {
                let failed = Event::status_changed_for(
                    tab_id,
                    target_pane,
                    AgentId::from(agent_id),
                    AgentStatus::Error,
                    Generation::from(generation),
                    Sequence::from(2_u64),
                );
                report_best_effort(transport, &failed);
            }
            return Err(CliError::Launch {
                program: safe_diagnostic(&program.to_string_lossy()),
                source,
            });
        }
    };

    if let Some(target_pane) = pane_id {
        let final_status = if status.success() {
            AgentStatus::Complete
        } else {
            AgentStatus::Error
        };
        let completed = Event::status_changed_for(
            tab_id,
            target_pane,
            AgentId::from(agent_id),
            final_status,
            Generation::from(generation),
            Sequence::from(2_u64),
        );
        report_best_effort(transport, &completed);
    }

    Ok(CommandOutcome::Child(status))
}

fn report_best_effort<T>(transport: &mut T, event: &Event)
where
    T: Transport,
    T::Error: std::fmt::Display,
{
    if let Err(error) = transport.send(event) {
        eprintln!("zja: warning: failed to report lifecycle status: {error}");
    }
}

fn command_agent_id(program: &OsStr) -> String {
    Path::new(program)
        .file_name()
        .filter(|name| !name.is_empty())
        .unwrap_or(program)
        .to_string_lossy()
        .into_owned()
}

fn safe_diagnostic(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn generation_seed() -> u64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX)
}

fn parse_pane_id(value: &str) -> Result<u32, String> {
    let numeric = value
        .strip_prefix("terminal_")
        .or_else(|| value.strip_prefix("pane_"))
        .unwrap_or(value);
    numeric
        .parse::<u32>()
        .map_err(|_| format!("invalid terminal pane ID: {value}"))
}

fn exit_code(status: ExitStatus) -> ExitCode {
    status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .map(ExitCode::from)
        .unwrap_or(ExitCode::FAILURE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingTransport {
        events: Vec<Event>,
        fail: bool,
    }

    impl Transport for RecordingTransport {
        type Error = std::io::Error;

        fn send(&mut self, event: &Event) -> Result<(), Self::Error> {
            if self.fail {
                Err(std::io::Error::other("transport unavailable"))
            } else {
                self.events.push(event.clone());
                Ok(())
            }
        }
    }

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).unwrap_or_else(|error| panic!("valid arguments: {error}"))
    }

    #[test]
    fn parses_zellij_terminal_pane_ids() {
        assert_eq!(parse_pane_id("terminal_42"), Ok(42));
        assert_eq!(parse_pane_id("42"), Ok(42));
        assert!(parse_pane_id("plugin_42").is_err());
    }

    #[test]
    fn status_sends_one_event() {
        let cli = parse(&[
            "zja",
            "--pane-id",
            "7",
            "status",
            "running",
            "--generation",
            "10",
            "--sequence",
            "3",
        ]);
        let mut transport = RecordingTransport::default();

        let outcome = execute(cli, &mut transport);

        assert!(matches!(outcome, Ok(CommandOutcome::Reported)));
        assert_eq!(transport.events.len(), 1);
        let json = serde_json::to_value(&transport.events[0])
            .unwrap_or_else(|error| panic!("event serializes: {error}"));
        assert_eq!(json["type"], "status_changed");
        assert_eq!(json["pane_id"], 7);
        assert_eq!(json["status"], "running");
        assert_eq!(json["generation"], 10);
        assert_eq!(json["sequence"], 3);
    }

    #[test]
    fn missing_pane_rejects_direct_status() {
        let mut cli = parse(&["zja", "--pane-id", "3", "status", "idle"]);
        cli.pane_id = None;
        let mut transport = RecordingTransport::default();

        assert!(matches!(
            execute(cli, &mut transport),
            Err(CliError::MissingPane)
        ));
    }

    #[test]
    fn run_reports_running_then_complete() {
        let cli = parse(&[
            "zja",
            "--pane-id",
            "3",
            "run",
            "--",
            "/bin/sh",
            "-c",
            "exit 0",
        ]);
        let mut transport = RecordingTransport::default();

        let outcome = execute(cli, &mut transport);

        assert!(matches!(outcome, Ok(CommandOutcome::Child(status)) if status.success()));
        assert_eq!(transport.events.len(), 2);
        let states: Vec<String> = transport
            .events
            .iter()
            .filter_map(|event| serde_json::to_value(event).ok())
            .filter_map(|json| json["status"].as_str().map(str::to_owned))
            .collect();
        assert_eq!(states, ["running".to_owned(), "complete".to_owned()]);
    }

    #[test]
    fn run_preserves_failure_and_reports_error() {
        let cli = parse(&[
            "zja",
            "--pane-id",
            "3",
            "run",
            "--",
            "/bin/sh",
            "-c",
            "exit 17",
        ]);
        let mut transport = RecordingTransport::default();

        let outcome = execute(cli, &mut transport);

        assert!(matches!(outcome, Ok(CommandOutcome::Child(status)) if status.code() == Some(17)));
        let final_event = transport
            .events
            .last()
            .and_then(|event| serde_json::to_value(event).ok())
            .unwrap_or_else(|| panic!("final event serializes"));
        assert_eq!(final_event["status"], "error");
    }

    #[test]
    fn run_continues_when_transport_is_unavailable() {
        let cli = parse(&[
            "zja",
            "--pane-id",
            "3",
            "run",
            "--",
            "/bin/sh",
            "-c",
            "exit 0",
        ]);
        let mut transport = RecordingTransport {
            fail: true,
            ..RecordingTransport::default()
        };

        assert!(matches!(
            execute(cli, &mut transport),
            Ok(CommandOutcome::Child(status)) if status.success()
        ));
    }

    #[test]
    fn launch_failure_replaces_running_with_error() {
        let cli = parse(&[
            "zja",
            "--pane-id",
            "3",
            "run",
            "--",
            "/path/that/does/not/exist/zja-test-agent",
        ]);
        let mut transport = RecordingTransport::default();

        assert!(matches!(
            execute(cli, &mut transport),
            Err(CliError::Launch { .. })
        ));
        let states: Vec<String> = transport
            .events
            .iter()
            .filter_map(|event| serde_json::to_value(event).ok())
            .filter_map(|json| json["status"].as_str().map(str::to_owned))
            .collect();
        assert_eq!(states, ["running".to_owned(), "error".to_owned()]);
    }
}
