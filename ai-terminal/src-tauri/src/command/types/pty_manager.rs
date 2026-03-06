use portable_pty::{Child, MasterPty};
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};

pub struct PtySession {
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
}

pub struct PtyManager {
    pub sessions: Mutex<HashMap<String, PtySession>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}
