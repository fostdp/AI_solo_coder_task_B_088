use crate::dtu_receiver::DtuReceiver;
use crate::models::*;
use crate::storage::ClickHouseStore;
use axum::{
    extract::{Path, State, Query},
    routing::{get, post},
    Json, Router,
    response::IntoResponse,
};
use metrics::{counter, histogram, gauge};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;

pub struct AppState {
    pub store: Arc<ClickHouseStore>,
    pub dtu_receiver: Arc<DtuReceiver>,
    pub sim_tx: tokio::sync::mpsc::Sender<SimulatorRequest>,
    pub analyzer_tx: tokio::sync::mpsc::Sender<AnalyzerRequest>,
    pub config: Arc<AcousticConfig>,
}

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub limit: Option<u32>,
    pub site_id: Option<String>,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/health", get(health_check))
        .route("/api/sites", get(get_sites))
        .route("/api/sensors", get(get_sensors))
        .route("/api/measurements", get(get_measurements).post(post_measurement))
        .route("/api/measurements/{site_id}", get(get_site_measurements))
        .route("/api/sound-paths", get(get_sound_paths).post(post_sound_path))
        .route("/api/intelligibility", get(get_intelligibility).post(post_intelligibility))
        .route("/api/alerts", get(get_alerts).post(post_alert))
        .route("/api/sound-field/{site_id}", get(get_sound_field))
        .route("/api/simulate/acoustics", post(run_acoustic_simulation))
        .route("/api/simulate/sti", post(run_sti_analysis))
        .route("/api/simulate/wave-field", post(run_wave_field_simulation))
        .route("/api/stats", get(get_stats))
        .with_state(state)
}

pub fn create_metrics_router() -> Router {
    Router::new().route("/metrics", get(metrics_endpoint))
}

async fn metrics_endpoint() -> impl IntoResponse {
    let handle = metrics_exporter_prometheus::handle();
    match handle.render() {
        Ok(body) => ([(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")], body),
        Err(_) => ([(axum::http::header::CONTENT_TYPE, "text/plain")], "# ERROR rendering metrics\n".to_string()),
    }
}

async fn health_check() -> Json<ApiResponse<String>> {
    counter!("health_check_total").increment(1);
    Json(ApiResponse::ok("天坛声学仿真系统运行正常".to_string()))
}

async fn get_sites(State(state): State<Arc<AppState>>) -> Json<ApiResponse<Vec<SiteInfo>>> {
    let start = Instant::now();
    let result = state.store.get_sites().await;
    histogram!("http_request_duration_seconds", "endpoint" => "get_sites").record(start.elapsed().as_secs_f64());
    match result {
        Ok(sites) => Json(ApiResponse::ok(sites)),
        Err(e) => { counter!("http_request_errors_total", "endpoint" => "get_sites").increment(1); Json(ApiResponse::error(&e.to_string())) }
    }
}

async fn get_sensors(State(state): State<Arc<AppState>>) -> Json<ApiResponse<Vec<SensorMetadata>>> {
    let start = Instant::now();
    let result = state.store.get_sensors().await;
    histogram!("http_request_duration_seconds", "endpoint" => "get_sensors").record(start.elapsed().as_secs_f64());
    match result {
        Ok(sensors) => Json(ApiResponse::ok(sensors)),
        Err(e) => { counter!("http_request_errors_total", "endpoint" => "get_sensors").increment(1); Json(ApiResponse::error(&e.to_string())) }
    }
}

async fn get_measurements(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Json<ApiResponse<Vec<AcousticMeasurement>>> {
    let site_id = params.site_id.as_deref().unwrap_or("huiyinbi");
    let limit = params.limit.unwrap_or(100);
    let start = Instant::now();
    let result = state.store.get_recent_measurements(site_id, limit).await;
    histogram!("http_request_duration_seconds", "endpoint" => "get_measurements").record(start.elapsed().as_secs_f64());
    match result {
        Ok(data) => Json(ApiResponse::ok(data)),
        Err(e) => Json(ApiResponse::error(&e.to_string())),
    }
}

async fn get_site_measurements(
    State(state): State<Arc<AppState>>,
    Path(site_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Json<ApiResponse<Vec<AcousticMeasurement>>> {
    let limit = params.limit.unwrap_or(100);
    match state.store.get_recent_measurements(&site_id, limit).await {
        Ok(data) => Json(ApiResponse::ok(data)),
        Err(e) => Json(ApiResponse::error(&e.to_string())),
    }
}

async fn post_measurement(
    State(state): State<Arc<AppState>>,
    Json(m): Json<AcousticMeasurement>,
) -> Json<ApiResponse<String>> {
    let start = Instant::now();
    counter!("measurements_received_total", "site" => m.site_id.clone()).increment(1);
    match state.dtu_receiver.receive_measurement(m).await {
        Ok(validated) => {
            histogram!("measurement_processing_duration_seconds").record(start.elapsed().as_secs_f64());
            gauge!("latest_spl_db", "site" => validated.site_id.clone(), "sensor" => validated.sensor_id.clone()).set(validated.sound_pressure_level);
            gauge!("latest_t60_seconds", "site" => validated.site_id.clone(), "sensor" => validated.sensor_id.clone()).set(validated.reverb_time_t60);
            Json(ApiResponse::ok(format!("测量数据已保存: {}", validated.measurement_id)))
        }
        Err(e) => {
            counter!("measurements_invalid_total").increment(1);
            Json(ApiResponse::error(&e.to_string()))
        }
    }
}

async fn get_sound_paths(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Json<ApiResponse<Vec<SoundPath>>> {
    let site_id = params.site_id.as_deref().unwrap_or("huiyinbi");
    let limit = params.limit.unwrap_or(200);
    match state.store.get_recent_paths(site_id, limit).await {
        Ok(data) => Json(ApiResponse::ok(data)),
        Err(e) => Json(ApiResponse::error(&e.to_string())),
    }
}

async fn post_sound_path(
    State(state): State<Arc<AppState>>,
    Json(mut p): Json<SoundPath>,
) -> Json<ApiResponse<String>> {
    p.timestamp = chrono::Utc::now();
    p.path_id = uuid::Uuid::new_v4();
    match state.store.insert_sound_path(&p).await {
        Ok(_) => Json(ApiResponse::ok(format!("声路径已保存: {}", p.path_id))),
        Err(e) => Json(ApiResponse::error(&e.to_string())),
    }
}

async fn get_intelligibility(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Json<ApiResponse<Vec<SpeechIntelligibility>>> {
    let site_id = params.site_id.as_deref().unwrap_or("huiyinbi");
    let limit = params.limit.unwrap_or(50);
    match state.store.get_recent_intelligibility(site_id, limit).await {
        Ok(data) => Json(ApiResponse::ok(data)),
        Err(e) => Json(ApiResponse::error(&e.to_string())),
    }
}

async fn post_intelligibility(
    State(state): State<Arc<AppState>>,
    Json(s): Json<SpeechIntelligibility>,
) -> Json<ApiResponse<String>> {
    match state.store.insert_intelligibility(&s).await {
        Ok(_) => Json(ApiResponse::ok(format!("STI分析结果已保存: {}", s.analysis_id))),
        Err(e) => Json(ApiResponse::error(&e.to_string())),
    }
}

async fn get_alerts(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Json<ApiResponse<Vec<AcousticAlert>>> {
    let limit = params.limit.unwrap_or(50);
    match state.store.get_recent_alerts(params.site_id.as_deref(), limit).await {
        Ok(data) => Json(ApiResponse::ok(data)),
        Err(e) => Json(ApiResponse::error(&e.to_string())),
    }
}

async fn post_alert(
    State(state): State<Arc<AppState>>,
    Json(mut a): Json<AcousticAlert>,
) -> Json<ApiResponse<String>> {
    a.timestamp = chrono::Utc::now();
    a.alert_id = uuid::Uuid::new_v4();
    match state.store.insert_alert(&a).await {
        Ok(_) => Json(ApiResponse::ok(format!("告警已保存: {}", a.alert_id))),
        Err(e) => Json(ApiResponse::error(&e.to_string())),
    }
}

async fn get_sound_field(
    State(state): State<Arc<AppState>>,
    Path(site_id): Path<String>,
) -> Json<ApiResponse<Option<SoundFieldSnapshot>>> {
    match state.store.get_latest_sound_field(&site_id).await {
        Ok(data) => Json(ApiResponse::ok(data)),
        Err(e) => Json(ApiResponse::error(&e.to_string())),
    }
}

async fn run_acoustic_simulation(
    State(state): State<Arc<AppState>>,
    Json(params): Json<SimulationParams>,
) -> Json<ApiResponse<Vec<SoundPath>>> {
    let start = Instant::now();
    if !state.config.valid_site_ids.contains(&params.site_id) {
        return Json(ApiResponse::error("无效的场所ID"));
    }

    counter!("simulations_total", "type" => "acoustic", "site" => params.site_id.clone()).increment(1);
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    let req = SimulatorRequest::TraceRays { params, reply: reply_tx };

    if state.sim_tx.send(req).await.is_err() {
        return Json(ApiResponse::error("仿真模块不可用"));
    }

    match reply_rx.await {
        Ok(paths) => {
            histogram!("simulation_duration_seconds", "type" => "acoustic").record(start.elapsed().as_secs_f64());
            gauge!("simulation_path_count", "type" => "acoustic").set(paths.len() as f64);
            for path in &paths {
                let _ = state.store.insert_sound_path(path).await;
            }
            Json(ApiResponse::ok(paths))
        }
        Err(_) => { counter!("simulation_failures_total", "type" => "acoustic").increment(1); Json(ApiResponse::error("仿真请求超时")) }
    }
}

async fn run_sti_analysis(
    State(state): State<Arc<AppState>>,
    Json(params): Json<StiAnalysisParams>,
) -> Json<ApiResponse<SpeechIntelligibility>> {
    let start = Instant::now();
    counter!("simulations_total", "type" => "sti").increment(1);
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    let req = AnalyzerRequest::AnalyzeSti { params, reply: reply_tx };

    if state.analyzer_tx.send(req).await.is_err() {
        return Json(ApiResponse::error("STI分析模块不可用"));
    }

    match reply_rx.await {
        Ok(result) => {
            histogram!("simulation_duration_seconds", "type" => "sti").record(start.elapsed().as_secs_f64());
            gauge!("latest_sti", "site" => result.site_id.clone()).set(result.sti_value);
            gauge!("latest_rasti", "site" => result.site_id.clone()).set(result.rasti_value);
            gauge!("latest_d50", "site" => result.site_id.clone()).set(result.definition_d50);
            gauge!("latest_c50", "site" => result.site_id.clone()).set(result.clarity_c50);
            let _ = state.store.insert_intelligibility(&result).await;
            Json(ApiResponse::ok(result))
        }
        Err(_) => { counter!("simulation_failures_total", "type" => "sti").increment(1); Json(ApiResponse::error("STI分析请求超时")) }
    }
}

async fn run_wave_field_simulation(
    State(state): State<Arc<AppState>>,
    Json(params): Json<SimulationParams>,
) -> Json<ApiResponse<SoundFieldSnapshot>> {
    let start = Instant::now();
    if !state.config.valid_site_ids.contains(&params.site_id) {
        return Json(ApiResponse::error("无效的场所ID"));
    }

    counter!("simulations_total", "type" => "wave_field", "site" => params.site_id.clone()).increment(1);
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    let req = SimulatorRequest::WaveField { params, reply: reply_tx };

    if state.sim_tx.send(req).await.is_err() {
        return Json(ApiResponse::error("仿真模块不可用"));
    }

    match reply_rx.await {
        Ok(field) => {
            histogram!("simulation_duration_seconds", "type" => "wave_field").record(start.elapsed().as_secs_f64());
            let _ = state.store.insert_sound_field(&field).await;
            Json(ApiResponse::ok(field))
        }
        Err(_) => { counter!("simulation_failures_total", "type" => "wave_field").increment(1); Json(ApiResponse::error("波动声场仿真请求超时")) }
    }
}

async fn get_stats(State(state): State<Arc<AppState>>) -> Json<ApiResponse<serde_json::Value>> {
    let (m, p, i, a, f) = state.store.stats();
    gauge!("store_measurements_count").set(m as f64);
    gauge!("store_paths_count").set(p as f64);
    gauge!("store_intelligibility_count").set(i as f64);
    gauge!("store_alerts_count").set(a as f64);
    gauge!("store_fields_count").set(f as f64);
    let stats = serde_json::json!({
        "measurements_count": m,
        "sound_paths_count": p,
        "intelligibility_count": i,
        "alerts_count": a,
        "sound_fields_count": f,
    });
    Json(ApiResponse::ok(stats))
}
