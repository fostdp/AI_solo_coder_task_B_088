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
use metrics_exporter_prometheus::PrometheusBuilder;
use metrics_util::layers::PrefixLayer;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::{TraceLayer, DefaultMakeSpan};
use tracing::{info, warn, Level};
use tracing_subscriber::{fmt, EnvFilter};

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=debug"));
    fmt()
        .with_env_filter(filter)
        .json()
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();
}

fn init_metrics() {
    let recorder = PrometheusBuilder::new()
        .set_buckets_for_prefix(
            "tiantan",
            vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
        )
        .expect("failed to set histogram buckets")
        .build_recorder();
    let recorder = PrefixLayer::new("tiantan").layer(recorder);
    metrics::set_boxed_recorder(Box::new(recorder))
        .expect("failed to install metrics recorder");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    init_metrics();

    info!("=====================================================");
    info!("  天坛回音壁声学仿真与语音清晰度分析系统 - Rust 后端");
    info!("=====================================================");

    let ch_url = std::env::var("CLICKHOUSE_URL")
        .unwrap_or_else(|_| "http://clickhouse:8123".to_string());
    let ch_database = std::env::var("CLICKHOUSE_DATABASE")
        .unwrap_or_else(|_| "tiantan_acoustics".to_string());
    let mqtt_host = std::env::var("MQTT_HOST")
        .unwrap_or_else(|_| "mosquitto".to_string());
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
    let metrics_port: u16 = std::env::var("METRICS_PORT")
        .unwrap_or_else(|_| "9090".to_string())
        .parse()
        .unwrap_or(9090);

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

    let analyzer_task = AnalyzerTask::new((*sti_config).clone(), acoustic_config.clone(), analyzer_rx, alarm_tx.clone());
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

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO));

    let app = create_router(app_state)
        .layer(trace_layer)
        .layer(cors);

    let metrics_addr: SocketAddr = format!("0.0.0.0:{}", metrics_port).parse()?;
    info!("Prometheus指标端点: http://{}/metrics", metrics_addr);
    tokio::spawn(async move {
        let metrics_app = routes::create_metrics_router();
        let listener = tokio::net::TcpListener::bind(metrics_addr).await.unwrap();
        axum::serve(listener, metrics_app).await.unwrap();
    });

    let addr: SocketAddr = format!("{}:{}", listen_host, listen_port).parse()?;
    info!("HTTP服务监听: http://{}", addr);
    info!("模块拓扑: dtu_receiver → alarm_mqtt, acoustic_simulator (RPC), clarity_analyzer (RPC) → alarm_mqtt");
    info!("=====================================================");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
