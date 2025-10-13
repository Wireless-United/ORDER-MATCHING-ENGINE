use crate::algorithms::fifo::Trade;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use chrono::Utc;

const BUFFER_SIZE: usize = 8192;
const FLUSH_INTERVAL_MS: u64 = 100;

#[derive(Debug, Clone)]
pub enum MatchResult {
    Matched(Trade),
    NoMatch(OrderInfo),
}

#[derive(Debug, Clone)]
pub struct OrderInfo {
    pub order_id: u64,
    pub side: String,
    pub price: f64,
    pub quantity: u64,
    pub timestamp: chrono::DateTime<Utc>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<Utc>,
    pub algorithm: String,
    pub symbol: String,
    pub result: MatchResult,
}

/// Trade logger for algorithm matching results
pub struct AlgorithmLogger {
    tx: Sender<LogEntry>,
    shutdown: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl AlgorithmLogger {
    /// Create a new algorithm logger
    pub fn new(log_path: PathBuf) -> Self {
        let (tx, rx) = unbounded();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        let handle = thread::Builder::new()
            .name("algorithm-logger".to_string())
            .spawn(move || {
                Self::logger_worker(rx, log_path, shutdown_clone);
            })
            .expect("Failed to spawn algorithm logger thread");

        Self {
            tx,
            shutdown,
            handle: Some(handle),
        }
    }

    /// Log a matched trade
    pub fn log_match(&self, algorithm: &str, symbol: &str, trade: Trade) {
        let entry = LogEntry {
            timestamp: Utc::now(),
            algorithm: algorithm.to_string(),
            symbol: symbol.to_string(),
            result: MatchResult::Matched(trade),
        };
        let _ = self.tx.send(entry);
    }

    /// Log an unmatched order
    pub fn log_no_match(&self, algorithm: &str, symbol: &str, order_info: OrderInfo) {
        let entry = LogEntry {
            timestamp: Utc::now(),
            algorithm: algorithm.to_string(),
            symbol: symbol.to_string(),
            result: MatchResult::NoMatch(order_info),
        };
        let _ = self.tx.send(entry);
    }

    /// Background worker that writes logs to file
    fn logger_worker(rx: Receiver<LogEntry>, log_path: PathBuf, shutdown: Arc<AtomicBool>) {
        // Create log directory if needed
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let file = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            Ok(f) => f,
            Err(_) => return,
        };

        let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);

        // Write header if file is empty
        if let Ok(metadata) = std::fs::metadata(&log_path) {
            if metadata.len() == 0 {
                let _ = writeln!(
                    writer,
                    "timestamp,algorithm,symbol,type,buy_id,sell_id,price,quantity,rank,order_id,side,reason"
                );
            }
        }

        let flush_interval = std::time::Duration::from_millis(FLUSH_INTERVAL_MS);
        let mut last_flush = std::time::Instant::now();
        let mut entries_logged = 0u64;

        loop {
            if shutdown.load(Ordering::Relaxed) {
                // Drain remaining entries
                while let Ok(entry) = rx.try_recv() {
                    let _ = Self::write_entry(&mut writer, &entry);
                    entries_logged += 1;
                }
                break;
            }

            match rx.recv_timeout(flush_interval) {
                Ok(entry) => {
                    let _ = Self::write_entry(&mut writer, &entry);
                    entries_logged += 1;
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    if last_flush.elapsed() >= flush_interval {
                        let _ = writer.flush();
                        last_flush = std::time::Instant::now();
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }
        }

        let _ = writer.flush();
    }

    /// Write a single log entry
    fn write_entry(writer: &mut BufWriter<File>, entry: &LogEntry) -> std::io::Result<()> {
        let timestamp = entry.timestamp.to_rfc3339();
        
        match &entry.result {
            MatchResult::Matched(trade) => {
                writeln!(
                    writer,
                    "{},{},{},MATCHED,{},{},{},{},{},,,,",
                    timestamp,
                    entry.algorithm,
                    entry.symbol,
                    trade.buy_id,
                    trade.sell_id,
                    trade.price,
                    trade.quantity,
                    trade.rank
                )
            }
            MatchResult::NoMatch(order_info) => {
                writeln!(
                    writer,
                    "{},{},{},NO_MATCH,,,,,{},{},{},{}",
                    timestamp,
                    entry.algorithm,
                    entry.symbol,
                    order_info.order_id,
                    order_info.side,
                    order_info.price,
                    order_info.reason
                )
            }
        }
    }

    /// Graceful shutdown
    pub fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for AlgorithmLogger {
    fn drop(&mut self) {
        if !self.shutdown.load(Ordering::Relaxed) {
            self.shutdown();
        }
    }
}

/// Create a default logger instance
pub fn create_default_logger() -> Arc<AlgorithmLogger> {
    Arc::new(AlgorithmLogger::new(PathBuf::from("./logs/algorithm_trades.log")))
}

/// Global logger instance (lazy static pattern)
static LOGGER_INIT: AtomicBool = AtomicBool::new(false);
static mut GLOBAL_LOGGER: Option<Arc<AlgorithmLogger>> = None;

/// Get or initialize the global logger
pub fn get_logger() -> Arc<AlgorithmLogger> {
    unsafe {
        if !LOGGER_INIT.load(Ordering::Acquire) {
            GLOBAL_LOGGER = Some(create_default_logger());
            LOGGER_INIT.store(true, Ordering::Release);
        }
        GLOBAL_LOGGER.as_ref().unwrap().clone()
    }
}

/// Initialize logger with custom path
pub fn init_logger(log_path: PathBuf) -> Arc<AlgorithmLogger> {
    unsafe {
        let logger = Arc::new(AlgorithmLogger::new(log_path));
        GLOBAL_LOGGER = Some(logger.clone());
        LOGGER_INIT.store(true, Ordering::Release);
        logger
    }
}
