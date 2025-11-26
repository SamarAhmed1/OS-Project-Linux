use std::time::{Duration, Instant};


use procfs::{
    self,
    process::all_processes,
    Meminfo,
    Current
};


use iced::{
    executor,
    Application as IcedApp, // <-- THE REAL TRAIT
    Element,
    Length,
    Settings,
    Command,
    Subscription,
    Theme,
    widget::{column, text, button},
    time,
};
//----------------- DATA ----------------------

#[derive(Debug, Clone)]
pub struct AppData {
    pub process_count: usize,
    pub total_mem_gb: f32,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    RefreshClicked,
    DataFetched(Result<AppData, String>),
}


async fn fetch_data() -> Result<AppData, String> {

    // --- 1. Get Process Count ---
    let process_count = match all_processes() {
        Ok(iter) => iter.count(),
        Err(e) => return Err(format!("Failed to read processes: {}", e)),
    };

    // --- 2. Get Memory Info ---
    let meminfo = match Meminfo::current() {
        Ok(info) => info,
        Err(e) => return Err(format!("Failed to read memory info: {}", e)),
    };


    let total_used_kb = meminfo.mem_total.saturating_sub(
        meminfo.mem_available.unwrap_or(meminfo.mem_free)
    );

    // Convert total used KB to MB (1 MB = 1024 KB)
    let total_mem_gb = (total_used_kb as f32) / (1024.0 * 1024.0 * 1024.0);

    Ok(AppData {
        process_count: process_count,
        total_mem_gb: total_mem_gb,
    })
}
// -------------------- STATE ----------------------


pub struct ProcessManagerGui {
    current_data: Option<AppData>,
    last_refresh: Instant,
    is_loading: bool,
}

// -------------------- ICED 0.13 APPLICATION ----------------------

impl IcedApp for ProcessManagerGui {

    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            Self {
                current_data: None,
                last_refresh: Instant::now(),
                is_loading: true,
            },
            Command::perform(fetch_data(), Message::DataFetched),
        )
    }

    fn title(&self) -> String {
        String::from("Rust Process Manager GUI")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Tick => {
                self.is_loading = true;
                self.last_refresh = Instant::now();
                Command::perform(fetch_data(), Message::DataFetched)
            }

            Message::RefreshClicked => {
                self.is_loading = true;
                self.last_refresh = Instant::now();
                Command::perform(fetch_data(), Message::DataFetched)
            }

            Message::DataFetched(Ok(data)) => {
                self.current_data = Some(data);
                self.is_loading = false;
                Command::none()
            }

            Message::DataFetched(Err(e)) => {
                eprintln!("Fetch error: {}", e);
                self.is_loading = false;
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let stats = match &self.current_data {
            Some(data) => {
                column![
                    text(format!("Processes: {}", data.process_count)),
                    text(format!("Memory: {} GB", data.total_mem_gb)),
                    text(format!(
                        "Last refresh: {:.1}s ago",
                        self.last_refresh.elapsed().as_secs_f32()
                    )),
                ]
                .spacing(10)
            }
            None => column![text("Loading system data...").size(20)],
        };

        let refresh_button = button(
            text(if self.is_loading { "Refreshing..." } else { "Refresh Now" })
        )
        .on_press_maybe(if self.is_loading {
            None
        } else {
            Some(Message::RefreshClicked)
        });

        column![
            text("System Overview").size(28),
            stats,
            refresh_button,
        ]
        .spacing(20)
        .padding(20)
        .width(Length::Fill)
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        time::every(Duration::from_secs(5)).map(|_| Message::Tick)
    }
}

// -------------------- MAIN ----------------------

pub fn main() -> iced::Result {
    ProcessManagerGui::run(Settings::default())
}