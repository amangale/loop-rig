use anyhow::{Context, Result};
use serde_json::{json, Value};

pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> String;
    fn execute(&self, input: Value) -> Result<Value>;
}

pub struct Registry {
    tools: Vec<Box<dyn Tool>>,
}

impl Registry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    pub fn find(&self, name: &str) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .map(|t| t.as_ref())
    }

    pub fn names(&self) -> String {
        self.tools
            .iter()
            .map(|t| t.name().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn tools_catalog(&self) -> String {
        self.tools
            .iter()
            .map(|t| format!("- {}: {}", t.name(), t.description()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// Core classification logic — same semantics as sense-rig's state machine.
fn classify(seq: u64, window_start: u64, window_end: u64) -> &'static str {
    if seq < window_start {
        "duplicate"
    } else if seq > window_end {
        "late"
    } else {
        "delivered"
    }
}

pub struct ClassifyEvent;

impl Tool for ClassifyEvent {
    fn name(&self) -> &'static str {
        "classify_event"
    }

    fn description(&self) -> String {
        "Classify one sensor event sequence number against a receive window. \
Input: {\"seq\": <u64>, \"window_start\": <u64>, \"window_end\": <u64>}. \
Returns {\"class\": \"delivered\"|\"duplicate\"|\"late\"}."
            .into()
    }

    fn execute(&self, input: Value) -> Result<Value> {
        let seq = input
            .get("seq")
            .and_then(Value::as_u64)
            .context("missing or non-u64 field 'seq'")?;
        let ws = input
            .get("window_start")
            .and_then(Value::as_u64)
            .context("missing or non-u64 field 'window_start'")?;
        let we = input
            .get("window_end")
            .and_then(Value::as_u64)
            .context("missing or non-u64 field 'window_end'")?;
        Ok(json!({ "class": classify(seq, ws, we) }))
    }
}

pub struct WindowStats;

impl Tool for WindowStats {
    fn name(&self) -> &'static str {
        "window_stats"
    }

    fn description(&self) -> String {
        "Classify a batch of sensor event sequence numbers against a receive window \
and return per-class counts. Input: {\"events\": [<u64>, ...], \
\"window_start\": <u64>, \"window_end\": <u64>}. \
Returns {\"delivered\": n, \"duplicate\": n, \"late\": n, \"detail\": [...]}."
            .into()
    }

    fn execute(&self, input: Value) -> Result<Value> {
        let events: Vec<u64> = input
            .get("events")
            .and_then(Value::as_array)
            .context("missing field 'events'")?
            .iter()
            .map(|v| v.as_u64().context("non-u64 value in 'events'"))
            .collect::<Result<Vec<u64>>>()?;
        let ws = input
            .get("window_start")
            .and_then(Value::as_u64)
            .context("missing or non-u64 field 'window_start'")?;
        let we = input
            .get("window_end")
            .and_then(Value::as_u64)
            .context("missing or non-u64 field 'window_end'")?;

        let (mut d, mut du, mut l) = (0u64, 0u64, 0u64);
        let detail: Vec<Value> = events
            .iter()
            .map(|&seq| {
                let c = classify(seq, ws, we);
                match c {
                    "delivered" => d += 1,
                    "duplicate" => du += 1,
                    "late" => l += 1,
                    _ => {}
                }
                json!({ "seq": seq, "class": c })
            })
            .collect();

        Ok(json!({
            "delivered": d,
            "duplicate": du,
            "late": l,
            "detail": detail
        }))
    }
}