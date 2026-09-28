mod agent;
mod model;
mod tools;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut registry = tools::Registry::new();
    registry.register(Box::new(tools::ClassifyEvent));
    registry.register(Box::new(tools::WindowStats));

    let agent = agent::Agent::new(model::ModelClient::new(), registry);

    let task = "A robot telemetry feed produced event sequence numbers \
[45, 38, 71, 55, 44]. The receive window is 40..=60. \
Classify the events, then report how many were delivered, duplicate, and late.";

    let answer = agent.run(task).await?;
    println!("=== FINAL ANSWER ===\n{}", answer);
    Ok(())
}