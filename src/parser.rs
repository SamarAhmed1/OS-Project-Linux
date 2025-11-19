use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub enum Command {
    ListProcesses {
        all: bool,
        user: Option<String>,
        sort_by: Option<String>,
    },
    KillProcess {
        pid: u32,
        signal: Option<String>,
    },
    ProcessInfo {
        pid: u32,
        detailed: bool,
    },
    SystemStats {
        refresh_interval: Option<u64>,
    },
    SearchProcess {
        name: String,
        exact: bool,
    },
    Monitor { 
        interval: u64 
    },
    Help,
    Exit,
    Unknown(String),
}

#[derive(Debug)]
pub struct ParseResult {
    pub command: Command,
    pub raw_input: String,
}

pub struct CommandParser;

impl CommandParser {
    pub fn new() -> Self {
        CommandParser
    }

    pub fn parse(&self, input: &str) -> ParseResult {
        let input = input.trim();
        if input.is_empty() {
            return ParseResult {
                command: Command::Help,
                raw_input: input.to_string(),
            };
        }

        let parts: Vec<&str> = input.split_whitespace().collect();
        let command = parts[0].to_lowercase();

        match command.as_str() {
            "ps" | "list" => self.parse_list_command(&parts[1..]),
            "kill" => self.parse_kill_command(&parts[1..]),
            "info" | "show" => self.parse_info_command(&parts[1..]),
            "stats" | "status" => self.parse_stats_command(&parts[1..]),
            "search" | "find" => self.parse_search_command(&parts[1..]),
            "monitor" => {
                // Use parts after command word as args:
                let args = &parts[1..];
                // Now you can use args here safely
                let interval = if !args.is_empty() {
                    args[0].parse::<u64>().unwrap_or(2)
                } else {
                    2
                };
                return ParseResult {
                    command: Command::Monitor { interval },
                    raw_input: input.to_string(),
                };
            },
            "help" => ParseResult {
                command: Command::Help,
                raw_input: input.to_string(),
            },
            "exit" | "quit" => ParseResult {
                command: Command::Exit,
                raw_input: input.to_string(),
            },
            _ => ParseResult {
                command: Command::Unknown(input.to_string()),
                raw_input: input.to_string(),
            },
        }
    }

    fn parse_list_command(&self, args: &[&str]) -> ParseResult {
        let mut all = false;
        let mut user = None;
        let mut sort_by = None;

        let mut i = 0;
        while i < args.len() {
            match args[i] {
                "-a" | "--all" => all = true,
                "-u" | "--user" => {
                    if i + 1 < args.len() {
                        user = Some(args[i + 1].to_string());
                        i += 1;
                    }
                }
                "-s" | "--sort" => {
                    if i + 1 < args.len() {
                        sort_by = Some(args[i + 1].to_string());
                        i += 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }

        ParseResult {
            command: Command::ListProcesses { all, user, sort_by },
            raw_input: args.join(" "),
        }
    }

    fn parse_kill_command(&self, args: &[&str]) -> ParseResult {
        if args.is_empty() {
            return ParseResult {
                command: Command::Unknown("kill: missing PID".to_string()),
                raw_input: args.join(" "),
            };
        }

        let pid = match args[0].parse() {
            Ok(pid) => pid,
            Err(_) => {
                return ParseResult {
                    command: Command::Unknown(format!("kill: invalid PID '{}'", args[0])),
                    raw_input: args.join(" "),
                }
            }
        };

        let signal = if args.len() > 1 {
            Some(args[1].to_string())
        } else {
            None
        };

        ParseResult {
            command: Command::KillProcess { pid, signal },
            raw_input: args.join(" "),
        }
    }

    fn parse_info_command(&self, args: &[&str]) -> ParseResult {
        if args.is_empty() {
            return ParseResult {
                command: Command::Unknown("info: missing PID".to_string()),
                raw_input: args.join(" "),
            };
        }

        let pid = match args[0].parse() {
            Ok(pid) => pid,
            Err(_) => {
                return ParseResult {
                    command: Command::Unknown(format!("info: invalid PID '{}'", args[0])),
                    raw_input: args.join(" "),
                }
            }
        };

        let detailed = args.iter().any(|&arg| arg == "-d" || arg == "--detailed");

        ParseResult {
            command: Command::ProcessInfo { pid, detailed },
            raw_input: args.join(" "),
        }
    }

    fn parse_stats_command(&self, args: &[&str]) -> ParseResult {
        let mut refresh_interval = None;

        for (i, arg) in args.iter().enumerate() {
            if let Some(num) = arg.strip_prefix("--refresh=") {
                if let Ok(interval) = num.parse() {
                    refresh_interval = Some(interval);
                }
            } else if *arg == "--refresh" {
                // Handle case where refresh interval is next argument
                if i + 1 < args.len() {
                    if let Ok(interval) = args[i + 1].parse::<u64>() {
                        refresh_interval = Some(interval);
                    }
                }
            }
        }

        ParseResult {
            command: Command::SystemStats { refresh_interval },
            raw_input: args.join(" "),
        }
    }

    fn parse_search_command(&self, args: &[&str]) -> ParseResult {
        if args.is_empty() {
            return ParseResult {
                command: Command::Unknown("search: missing process name".to_string()),
                raw_input: args.join(" "),
            };
        }

        let name = args[0].to_string();
        let exact = args.iter().any(|&arg| arg == "-e" || arg == "--exact");

        ParseResult {
            command: Command::SearchProcess { name, exact },
            raw_input: args.join(" "),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_list_command() {
        let parser = CommandParser::new();
        let result = parser.parse("ps -a -u root --sort cpu");
        
        if let Command::ListProcesses { all, user, sort_by } = result.command {
            assert!(all);
            assert_eq!(user, Some("root".to_string()));
            assert_eq!(sort_by, Some("cpu".to_string()));
        } else {
            panic!("Expected ListProcesses command");
        }
    }

    #[test]
    fn test_parse_kill_command() {
        let parser = CommandParser::new();
        let result = parser.parse("kill 1234 SIGTERM");
        
        if let Command::KillProcess { pid, signal } = result.command {
            assert_eq!(pid, 1234);
            assert_eq!(signal, Some("SIGTERM".to_string()));
        } else {
            panic!("Expected KillProcess command");
        }
    }

    #[test]
    fn test_parse_help() {
        let parser = CommandParser::new();
        let result = parser.parse("help");
        assert!(matches!(result.command, Command::Help));
    }
}