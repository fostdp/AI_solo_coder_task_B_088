mod models;
mod acoustics;
mod sti;
mod alerts;
mod storage;
mod routes;

use crate::alerts::AlertManager;
use crate::routes::{AppState, create_router};
use crate::storage::ClickHouseStore;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");

    info!("=====================================================");
    info!("  天坛回音壁声学仿真与语音清晰度分析系统 - Rust 后端");
    info!("=====================================================");

    let ch_url = std::env::var("CLICKHOUSE_URL")
        .unwrap_or_else(|_| "http://localhost:8123".to_string());
    let ch_database = std::env::var("CLICKHOUSE_DATABASE")
        .unwrap_or_else(|_| "tiantan_acoustics".to_string());
    let mqtt_host = std::env::var("MQTT_HOST")
        .unwrap_or_else(|_| "localhost".to_string());
    let mqtt_port: u16 = std::env::var("MQTT_PORT")
        .unwrap_or_else(|_| "1883".to_string())
        .parse()
        .unwrap_or(1883);
    let mqtt_topic = std::env::var("MQTT_BASE_TOPIC")
        .unwrap_or_else(|_| "tiantan/acoustics".to_string());
    let listen_host = std::env::var("LISTEN_HOST")
        .unwrap_or_else(|_| "0.0.0.0".to_string());
    let listen_port: u16 = std::env::var("LISTEN_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .unwrap_or(8080);

    info!("ClickHouse: {} / {}", ch_url, ch_database);
    info!("MQTT Broker: {}:{}", mqtt_host, mqtt_port);
    info!("MQTT Base Topic: {}", mqtt_topic);

    let store = Arc::new(ClickHouseStore::new(&ch_url, &ch_database));
    let alerts = Arc::new(
        AlertManager::new(&mqtt_host, mqtt_port, "tiantan-backend", &mqtt_topic)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to connect to MQTT broker ({}), continuing without MQTT", e);
                let client = rumqttc::AsyncClient::new(
                    rumqttc::MqttOptions::new("disabled", "localhost", 1883),
                    1,
                ).0;
                alerts::AlertManager {
                    client,
                    base_topic: mqtt_topic,
                    thresholds: alerts::AlertThresholds::default(),
                    alert_history: Arc::new(tokio::sync::Mutex::new(Vec::new())),
                }
            }),
    );

    let app_state = Arc::new(AppState {
        store: store.clone(),
        alerts: alerts.clone(),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = create_router(app_state)
        .layer(cors);

    let addr: SocketAddr = format!("{}:{}", listen_host, listen_port).parse()?;
    info!("HTTP服务监听: http://{}", addr);
    info!("可用API端点:");
    info!("  GET  /api/health                - 健康检查");
    info!("  GET  /api/sites                 - 获取场所列表");
    info!("  GET  /api/sensors               - 获取传感器列表");
    info!("  GET  /api/measurements          - 获取声学测量数据");
    info!("  POST /api/measurements          - 上报声学测量数据");
    info!("  GET  /api/sound-paths           - 获取声传播路径");
    info!("  GET  /api/intelligibility       - 获取语音清晰度分析");
    info!("  POST /api/intelligibility       - 提交STI分析");
    info!("  GET  /api/alerts                - 获取告警信息");
    info!("  GET  /api/sound-field/{site_id} - 获取声场快照");
    info!("  POST /api/simulate/acoustics    - 运行几何声学仿真");
    info!("  POST /api/simulate/sti          - 运行STI语音清晰度分析");
    info!("  POST /api/simulate/wave-field   - 运行波动声学声场仿真");
    info!("  GET  /api/stats                 - 获取系统统计");
    info!("=====================================================");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
