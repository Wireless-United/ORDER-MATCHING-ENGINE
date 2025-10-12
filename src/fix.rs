use crate::types::{Event, Side};
use crossbeam_channel::Sender;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use chrono::Utc;

const SOH: u8 = 0x01;
const HEARTBEAT_INTERVAL: u64 = 30; // seconds

#[derive(Clone)]
pub struct FixState {
    pub ingress_sender: Sender<Event>,
    pub valid_symbols: Arc<Mutex<HashSet<String>>>,
    pub sessions: Arc<Mutex<HashMap<String, FixSession>>>,
}

#[derive(Debug, Clone)]
pub struct FixSession {
    pub sender_comp_id: String,
    pub target_comp_id: String,
    pub msg_seq_num: u32,
    pub expected_seq_num: u32,
    pub logged_in: bool,
    pub last_heartbeat: i64,
}

impl FixState {
    pub fn new(ingress_sender: Sender<Event>) -> Self {
        let mut symbols = HashSet::new();
        // Add default symbols
        symbols.insert("AAPL".to_string());
        symbols.insert("GOOGL".to_string());
        symbols.insert("MSFT".to_string());
        symbols.insert("TSLA".to_string());

        Self {
            ingress_sender,
            valid_symbols: Arc::new(Mutex::new(symbols)),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_symbol(&self, symbol: String) -> bool {
        let mut symbols = self.valid_symbols.lock().unwrap();
        if symbols.contains(&symbol) {
            false
        } else {
            symbols.insert(symbol);
            true
        }
    }

    pub fn is_valid_symbol(&self, symbol: &str) -> bool {
        let symbols = self.valid_symbols.lock().unwrap();
        symbols.contains(symbol)
    }

    pub fn get_symbols(&self) -> Vec<String> {
        let symbols = self.valid_symbols.lock().unwrap();
        symbols.iter().cloned().collect()
    }
}

/// Start the FIX protocol acceptor server
pub fn start_fix_acceptor(bind_addr: &str, fix_state: FixState) -> JoinHandle<()> {
    let fix_state = Arc::new(fix_state);
    let bind = bind_addr.to_string();

    tokio::spawn(async move {
        let listener = match TcpListener::bind(&bind).await {
            Ok(l) => l,
            Err(e) => {
                error!("Failed to bind FIX acceptor on {}: {}", bind, e);
                return;
            }
        };

        info!("FIX Protocol acceptor listening on {}", bind);
        info!("Supported FIX message types:");
        info!("  Logon (35=A) - Session establishment");
        info!("  Heartbeat (35=0) - Keep-alive");
        info!("  NewOrderSingle (35=D) - Order submission");
        info!("  OrderStatusRequest (35=H) - Order book query");
        info!("  SecurityListRequest (35=x) - Symbol list query");
        info!("  SecurityDefinitionRequest (35=c) - Symbol creation");
        info!("  TestRequest (35=1) - Connection test");
        info!("  Logout (35=5) - Session termination");

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    info!("Accepted FIX connection from {}", addr);
                    let fix_state = fix_state.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_fix_connection(socket, fix_state).await {
                            error!("FIX connection error from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept FIX connection: {}", e);
                }
            }
        }
    })
}

async fn handle_fix_connection(mut socket: TcpStream, fix_state: Arc<FixState>) -> Result<(), String> {
    let mut buf = Vec::new();
    let mut read_buf = [0u8; 4096];
    let mut session_id: Option<String> = None;

    loop {
        let n = socket
            .read(&mut read_buf)
            .await
            .map_err(|e| format!("read error: {}", e))?;
        
        if n == 0 {
            info!("FIX connection closed by peer");
            if let Some(sid) = session_id {
                remove_session(&fix_state, &sid);
            }
            return Ok(());
        }

        buf.extend_from_slice(&read_buf[..n]);

        // Process complete FIX messages
        while let Some(msg_end) = find_complete_fix_message(&buf) {
            let msg_bytes = buf.drain(..msg_end).collect::<Vec<u8>>();
            let raw_msg = String::from_utf8_lossy(&msg_bytes);
            debug!("Received FIX message: {}", sanitize_fix_message(&raw_msg));

            match parse_fix_message(&msg_bytes) {
                Ok(msg_map) => {
                    match process_fix_message(&mut socket, &fix_state, &msg_map, &mut session_id).await {
                        Ok(_) => {},
                        Err(e) => {
                            warn!("Error processing FIX message: {}", e);
                            send_reject(&mut socket, &msg_map, &e).await?;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to parse FIX message: {}", e);
                    send_reject(&mut socket, &HashMap::new(), &e).await?;
                }
            }
        }
    }
}

async fn process_fix_message(
    socket: &mut TcpStream, 
    fix_state: &Arc<FixState>, 
    msg: &HashMap<String, String>,
    session_id: &mut Option<String>
) -> Result<(), String> {
    let msg_type = msg.get("35").ok_or("Missing message type (35)")?;
    
    match msg_type.as_str() {
        "A" => handle_logon(socket, fix_state, msg, session_id).await,
        "0" => handle_heartbeat(socket, fix_state, msg, session_id).await,
        "1" => handle_test_request(socket, fix_state, msg, session_id).await,
        "5" => handle_logout(socket, fix_state, msg, session_id).await,
        "D" => handle_new_order_single(socket, fix_state, msg, session_id).await,
        "H" => handle_order_status_request(socket, fix_state, msg, session_id).await,
        "x" => handle_security_list_request(socket, fix_state, msg, session_id).await,
        "c" => handle_security_definition_request(socket, fix_state, msg, session_id).await,
        _ => {
            warn!("Unsupported message type: {}", msg_type);
            Err(format!("Unsupported message type: {}", msg_type))
        }
    }
}

async fn handle_logon(
    socket: &mut TcpStream,
    fix_state: &Arc<FixState>,
    msg: &HashMap<String, String>,
    session_id: &mut Option<String>
) -> Result<(), String> {
    let sender_comp_id = msg.get("49").ok_or("Missing SenderCompID (49)")?.clone();
    let target_comp_id = msg.get("56").ok_or("Missing TargetCompID (56)")?.clone();
    
    let sid = format!("{}:{}", sender_comp_id, target_comp_id);
    *session_id = Some(sid.clone());
    
    // Create or update session
    {
        let mut sessions = fix_state.sessions.lock().unwrap();
        sessions.insert(sid.clone(), FixSession {
            sender_comp_id: sender_comp_id.clone(),
            target_comp_id: target_comp_id.clone(),
            msg_seq_num: 1,
            expected_seq_num: 2,
            logged_in: true,
            last_heartbeat: Utc::now().timestamp(),
        });
    }
    
    info!("Session established: {}", sid);
    
    // Send Logon response
    let response = create_fix_message(hashmap!{
        "35" => "A", // Logon
        "49" => &target_comp_id,
        "56" => &sender_comp_id,
        "34" => "1",
        "52" => &get_utc_timestamp(),
        "98" => "0", // EncryptMethod
        "108" => &HEARTBEAT_INTERVAL.to_string(), // HeartBtInt
    });
    
    send_fix_message(socket, &response).await
}

async fn handle_heartbeat(
    socket: &mut TcpStream,
    fix_state: &Arc<FixState>,
    msg: &HashMap<String, String>,
    session_id: &Option<String>
) -> Result<(), String> {
    if let Some(sid) = session_id {
        update_heartbeat(fix_state, sid);
        debug!("Heartbeat received from session: {}", sid);
    }
    Ok(())
}

async fn handle_test_request(
    socket: &mut TcpStream,
    fix_state: &Arc<FixState>,
    msg: &HashMap<String, String>,
    session_id: &Option<String>
) -> Result<(), String> {
    let test_req_id = msg.get("112").unwrap_or(&"".to_string()).clone();
    
    if let Some(sid) = session_id {
        let (sender_comp_id, target_comp_id, seq_num) = get_session_info(fix_state, sid)?;
        
        let response = create_fix_message(hashmap!{
            "35" => "0", // Heartbeat
            "49" => &sender_comp_id,
            "56" => &target_comp_id,
            "34" => &seq_num.to_string(),
            "52" => &get_utc_timestamp(),
            "112" => &test_req_id, // TestReqID
        });
        
        send_fix_message(socket, &response).await?;
        increment_seq_num(fix_state, sid);
    }
    
    Ok(())
}

async fn handle_logout(
    socket: &mut TcpStream,
    fix_state: &Arc<FixState>,
    msg: &HashMap<String, String>,
    session_id: &mut Option<String>
) -> Result<(), String> {
    if let Some(sid) = session_id {
        let (sender_comp_id, target_comp_id, seq_num) = get_session_info(fix_state, sid)?;
        
        let response = create_fix_message(hashmap!{
            "35" => "5", // Logout
            "49" => &sender_comp_id,
            "56" => &target_comp_id,
            "34" => &seq_num.to_string(),
            "52" => &get_utc_timestamp(),
            "58" => "Logout acknowledged",
        });
        
        send_fix_message(socket, &response).await?;
        remove_session(fix_state, sid);
        info!("Session logged out: {}", sid);
        *session_id = None;
    }
    
    Ok(())
}

async fn handle_new_order_single(
    socket: &mut TcpStream,
    fix_state: &Arc<FixState>,
    msg: &HashMap<String, String>,
    session_id: &Option<String>
) -> Result<(), String> {
    require_logged_in_session(fix_state, session_id)?;
    
    let symbol = msg.get("55").ok_or("Missing Symbol (55)")?.clone();
    let side_str = msg.get("54").ok_or("Missing Side (54)")?;
    let price_str = msg.get("44").ok_or("Missing Price (44)")?;
    let qty_str = msg.get("38").ok_or("Missing OrderQty (38)")?;
    let cl_ord_id = msg.get("11").ok_or("Missing ClOrdID (11)")?.clone();
    
    // Validate symbol
    if !fix_state.is_valid_symbol(&symbol) {
        return Err(format!("Invalid symbol: {}", symbol));
    }
    
    // Parse values
    let price = price_str.parse::<u64>()
        .map_err(|_| format!("Invalid price: {}", price_str))?;
    let qty = qty_str.parse::<u64>()
        .map_err(|_| format!("Invalid quantity: {}", qty_str))?;
    
    let side = match side_str.as_str() {
        "1" => Side::BUY,
        "2" => Side::SELL,
        _ => return Err(format!("Invalid side: {}", side_str)),
    };
    
    // Create and send event
    let event = Event::new_order(side, price, qty, symbol.clone());
    
    match fix_state.ingress_sender.send(event) {
        Ok(_) => {
            info!("Order accepted: {} {} {} @ {}", side_str, qty, symbol, price);
            
            // Send ExecutionReport (ACK)
            if let Some(sid) = session_id {
                let (sender_comp_id, target_comp_id, seq_num) = get_session_info(fix_state, sid)?;
                
                let response = create_fix_message(hashmap!{
                    "35" => "8", // ExecutionReport
                    "49" => &sender_comp_id,
                    "56" => &target_comp_id,
                    "34" => &seq_num.to_string(),
                    "52" => &get_utc_timestamp(),
                    "11" => &cl_ord_id, // ClOrdID
                    "17" => &generate_exec_id(), // ExecID
                    "55" => &symbol, // Symbol
                    "54" => side_str, // Side
                    "38" => qty_str, // OrderQty
                    "44" => price_str, // Price
                    "39" => "0", // OrdStatus (New)
                    "150" => "0", // ExecType (New)
                });
                
                send_fix_message(socket, &response).await?;
                increment_seq_num(fix_state, sid);
            }
        }
        Err(e) => {
            error!("Failed to send order to ingress: {}", e);
            return Err(format!("Ingress channel error: {}", e));
        }
    }
    
    Ok(())
}

async fn handle_order_status_request(
    socket: &mut TcpStream,
    fix_state: &Arc<FixState>,
    msg: &HashMap<String, String>,
    session_id: &Option<String>
) -> Result<(), String> {
    require_logged_in_session(fix_state, session_id)?;
    
    let symbol = msg.get("55").ok_or("Missing Symbol (55)")?.clone();
    
    if !fix_state.is_valid_symbol(&symbol) {
        return Err(format!("Invalid symbol: {}", symbol));
    }
    
    if let Some(sid) = session_id {
        let (sender_comp_id, target_comp_id, seq_num) = get_session_info(fix_state, sid)?;
        
        // Mock order book data (in real implementation, this would query the order book)
        let response = create_fix_message(hashmap!{
            "35" => "8", // ExecutionReport (used for order book status)
            "49" => &sender_comp_id,
            "56" => &target_comp_id,
            "34" => &seq_num.to_string(),
            "52" => &get_utc_timestamp(),
            "55" => &symbol, // Symbol
            "58" => &format!("Order book status for {}", symbol), // Text
            "39" => "0", // OrdStatus
            "150" => "I", // ExecType (Order Status)
        });
        
        send_fix_message(socket, &response).await?;
        increment_seq_num(fix_state, sid);
        
        info!("Order book status requested for symbol: {}", symbol);
    }
    
    Ok(())
}

async fn handle_security_list_request(
    socket: &mut TcpStream,
    fix_state: &Arc<FixState>,
    msg: &HashMap<String, String>,
    session_id: &Option<String>
) -> Result<(), String> {
    require_logged_in_session(fix_state, session_id)?;
    
    let security_req_id = msg.get("320").unwrap_or(&generate_request_id()).clone();
    
    if let Some(sid) = session_id {
        let symbols = fix_state.get_symbols();
        let (sender_comp_id, target_comp_id, seq_num) = get_session_info(fix_state, sid)?;
        
        let response = create_fix_message(hashmap!{
            "35" => "y", // SecurityList
            "49" => &sender_comp_id,
            "56" => &target_comp_id,
            "34" => &seq_num.to_string(),
            "52" => &get_utc_timestamp(),
            "320" => &security_req_id, // SecurityReqID
            "322" => "0", // SecurityRequestResult (Valid request)
            "393" => &symbols.len().to_string(), // TotNoRelatedSym
            "146" => &symbols.len().to_string(), // NoRelatedSym
            "55" => &symbols.join(","), // Symbols (concatenated for simplicity)
        });
        
        send_fix_message(socket, &response).await?;
        increment_seq_num(fix_state, sid);
        
        info!("Security list sent: {:?}", symbols);
    }
    
    Ok(())
}

async fn handle_security_definition_request(
    socket: &mut TcpStream,
    fix_state: &Arc<FixState>,
    msg: &HashMap<String, String>,
    session_id: &Option<String>
) -> Result<(), String> {
    require_logged_in_session(fix_state, session_id)?;
    
    let symbol = msg.get("55").ok_or("Missing Symbol (55)")?.clone();
    let security_req_id = msg.get("320").unwrap_or(&generate_request_id()).clone();
    
    let success = fix_state.add_symbol(symbol.clone());
    
    if let Some(sid) = session_id {
        let (sender_comp_id, target_comp_id, seq_num) = get_session_info(fix_state, sid)?;
        
        let (result, text) = if success {
            ("0", format!("Symbol {} created successfully", symbol))
        } else {
            ("1", format!("Symbol {} already exists", symbol))
        };
        
        let response = create_fix_message(hashmap!{
            "35" => "d", // SecurityDefinition
            "49" => &sender_comp_id,
            "56" => &target_comp_id,
            "34" => &seq_num.to_string(),
            "52" => &get_utc_timestamp(),
            "320" => &security_req_id, // SecurityReqID
            "322" => result, // SecurityRequestResult
            "55" => &symbol, // Symbol
            "58" => &text, // Text
        });
        
        send_fix_message(socket, &response).await?;
        increment_seq_num(fix_state, sid);
        
        info!("Symbol creation request: {} - {}", symbol, text);
    }
    
    Ok(())
}

// Helper functions

fn find_complete_fix_message(buf: &[u8]) -> Option<usize> {
    // Look for checksum tag "10=" followed by 3 digits and a delimiter
    if let Some(pos) = buf.windows(3).position(|w| w == b"10=") {
        let start = pos + 3;
        if buf.len() > start + 3 {
            // Look for delimiter after checksum
            if let Some(rel_idx) = buf[start + 3..].iter().position(|&b| b == SOH || b == b'|' || b == b'\n') {
                return Some(start + 3 + rel_idx + 1);
            }
        }
    }
    
    // Fallback: look for message terminator
    if let Some(pos) = buf.iter().position(|&b| b == SOH || b == b'\n') {
        return Some(pos + 1);
    }
    
    None
}

fn parse_fix_message(bytes: &[u8]) -> Result<HashMap<String, String>, String> {
    let s = String::from_utf8_lossy(bytes);
    let mut map = HashMap::new();
    
    // Determine delimiter
    let delimiter = if s.contains('\u{0001}') { '\u{0001}' } else { '|' };
    
    for field in s.split(delimiter) {
        let field = field.trim();
        if field.is_empty() { continue; }
        
        if let Some(eq_pos) = field.find('=') {
            let key = field[..eq_pos].to_string();
            let value = field[eq_pos + 1..].to_string();
            map.insert(key, value);
        }
    }
    
    Ok(map)
}

fn create_fix_message(fields: HashMap<String, String>) -> String {
    let mut msg = String::new();

    // Add fields (HashMap iteration order is unspecified)
    for (tag, value) in fields {
        msg.push_str(&format!("{}={}{}", tag, value, SOH as char));
    }

    // Calculate and append checksum
    let checksum = calculate_checksum(&msg);
    msg.push_str(&format!("10={:03}{}", checksum, SOH as char));

    msg
}

fn calculate_checksum(msg: &str) -> u8 {
    // Sum bytes modulo 256; if msg is empty, returns 0
    let sum: u32 = msg.bytes().map(|b| b as u32).fold(0u32, |acc, b| acc.wrapping_add(b));
    (sum % 256) as u8
}

async fn send_fix_message(socket: &mut TcpStream, message: &str) -> Result<(), String> {
    socket.write_all(message.as_bytes()).await
        .map_err(|e| format!("Failed to send FIX message: {}", e))?;
    debug!("Sent FIX message: {}", sanitize_fix_message(message));
    Ok(())
}

async fn send_reject(
    socket: &mut TcpStream, 
    msg: &HashMap<String, String>, 
    reason: &str
) -> Result<(), String> {
    let ref_msg_type = msg.get("35").unwrap_or(&"".to_string()).clone();
    
    let reject_msg = create_fix_message(hashmap!{
        "35" => "3", // Reject
        "49" => "MATCHING_ENGINE",
        "56" => "CLIENT",
        "34" => "1",
        "52" => &get_utc_timestamp(),
        "45" => &ref_msg_type, // RefMsgType
        "58" => reason, // Text
        "371" => "0", // RefTagID
        "372" => "0", // RefMsgType
        "373" => "5", // SessionRejectReason (Other)
    });
    
    send_fix_message(socket, &reject_msg).await
}

// Session management helpers

fn get_session_info(fix_state: &Arc<FixState>, session_id: &str) -> Result<(String, String, u32), String> {
    let sessions = fix_state.sessions.lock().unwrap();
    let session = sessions.get(session_id).ok_or("Session not found")?;
    Ok((session.target_comp_id.clone(), session.sender_comp_id.clone(), session.msg_seq_num))
}

fn increment_seq_num(fix_state: &Arc<FixState>, session_id: &str) {
    let mut sessions = fix_state.sessions.lock().unwrap();
    if let Some(session) = sessions.get_mut(session_id) {
        session.msg_seq_num += 1;
    }
}

fn update_heartbeat(fix_state: &Arc<FixState>, session_id: &str) {
    let mut sessions = fix_state.sessions.lock().unwrap();
    if let Some(session) = sessions.get_mut(session_id) {
        session.last_heartbeat = Utc::now().timestamp();
    }
}

fn remove_session(fix_state: &Arc<FixState>, session_id: &str) {
    let mut sessions = fix_state.sessions.lock().unwrap();
    sessions.remove(session_id);
}

fn require_logged_in_session(fix_state: &Arc<FixState>, session_id: &Option<String>) -> Result<(), String> {
    let session_id = session_id.as_ref().ok_or("No active session")?;
    let sessions = fix_state.sessions.lock().unwrap();
    let session = sessions.get(session_id).ok_or("Session not found")?;
    
    if !session.logged_in {
        return Err("Session not logged in".to_string());
    }
    
    Ok(())
}

// Utility functions

fn get_utc_timestamp() -> String {
    Utc::now().format("%Y%m%d-%H:%M:%S").to_string()
}

fn generate_exec_id() -> String {
    format!("EXEC{}", Utc::now().timestamp_millis())
}

fn generate_request_id() -> String {
    format!("REQ{}", Utc::now().timestamp_millis())
}

fn sanitize_fix_message(msg: &str) -> String {
    msg.replace(SOH as char, "|").replace('\n', "\\n")
}

// Macro for easy HashMap creation
macro_rules! hashmap {
    ($( $key:expr => $val:expr ),* $(,)?) => {{
        let mut map: HashMap<String, String> = HashMap::new();
        $( map.insert($key.to_string(), $val.to_string()); )*
        map
    }};
}

pub(crate) use hashmap;

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    #[test]
    fn test_fix_message_creation() {
        let fields = hashmap!{
            "35" => "D",
            "49" => "SENDER",
            "56" => "TARGET",
            "34" => "1"
        };
        
        let msg = create_fix_message(fields);
        assert!(msg.contains("35=D"));
        assert!(msg.contains("10="));
        assert!(msg.ends_with(&format!("{}", SOH as char)));
    }

    #[test]
    fn test_fix_message_parsing() {
        let msg = format!("35=D{}49=SENDER{}56=TARGET{}10=123{}", SOH as char, SOH as char, SOH as char, SOH as char);
        let parsed = parse_fix_message(msg.as_bytes()).unwrap();
        
        assert_eq!(parsed.get("35"), Some(&"D".to_string()));
        assert_eq!(parsed.get("49"), Some(&"SENDER".to_string()));
        assert_eq!(parsed.get("56"), Some(&"TARGET".to_string()));
    }

    #[test]
    fn test_checksum_calculation() {
        let msg = "35=D\x0149=SENDER\x01";
        let checksum = calculate_checksum(msg);
        assert!(checksum < 256);
    }

    #[test]
    fn test_fix_state_symbol_management() {
        let (sender, _) = unbounded();
        let state = FixState::new(sender);
        
        // Test adding new symbol
        assert!(state.add_symbol("NVDA".to_string()));
        assert!(state.is_valid_symbol("NVDA"));
        
        // Test duplicate symbol
        assert!(!state.add_symbol("NVDA".to_string()));
        
        // Test symbol list
        let symbols = state.get_symbols();
        assert!(symbols.contains(&"NVDA".to_string()));
    }
}