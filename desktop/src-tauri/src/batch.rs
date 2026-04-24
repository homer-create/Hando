use crate::trash::Disposal;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Default, Clone)]
pub struct Batch {
    pub id: String,
    pub disposals: Vec<Disposal>,
}

#[derive(Default)]
pub struct BatchState {
    pub current: Mutex<HashMap<String, Batch>>,
    pub last_complete: Mutex<Option<Batch>>,
}

impl BatchState {
    pub fn start(&self, id: String) {
        let mut cur = self.current.lock().unwrap();
        cur.insert(id.clone(), Batch { id, disposals: vec![] });
    }
    pub fn record_disposal(&self, batch_id: &str, disposal: Disposal) {
        let mut cur = self.current.lock().unwrap();
        if let Some(b) = cur.get_mut(batch_id) {
            b.disposals.push(disposal);
        }
    }
    pub fn complete(&self, batch_id: &str) {
        let mut cur = self.current.lock().unwrap();
        if let Some(b) = cur.remove(batch_id) {
            *self.last_complete.lock().unwrap() = Some(b);
        }
    }
    pub fn take_last(&self) -> Option<Batch> {
        self.last_complete.lock().unwrap().take()
    }
}
