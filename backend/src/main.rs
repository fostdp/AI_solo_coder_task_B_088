mod models;
mod acoustic_simulator;
mod clarity_analyzer;
mod alarm_mqtt;
mod dtu_receiver;
mod storage;
mod routes;

use crate::alarm_mqtt::AlarmTask;
use crate::acoustic_simulator::SimulatorTask;
use crate::clarity_analyzer::AnalyzerTask;
use crate::dtu_receiver::DtuReceiver;
use crate::models::{AcousticConfig, StiWeightsConfig, AlarmEvent, SimulatorRequest, AnalyzerRequest};
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

    let config_path = std::env::var("ACOUSTIC_CONFIG_PATH")
        .unwrap_or_else(|_| "config/acoustic_config.json".to_string());
    let sti_config_path = std::env::var("STI_WEIGHTS_CONFIG_PATH")
        .unwrap_or_else(|_| "config/sti_weights.json".to_string());

    let acoustic_config: AcousticConfig = {
        let text = std::fs::read_to_string(&config_path)
            .unwrap_or_else(|e| panic!("无法加载声学配置文件 {}: {}", config_path, e));
        serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("声学配置文件格式错误: {}", e))
    };
    let sti_config: StiWeightsConfig = {
        let text = std::fs::read_to_string(&sti_config_path)
            .unwrap_or_else(|e| panic!("无法加载STI权重配置文件 {}: {}", sti_config_path, e));
        serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("STI权重配置文件格式错误: {}", e))
    };

    let acoustic_config = Arc::new(acoustic_config);
    let sti_config = Arc::new(sti_config);

    info!("配置已加载: 声学配置从 {}, STI权重从 {}", config_path, sti_config_path);
    info!("ClickHouse: {} / {}", ch_url, ch_database);
    info!("MQTT Broker: {}:{}", mqtt_host, mqtt_port);

    let store = Arc::new(ClickHouseStore::new(&ch_url, &ch_database, acoustic_config.clone()));

    let (alarm_tx, alarm_rx) = tokio::sync::mpsc::channel::<AlarmEvent>(256);
    let (sim_tx, sim_rx) = tokio::sync::mpsc::channel::<SimulatorRequest>(64);
    let (analyzer_tx, analyzer_rx) = tokio::sync::mpsc::channel::<AnalyzerRequest>(64);

    let alarm_task = AlarmTask::new(
        &mqtt_host, mqtt_port, "tiantan-backend",
        &mqtt_topic,
        acoustic_config.alert_thresholds.clone(),
        alarm_rx,
        store.clone(),
    );
    tokio::spawn(async move { alarm_task.run().await });

    let sim_task = SimulatorTask::new(acoustic_config.clone(), sim_rx);
    tokio::spawn(async move { sim_task.run().await });

    let analyzer_task = AnalyzerTask::new((*sti_config).clone(), analyzer_rx, alarm_tx.clone());
    tokio::spawn(async move { analyzer_task.run().await });

    let dtu_receiver = Arc::new(DtuReceiver::new(
        (*acoustic_config).clone(),
        tokio::sync::mpsc::channel::<crate::models::DtuEvent>(256).0,
        alarm_tx,
        store.clone(),
    ));

    let app_state = Arc::new(AppState {
        store: store.clone(),
        dtu_receiver,
        sim_tx,
        analyzer_tx,
        config: acoustic_config.clone(),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = create_router(app_state).layer(cors);

    let addr: SocketAddr = format!("{}:{}", listen_host, listen_port).parse()?;
    info!("HTTP服务监听: http://{}", addr);
    info!("模块拓扑: dtu_receiver → alarm_mqtt, acoustic_simulator (RPC), clarity_analyzer (RPC) → alarm_mqtt");
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
    info!("  GET  /api/sound-field/{{site_id}} - 获取声场快照");
    info!("  POST /api/simulate/acoustics    - 运行几何声学仿真");
    info!("  POST /api/simulate/sti          - 运行STI语音清晰度分析");
    info!("  POST /api/simulate/wave-field   - 运行波动声学声场仿真");
    info!("  GET  /api/stats                 - 获取系统统计");
    info!("=====================================================");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
