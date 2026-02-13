use log::{Record, Metadata, LevelFilter};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

pub struct BufferLogger {
    pub logs: Arc<Mutex<VecDeque<String>>>,
    pub capacity: usize,
}

impl BufferLogger {
    pub fn new(capacity: usize) -> Self {
        Self {
            logs: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    pub fn init(self) -> Result<(), log::SetLoggerError> {
        let max_level = LevelFilter::Info;
        log::set_max_level(max_level);
        log::set_boxed_logger(Box::new(self))
    }
}

impl log::Log for BufferLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let msg = format!(
                "{} [{}] - {}",
                chrono::Local::now().format("%H:%M:%S"),
                record.level(),
                record.args()
            );
            
            let mut logs = self.logs.lock().unwrap();
            if logs.len() >= self.capacity {
                logs.pop_front();
            }
            logs.push_back(msg);
        }
    }

    fn flush(&self) {}
}
