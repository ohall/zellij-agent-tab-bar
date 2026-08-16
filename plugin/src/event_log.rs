use std::collections::VecDeque;

const EVENT_LOG_CAPACITY: usize = 128;
const EVENT_MESSAGE_MAX_CHARS: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LogEntry {
    pub(crate) level: LogLevel,
    pub(crate) source: &'static str,
    pub(crate) message: String,
}

#[derive(Debug)]
pub(crate) struct EventLog {
    entries: VecDeque<LogEntry>,
    capacity: usize,
}

impl Default for EventLog {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(EVENT_LOG_CAPACITY),
            capacity: EVENT_LOG_CAPACITY,
        }
    }
}

impl EventLog {
    pub(crate) fn record(
        &mut self,
        level: LogLevel,
        source: &'static str,
        message: impl Into<String>,
        emit: bool,
    ) {
        let entry = LogEntry {
            level,
            source,
            message: bounded_message(message.into()),
        };
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        if emit {
            eprintln!(
                "{} zellij-agent-tab-bar {}: {}",
                entry.level.as_str(),
                entry.source,
                entry.message
            );
        }
        self.entries.push_back(entry);
    }

    #[cfg(test)]
    fn entries(&self) -> &VecDeque<LogEntry> {
        &self.entries
    }
}

fn bounded_message(message: String) -> String {
    message
        .chars()
        .take(EVENT_MESSAGE_MAX_CHARS)
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_is_bounded_and_discards_oldest_entries() {
        let mut log = EventLog {
            entries: VecDeque::new(),
            capacity: 2,
        };
        log.record(LogLevel::Trace, "test", "first", false);
        log.record(LogLevel::Debug, "test", "second", false);
        log.record(LogLevel::Warn, "test", "third", false);

        let messages: Vec<&str> = log
            .entries()
            .iter()
            .map(|entry| entry.message.as_str())
            .collect();
        assert_eq!(messages, ["second", "third"]);
    }
}
