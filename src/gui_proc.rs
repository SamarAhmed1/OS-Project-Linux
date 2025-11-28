use std::time::{Duration, Instant};

use iced::{
    executor,
    Application as IcedApp,
    Element,
    Length,
    Settings,
    Command,
    Subscription,
    Theme,
    widget::{column, row, text, button, scrollable, text_input, container, Space},
    time,
};

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;

// Import your existing modules
mod proc_reader;
mod nice_renice;
mod history;
use history::ProcessHistoryTracker;
use proc_reader::{enumerate_processes, ProcessInfo};
use nice_renice::{get_nice, set_nice, spawn_with_nice};

//----------------- DATA ----------------------

#[derive(Debug, Clone)]
pub struct SystemStats {
    pub total_mem_mb: u64,
    pub used_mem_mb: u64,
    pub cpu_usage: f32,
}

#[derive(Debug, Clone)]
pub enum ViewMode {
    ProcessList,
    SystemStats,
    NiceManager,
    ProcessHistory,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    RefreshProcesses,
    ProcessesLoaded(Vec<ProcessInfo>),
    SystemStatsLoaded(SystemStats),
    
    // View switching
    ShowProcessList,
    ShowSystemStats,
    ShowNiceManager,
    ShowProcessHistory,
    // Process actions
    KillProcess(u32),
    SearchChanged(String),
    
    // Nice/Renice
    NiceValueChanged(String),
    NiceCmdChanged(String),
    NiceArgsChanged(String),
    StartWithNice,
    
    ReniceTargetChanged(String),
    ReniceValueChanged(String),
    ApplyRenice,
    GetNiceValue,
    
    // Results
    ActionResult(String),
}

// -------------------- STATE ----------------------

pub struct ProcessManagerGui {
    view_mode: ViewMode,
    processes: Vec<ProcessInfo>,
    system_stats: Option<SystemStats>,
    search_filter: String,
    last_refresh: Instant,
    
    // Nice/Renice state
    nice_value: String,
    nice_cmd: String,
    nice_args: String,
    
    renice_target: String,
    renice_value: String,
    
    status_message: String,

    history_tracker: Option<ProcessHistoryTracker>
}

// -------------------- ASYNC TASKS ----------------------

async fn fetch_processes() -> Vec<ProcessInfo> {
    enumerate_processes()
}

async fn fetch_system_stats() -> SystemStats {
    use std::fs;
    
    let meminfo = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut total = 0;
    let mut free = 0;
    let mut buffers = 0;
    let mut cached = 0;

    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            total = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
        } else if line.starts_with("MemFree:") {
            free = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
        } else if line.starts_with("Buffers:") {
            buffers = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
        } else if line.starts_with("Cached:") {
            cached = line.split_whitespace().nth(1).unwrap_or("0").parse().unwrap_or(0);
        }
    }
    
    let used = total - free - buffers - cached;
    
    SystemStats {
        total_mem_mb: total / 1024,
        used_mem_mb: used / 1024,
        cpu_usage: 0.0, // You can add CPU calculation if needed
    }
}

// -------------------- ICED APPLICATION ----------------------

impl IcedApp for ProcessManagerGui {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
    // Initialize the tracker HERE, before the Self block
        let mut history_tracker = None;
        if let Ok(mut t) = ProcessHistoryTracker::new() {
            let _ = t.start_monitoring();
            history_tracker = Some(t);
        }
        
        (
            Self {
                view_mode: ViewMode::ProcessList,
                processes: Vec::new(),
                system_stats: None,
                search_filter: String::new(),
                last_refresh: Instant::now(),
                nice_value: String::from("0"),
                nice_cmd: String::new(),
                nice_args: String::new(),
                renice_target: String::new(),
                renice_value: String::new(),
                status_message: String::new(),
                history_tracker,  // Use the variable we created above
            },
            Command::batch(vec![
                Command::perform(fetch_processes(), Message::ProcessesLoaded),
                Command::perform(fetch_system_stats(), Message::SystemStatsLoaded),
            ]),
        )
    }

    fn title(&self) -> String {
        String::from("Linux Process Manager - Rust GUI")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Tick => {
                self.last_refresh = Instant::now();
                if let Some(ref mut tracker) = self.history_tracker {
                    let _ = tracker.update();
                }
                Command::batch(vec![
                    Command::perform(fetch_processes(), Message::ProcessesLoaded),
                    Command::perform(fetch_system_stats(), Message::SystemStatsLoaded),
                ])
            }

            Message::RefreshProcesses => {
                self.last_refresh = Instant::now();
                Command::perform(fetch_processes(), Message::ProcessesLoaded)
            }

            Message::ProcessesLoaded(procs) => {
                self.processes = procs;
                Command::none()
            }

            Message::SystemStatsLoaded(stats) => {
                self.system_stats = Some(stats);
                Command::none()
            }

            Message::ShowProcessList => {
                self.view_mode = ViewMode::ProcessList;
                Command::none()
            }

            Message::ShowSystemStats => {
                self.view_mode = ViewMode::SystemStats;
                Command::none()
            }

            Message::ShowNiceManager => {
                self.view_mode = ViewMode::NiceManager;
                Command::none()
            }

            Message::KillProcess(pid) => {
                match signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
                    Ok(_) => {
                        self.status_message = format!("✓ Killed process {}", pid);
                    }
                    Err(e) => {
                        self.status_message = format!("✗ Failed to kill {}: {}", pid, e);
                    }
                }
                Command::perform(fetch_processes(), Message::ProcessesLoaded)
            }

            Message::SearchChanged(value) => {
                self.search_filter = value;
                Command::none()
            }

            Message::NiceValueChanged(value) => {
                self.nice_value = value;
                Command::none()
            }

            Message::NiceCmdChanged(value) => {
                self.nice_cmd = value;
                Command::none()
            }

            Message::NiceArgsChanged(value) => {
                self.nice_args = value;
                Command::none()
            }

            Message::StartWithNice => {
                if let Ok(nice) = self.nice_value.parse::<i32>() {
                    let args: Vec<String> = self.nice_args
                        .split_whitespace()
                        .map(|s| s.to_string())
                        .collect();
                    
                    match spawn_with_nice(nice, &self.nice_cmd, &args) {
                        Ok(child) => {
                            self.status_message = format!(
                                "✓ Started '{}' (PID {}) with nice {}",
                                self.nice_cmd,
                                child.id(),
                                nice
                            );
                        }
                        Err(e) => {
                            self.status_message = format!("✗ Failed: {}", e);
                        }
                    }
                } else {
                    self.status_message = "✗ Invalid nice value".to_string();
                }
                Command::perform(fetch_processes(), Message::ProcessesLoaded)
            }

            Message::ReniceTargetChanged(value) => {
                self.renice_target = value;
                Command::none()
            }

            Message::ReniceValueChanged(value) => {
                self.renice_value = value;
                Command::none()
            }

            Message::ShowProcessHistory => {
                self.view_mode = ViewMode::ProcessHistory;
                Command::none()
            }

            Message::ApplyRenice => {
                if let Ok(pid) = self.renice_target.parse::<u32>() {
                    if let Ok(nice) = self.renice_value.parse::<i32>() {
                        match set_nice(pid, nice) {
                            Ok(()) => {
                                self.status_message = format!("✓ Set nice of {} to {}", pid, nice);
                            }
                            Err(e) => {
                                self.status_message = format!("✗ Failed: {}", e);
                            }
                        }
                    } else {
                        self.status_message = "✗ Invalid nice value".to_string();
                    }
                } else {
                    self.status_message = "✗ Invalid PID".to_string();
                }
                Command::none()
            }

            Message::GetNiceValue => {
                if let Ok(pid) = self.renice_target.parse::<u32>() {
                    match get_nice(pid) {
                        Ok(nice) => {
                            self.status_message = format!("Process {} nice value: {}", pid, nice);
                            self.renice_value = nice.to_string();
                        }
                        Err(e) => {
                            self.status_message = format!("✗ Failed: {}", e);
                        }
                    }
                } else {
                    self.status_message = "✗ Invalid PID".to_string();
                }
                Command::none()
            }

            Message::ActionResult(msg) => {
                self.status_message = msg;
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        // Navigation bar
        let nav = row![
            button(text("Process List"))
                .on_press(Message::ShowProcessList),
            button(text("System Stats"))
                .on_press(Message::ShowSystemStats),
            button(text("Nice Manager"))
                .on_press(Message::ShowNiceManager),
            Space::with_width(Length::Fill),
            button(text("🔄 Refresh"))
                .on_press(Message::RefreshProcesses),
                button(text("Process History"))
                    .on_press(Message::ShowProcessHistory),
        ]
        .spacing(10)
        .padding(10);

        // Status bar
        let status_bar = container(
            text(&self.status_message).size(14)
        )
        .padding(5)
        .width(Length::Fill);

        // Main content based on view mode
        let content = match self.view_mode {
            ViewMode::ProcessList => self.view_process_list(),
            ViewMode::SystemStats => self.view_system_stats(),
            ViewMode::NiceManager => self.view_nice_manager(),
            ViewMode::ProcessHistory => self.view_process_history(),
        };

        column![
            nav,
            status_bar,
            content,
        ]
        .spacing(10)
        .padding(10)
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        time::every(Duration::from_secs(2)).map(|_| Message::Tick)
    }
}

impl ProcessManagerGui {
    fn view_process_list(&self) -> Element<Message> {
        let search_bar = row![
            text("Search:").size(16),
            text_input("Process name...", &self.search_filter)
                .on_input(Message::SearchChanged)
                .width(Length::Fixed(200.0)),
        ]
        .spacing(10)
        .padding(10);

        // Header
        let header = row![
            text("PID").width(Length::Fixed(80.0)),
            text("Name").width(Length::Fixed(150.0)),
            text("User").width(Length::Fixed(100.0)),
            text("CPU(ms)").width(Length::Fixed(80.0)),
            text("Mem(KB)").width(Length::Fixed(100.0)),
            text("Actions").width(Length::Fixed(100.0)),
        ]
        .spacing(10)
        .padding(10);

        // Filter processes
        let filtered: Vec<_> = self.processes
            .iter()
            .filter(|p| {
                if self.search_filter.is_empty() {
                    true
                } else {
                    p.name.to_lowercase().contains(&self.search_filter.to_lowercase())
                }
            })
            .take(50) // Limit to first 50 for performance
            .collect();

        let filtered_count = filtered.len();
        let mut process_rows = column![].spacing(5);

        for process in &filtered {
            let row_content = row![
                text(format!("{}", process.pid)).width(Length::Fixed(80.0)),
                text(&process.name).width(Length::Fixed(150.0)),
                text(&process.username).width(Length::Fixed(100.0)),
                text(format!("{}", process.cpu_time_ms)).width(Length::Fixed(80.0)),
                text(format!("{}", process.memory_kb)).width(Length::Fixed(100.0)),
                button(text("Kill"))
                    .on_press(Message::KillProcess(process.pid))
                    .width(Length::Fixed(100.0)),
            ]
            .spacing(10)
            .padding(5);

            process_rows = process_rows.push(row_content);
        }

        let list = scrollable(process_rows)
            .height(Length::Fill);

        column![
            search_bar,
            text(format!("Showing {} of {} processes", filtered_count, self.processes.len())).size(14),
            header,
            list,
        ]
        .spacing(10)
        .into()
    }

    fn view_system_stats(&self) -> Element<Message> {
        let stats_content = if let Some(stats) = &self.system_stats {
            column![
                text("System Statistics").size(28),
                Space::with_height(Length::Fixed(20.0)),
                text(format!("Total Memory: {} MB", stats.total_mem_mb)).size(20),
                text(format!("Used Memory: {} MB", stats.used_mem_mb)).size(20),
                text(format!("Memory Usage: {:.1}%", 
                    (stats.used_mem_mb as f32 / stats.total_mem_mb as f32) * 100.0
                )).size(20),
                Space::with_height(Length::Fixed(20.0)),
                text(format!("Total Processes: {}", self.processes.len())).size(20),
                Space::with_height(Length::Fixed(20.0)),
                text(format!(
                    "Last refresh: {:.1}s ago",
                    self.last_refresh.elapsed().as_secs_f32()
                )).size(16),
            ]
            .spacing(10)
        } else {
            column![text("Loading system statistics...").size(20)]
        };

        container(stats_content)
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_nice_manager(&self) -> Element<Message> {
        let nice_section = column![
            text("Start Process with Nice Value").size(24),
            row![
                text("Nice:").width(Length::Fixed(80.0)),
                text_input("-20 to 19", &self.nice_value)
                    .on_input(Message::NiceValueChanged)
                    .width(Length::Fixed(100.0)),
            ].spacing(10),
            row![
                text("Command:").width(Length::Fixed(80.0)),
                text_input("e.g., sleep", &self.nice_cmd)
                    .on_input(Message::NiceCmdChanged)
                    .width(Length::Fixed(200.0)),
            ].spacing(10),
            row![
                text("Args:").width(Length::Fixed(80.0)),
                text_input("e.g., 100", &self.nice_args)
                    .on_input(Message::NiceArgsChanged)
                    .width(Length::Fixed(200.0)),
            ].spacing(10),
            button(text("Start Process"))
                .on_press(Message::StartWithNice),
        ]
        .spacing(15)
        .padding(20);

        let renice_section = column![
            text("Renice Existing Process").size(24),
            row![
                text("PID:").width(Length::Fixed(80.0)),
                text_input("Process ID", &self.renice_target)
                    .on_input(Message::ReniceTargetChanged)
                    .width(Length::Fixed(150.0)),
                button(text("Get Nice"))
                    .on_press(Message::GetNiceValue),
            ].spacing(10),
            row![
                text("Nice:").width(Length::Fixed(80.0)),
                text_input("-20 to 19", &self.renice_value)
                    .on_input(Message::ReniceValueChanged)
                    .width(Length::Fixed(150.0)),
                button(text("Set Nice"))
                    .on_press(Message::ApplyRenice),
            ].spacing(10),
        ]
        .spacing(15)
        .padding(20);

        column![
            nice_section,
            Space::with_height(Length::Fixed(40.0)),
            renice_section,
        ]
        .spacing(20)
        .padding(20)
        .into()
    }

    fn view_process_history(&self) -> Element<Message> {
        if let Some(ref tracker) = self.history_tracker {
            let finished_processes = tracker.get_finished_processes();
            
            // Header
            let header = row![
                text("PID").width(Length::Fixed(80.0)),
                text("User").width(Length::Fixed(100.0)),
                text("Command").width(Length::Fixed(250.0)),
                text("Duration").width(Length::Fixed(100.0)),
                text("CPU Time").width(Length::Fixed(100.0)),
            ]
            .spacing(10)
            .padding(10);
            
            let mut history_rows = column![].spacing(5);
            
            if finished_processes.is_empty() {
                history_rows = history_rows.push(
                    text("No processes have finished yet. Keep the monitor running...")
                        .size(16)
                );
            } else {
                for proc in finished_processes.iter().rev().take(100) {  // Show last 100
                    let duration_str = format!("{:.2}s", proc.duration.as_secs_f64());
                    let cpu_str = format!("{:.2}s", proc.cpu_time_sec);
                    let cmd = if proc.command.len() > 40 {
                        format!("{}...", &proc.command[..37])
                    } else {
                        proc.command.clone()
                    };
                    
                    let row_content = row![
                        text(format!("{}", proc.pid)).width(Length::Fixed(80.0)),
                        text(&proc.user).width(Length::Fixed(100.0)),
                        text(cmd).width(Length::Fixed(250.0)),
                        text(duration_str).width(Length::Fixed(100.0)),
                        text(cpu_str).width(Length::Fixed(100.0)),
                    ]
                    .spacing(10)
                    .padding(5);
                    
                    history_rows = history_rows.push(row_content);
                }
            }
            
            let list = scrollable(history_rows)
                .height(Length::Fill);
            
            column![
                text("Process History").size(28),
                text(format!("Showing {} finished processes", finished_processes.len())).size(14),
                Space::with_height(Length::Fixed(20.0)),
                header,
                list,
            ]
            .spacing(10)
            .padding(20)
            .into()
        } else {
            container(
                text("History tracker not available").size(20)
            )
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        }
    }
}

pub fn main() -> iced::Result {
    ProcessManagerGui::run(Settings::default())
}