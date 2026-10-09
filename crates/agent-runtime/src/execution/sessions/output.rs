use serde_json::{Value, json};
use std::collections::VecDeque;

const MAX_BYTES: usize = 1024 * 1024;
pub(super) struct Output {
    chunks: VecDeque<(u64, bool, Vec<u8>)>,
    bytes: usize,
    seq: u64,
    status: &'static str,
    exit_code: Option<i32>,
    private: bool,
}
impl Output {
    pub(super) fn new() -> Self {
        Self {
            chunks: VecDeque::new(),
            bytes: 0,
            seq: 0,
            status: "running",
            exit_code: None,
            private: false,
        }
    }
    pub(super) fn append(&mut self, stderr: bool, bytes: &[u8]) {
        // After private human input, no later output enters model-visible storage.
        // A child may echo a secret in transformed form, so substring redaction is insufficient.
        self.seq += 1;
        self.bytes += bytes.len();
        self.chunks.push_back((self.seq, stderr, bytes.into()));
        while self.bytes > MAX_BYTES || self.chunks.len() > 256 {
            if let Some((_, _, old)) = self.chunks.pop_front() {
                self.bytes -= old.len();
            }
        }
    }
    pub(super) fn private(&mut self) {
        self.private = true;
    }
    pub(super) fn finish(&mut self, status: &'static str, code: Option<i32>) {
        self.status = status;
        self.exit_code = code;
    }
    pub(super) fn running(&self) -> bool {
        self.status == "running"
    }
    pub(super) fn view(&self, after: u64, human: bool) -> Value {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        if human || !self.private {
            for (_, err, bytes) in self.chunks.iter().filter(|(seq, _, _)| *seq > after) {
                if *err {
                    stderr.extend_from_slice(bytes)
                } else {
                    stdout.extend_from_slice(bytes)
                }
            }
        }
        json!({"status":self.status,"success":if self.running(){None}else{Some(self.status=="completed"&&self.exit_code==Some(0))},"exit_code":self.exit_code,"stdout":String::from_utf8_lossy(&stdout),"stderr":String::from_utf8_lossy(&stderr),"next_cursor":self.seq,"truncated":self.chunks.front().is_some_and(|(seq,_,_)|*seq>after.saturating_add(1)),"private_output":self.private,"notice":if self.private&&!human {"Output hidden after private human input; ask the user to confirm the outcome."}else{""}})
    }
}
