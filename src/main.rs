use anyhow::Error;
use crabot_runtime::{Runtime, RuntimeCommandResult};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    ai::init();

    let runtime = Runtime::default();
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        print_startup(&runtime);
        return Ok(());
    }

    let input = args.join(" ");
    if input.trim_start().starts_with('/') {
        print_command_result(runtime.handle_slash_command(&input)?);
    } else {
        print_command_result(RuntimeCommandResult::Snapshot(runtime.plan(input)));
    }

    Ok(())
}

fn print_startup(runtime: &Runtime) {
    let summary = runtime.bootstrap_summary();
    println!("╭──────────────────────────────────────────────╮");
    println!("│ crabot — company-style agent workspace       │");
    println!("╰──────────────────────────────────────────────╯");
    println!("Company: {}", summary.company.name);
    println!("Departments:");
    for department in summary.company.departments {
        println!("  - {}: {}", department.name, department.mission);
    }
    println!();
    println!(
        "Runtime ready: {} skills · {} slash commands · {} capabilities",
        summary.skill_count, summary.command_count, summary.capability_count
    );
    println!("Try:");
    println!("  cargo run -- /status");
    println!("  cargo run -- /skills");
    println!("  cargo run -- /tools");
    println!("  cargo run -- /plan 优化代码，支持 claude skill、planner、tool 和 MCP");
    println!();
    println!("Layout target: sessions | planner graph | conversation | tool timeline | artifacts");
}

fn print_command_result(result: RuntimeCommandResult) {
    match result {
        RuntimeCommandResult::Text(text) => println!("{}", text),
        RuntimeCommandResult::Snapshot(snapshot) => {
            println!("Session: {}", snapshot.session.id);
            println!("Objective: {}", snapshot.session.objective);
            println!("Phase: {:?}", snapshot.session.phase);
            if let Some(graph) = snapshot.graph {
                println!("\nPlanner DAG:");
                for node in graph.nodes {
                    println!(
                        "  - [{:?}] {} :: {} -> {:?}/{}",
                        node.status,
                        node.department,
                        node.title,
                        node.assignee.source,
                        node.assignee.name
                    );
                    println!("    goal: {}", node.goal);
                    if !node.dependencies.is_empty() {
                        println!("    depends_on: {:?}", node.dependencies);
                    }
                }
            }
            println!("\nEvents:");
            for event in snapshot.events {
                println!("  - {:?}", event.kind);
            }
        }
    }
}
