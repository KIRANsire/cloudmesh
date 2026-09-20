mod agent;
mod api;
mod config;
mod db;
mod models;
mod telemetry;

use std::sync::Arc;
use config::AppConfig;
use api::{create_app, AppState};
use agent::AgentWorker;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Determine intent from the raw OS environment BEFORE loading .env.
    // If CLOUDMESH_SERVER_URL is explicitly set in the shell and DATABASE_URL
    // is NOT set in the shell, this process is intentionally an Agent.
    // In that case we must NOT load .env, because .env contains DATABASE_URL
    // which would incorrectly switch us into Server mode.
    let raw_has_server_url = std::env::var("CLOUDMESH_SERVER_URL").is_ok();
    let raw_has_database_url = std::env::var("DATABASE_URL").is_ok();
    let is_explicit_agent = raw_has_server_url && !raw_has_database_url;

    if !is_explicit_agent {
        // Server mode: load .env so DATABASE_URL and other vars are available.
        dotenvy::dotenv().ok();
    }
    // Agent mode: .env is intentionally NOT loaded. The shell env is the
    // sole source of truth so DATABASE_URL can never sneak in from the file.

    // Parse configuration
    let config = AppConfig::load();

    println!("Starting CloudMesh ...");

    if config.is_server() {
        // Run as Server (and potentially Agent)
        let database_url = config.database_url.as_ref().unwrap();
        
        let db_pool = db::create_pool(database_url).await?;
        println!("Connected to PostgreSQL");

        let app_state = AppState {
            db_pool,
            config: Arc::new(config.clone()),
        };

        let app = create_app(app_state);

        // Start HTTP server
        let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
        println!("CloudMesh Server running on http://127.0.0.1:3000");

        // Run local Agent in the background
        if config.cloudmesh_server_url.is_some() {
            println!("Starting local AgentWorker for node: {}", config.node_id);
            let agent = AgentWorker::new(config);
            tokio::spawn(async move {
                agent.run().await;
            });
        } else {
            println!("No CLOUDMESH_SERVER_URL, skipping local AgentWorker.");
        }

        axum::serve(listener, app).await?;
    } else {
        // Run as Agent Only
        if config.cloudmesh_server_url.is_none() {
            eprintln!("Error: Agent mode requires CLOUDMESH_SERVER_URL to be set.");
            std::process::exit(1);
        }

        println!("CloudMesh starting in Agent-Only mode for node: {}", config.node_id);
        let agent = AgentWorker::new(config);
        agent.run().await;
    }

    Ok(())
}