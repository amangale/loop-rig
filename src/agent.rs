use crate::model::{ChatMessage, ModelClient};
use crate::tools::Registry;
use anyhow::{bail, Context, Result};
use serde_json::Value;

const MAX_STEPS: usize = 6;

enum StepOutcome {
    Call { tool: String, input: Value },
    Finish(String),
}

pub struct Agent {
    model: ModelClient,
    tools: Registry,
}

impl Agent {
    pub fn new(model: ModelClient, tools: Registry) -> Self {
        Self { model, tools }
    }

    fn system_prompt(&self) -> String {
        format!(
            "You are a small autonomous agent operating a sensor-data pipeline.

You communicate ONLY in JSON. Every reply must be exactly one JSON object:

To call a tool:
{{\"thought\": \"brief reasoning\", \"action\": {{\"tool\": \"<name>\", \"input\": {{...}}}}}}

When the task is complete:
{{\"thought\": \"brief reasoning\", \"final\": \"your answer to the user\"}}

Do not reply with prose outside JSON. Do not invent tool names.

Available tools:
{}",
            self.tools
                .tools_catalog()
        )
    }

    fn parse_step(&self, raw: &str) -> Result<StepOutcome> {
        // Local models sometimes wrap JSON in code fences. Strip them.
        let cleaned = raw
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        let v: Value =
            serde_json::from_str(cleaned).context("reply was not valid JSON")?;

        if let Some(final_value) = v.get("final") {
            let final_answer = match final_value {
                Value::String(s) => s.clone(),
                other => serde_json::to_string_pretty(other)
                    .context("serializing final answer")?,
            };
            return Ok(StepOutcome::Finish(final_answer));
        }
        if let Some(action) = v.get("action") {
            let tool = action
                .get("tool")
                .and_then(Value::as_str)
                .context("action present but action.tool missing")?
                .to_string();
            let input = action.get("input").cloned().unwrap_or(Value::Null);
            return Ok(StepOutcome::Call { tool, input });
        }
        bail!("JSON contained neither 'final' nor 'action'");
    }

    pub async fn run(&self, task: &str) -> Result<String> {
        let mut messages = vec![
            ChatMessage::system(self.system_prompt()),
            ChatMessage::user(task.to_string()),
        ];

        for step in 1..=MAX_STEPS {
            let raw = self.model.chat(&messages).await?;
            println!("--- step {} ---\n{}\n", step, raw);
            messages.push(ChatMessage::assistant(raw.clone()));

            match self.parse_step(&raw) {
                Ok(StepOutcome::Finish(answer)) => {
                    return Ok(answer);
                }
                Ok(StepOutcome::Call { tool, input }) => {
                    let observation = match self.tools.find(&tool) {
                        Some(t) => match t.execute(input) {
                            Ok(out) => format!("Observation: {}", out),
                            Err(e) => format!("Observation: tool error: {e:#}"),
                        },
                        None => format!(
                            "Observation: unknown tool '{}'. Available tools: {}",
                            tool,
                            self.tools.names()
                        ),
                    };
                    println!("{}\n", observation);
                    messages.push(ChatMessage::user(observation));
                }
                Err(e) => {
                    // Malformed reply: feed the error back instead of crashing.
                    // The model gets to self-correct — but the step still counted.
                    let msg = format!(
                        "Your reply was rejected: {e:#}. Reply again as a single \
JSON object with either 'action' or 'final'."
                    );
                    messages.push(ChatMessage::user(msg));
                }
            }
        }
        bail!("agent exhausted MAX_STEPS ({}) without finishing", MAX_STEPS);
    }
}