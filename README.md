# loop-rig

A minimal LLM agent loop in Rust: hand-rolled, no agent frameworks, no native
tool-calling APIs. Just a model endpoint, a JSON protocol, and guardrails.

```
$ cargo run --release
--- step 1 ---
{"thought": "Use window_stats to classify all events...", "action": {...}}
Observation: {"delivered":3,"duplicate":1,"late":1,...}
--- step 2 ---
{"thought": "The events have been classified.", "final": {"delivered": 3, "duplicate": 1, "late": 1}}
```

## Why hand-rolled

Agent frameworks abstract away exactly the parts that matter for
operational judgment: the wire protocol, the state transitions, and the
failure modes. This project keeps them visible on purpose:

- **Auditable protocol.** The agent speaks a small JSON contract
  (`{"thought", "action"}` to call a tool, `{"final"}` to stop). Every step
  in the transcript is a plain JSON object that can be logged, replayed,
  and diffed — useful precisely because LLM agents are otherwise opaque.
- **Portable.** Works against any OpenAI-compatible endpoint. Built and
  tested against local [Ollama](https://ollama.com) models
  (qwen2.5:14b) — nothing leaves the machine.
- **Guardrails are structural.** A hard iteration cap, per-request timeouts,
  unknown-tool rejection fed back as observations, and malformed replies
  that cost a step but don't crash the loop. The agent cannot run away.

## Architecture

```
src/
├── main.rs    # wires registry + client, defines the task
├── model.rs   # OpenAI-compatible chat client (reqwest, rustls)
├── tools.rs   # Tool trait, registry, sensor-event tools
└── agent.rs   # the loop: parse → dispatch → observe → repeat
```

Tools implement a trait (`name`, `description`, `execute`) and are
dispatched by name through a registry — the same structural-typing idea
behind MCP-style tool surfaces, but visible and small enough to read in
one sitting:

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> String;
    fn execute(&self, input: Value) -> Result<Value>;
}
```

The bundled tools reuse the sensor-event state machine from
[sense-rig](https://github.com/amangale/sense-rig) (classify sequence
numbers against a receive window as `delivered` / `duplicate` / `late`),
so the agent operates on a familiar domain: the delivery-semantics
questions that matter in fleet telemetry.

## Running

```bash
ollama pull qwen2.5:14b     # or any local OpenAI-compatible model
cargo run --release

# override model / endpoint:
LOOP_RIG_MODEL=llama3.1:8b cargo run --release
LOOP_RIG_BASE_URL=http://localhost:11434/v1 cargo run --release
```

## Lessons so far

The first full run surfaced a textbook agent failure: the model emitted a
structured `final` (a JSON object) where the loop expected a string, the
parser silently rejected it, and the loop re-prompted until the iteration
cap stopped it. Two takeaways worth recording:

1. **Contract drift is the default**, not the exception — models interpret
   free-text specs liberally, and parsers must accept the superset or fail
   loudly.
2. **The iteration cap did its job.** A buggy parser fed a stable model
   garbage for four rounds and nothing ran away. Guardrails earn their
   keep exactly when something else is broken.

## Roadmap

- Sense-rig as a path dependency instead of copied state-machine logic
- Tracing spans per loop iteration (OpenTelemetry)
- A comparison harness: LLM reasoner vs. a deterministic decision engine
  on identical tasks, scored
- Replay mode: re-execute a recorded transcript deterministically

## Related experiments

- [`sense-rig`](https://github.com/amangale/sense-rig) — Rust sensor-data
  pipeline, now also a Python package via PyO3
- [`bidi-rig`](https://github.com/amangale/bidi-rig) — gRPC bidirectional
  streaming under injected failure, in Go

## License

MIT