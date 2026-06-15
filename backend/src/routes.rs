use crate::acoustics::{AcousticSimulator, SiteAcousticParams};
use crate::alerts::AlertManager;
use crate::models::*;
use crate::sti::StiCalculator;
use crate::storage::ClickHouseStore;
use axum::{
    extract::{Path, State, Query},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::info;

pub struct AppState {
    pub store: Arc<ClickHouseStore>,
    pub alerts: Arc<AlertManager>,
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

async fn health_check() -> Json<ApiResponse<String>> {
    Json(ApiResponse::ok("天坛声学仿真系统运行正常".to_string()))
}

async fn get_sites(State(state): State<Arc<AppState>>) -> Json<ApiResponse<Vec<SiteInfo>>> {
    match state.store.get_sites().await {
        Ok(sites) => Json(ApiResponse::ok(sites)),
        Err(e) => Json(ApiResponse::error(&e.to_string())),
    }
}

async fn get_sensors(State(state): State<Arc<AppState>>) -> Json<ApiResponse<Vec<SensorMetadata>>> {
    match state.store.get_sensors().await {
        Ok(sensors) => Json(ApiResponse::ok(sensors)),
        Err(e) => Json(ApiResponse::error(&e.to_string())),
    }
}

async fn get_measurements(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Json<ApiResponse<Vec<AcousticMeasurement>>> {
    let site_id = params.site_id.as_deref().unwrap_or("huiyinbi");
    let limit = params.limit.unwrap_or(100);
    match state.store.get_recent_measurements(site_id, limit).await {
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
    Json(mut m): Json<AcousticMeasurement>,
) -> Json<ApiResponse<String>> {
    m.timestamp = chrono::Utc::now();
    m.measurement_id = uuid::Uuid::new_v4();

    if let Some(alert) = state.alerts.check_reverb(&m.site_id, m.reverb_time_t60) {
        let _ = state.store.insert_alert(&alert).await;
        let _ = state.alerts.publish_alert(&alert).await;
    }
    if let Some(alert) = state.alerts.check_spl(&m.site_id, m.sound_pressure_level) {
        let _ = state.store.insert_alert(&alert).await;
        let _ = state.alerts.publish_alert(&alert).await;
    }

    match state.store.insert_measurement(&m).await {
        Ok(_) => Json(ApiResponse::ok(format!("测量数据已保存: {}", m.measurement_id))),
        Err(e) => Json(ApiResponse::error(&e.to_string())),
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
    Json(mut s): Json<SpeechIntelligibility>,
) -> Json<ApiResponse<String>> {
    s.timestamp = chrono::Utc::now();
    s.analysis_id = uuid::Uuid::new_v4();

    if let Some(alert) = state.alerts.check_sti(&s.site_id, s.sti_value) {
        let _ = state.store.insert_alert(&alert).await;
        let _ = state.alerts.publish_alert(&alert).await;
    }
    if let Some(alert) = state.alerts.check_definition(&s.site_id, s.definition_d50, s.clarity_c50) {
        let _ = state.store.insert_alert(&alert).await;
        let _ = state.alerts.publish_alert(&alert).await;
    }

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
    let _ = state.alerts.publish_alert(&a).await;
    match state.store.insert_alert(&a).await {
        Ok(_) => Json(ApiResponse::ok(format!("告警已保存并推送: {}", a.alert_id))),
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
    info!("Running acoustic simulation for site: {}", params.site_id);

    let site_params = match SiteAcousticParams::from_id(&params.site_id) {
        Some(p) => p,
        None => return Json(ApiResponse::error("无效的场所ID")),
    };

    let simulator = AcousticSimulator::new(site_params);
    let paths = simulator.trace_ray_geometric(&params);

    for path in &paths {
        let _ = state.store.insert_sound_path(path).await;
    }

    Json(ApiResponse::ok(paths))
}

async fn run_sti_analysis(
    State(state): State<Arc<AppState>>,
    Json(params): Json<StiAnalysisParams>,
) -> Json<ApiResponse<SpeechIntelligibility>> {
    info!("Running STI analysis for site: {}", params.site_id);

    let result = StiCalculator::analyze(&params);

    if let Some(alert) = state.alerts.check_sti(&result.site_id, result.sti_value) {
        let _ = state.store.insert_alert(&alert).await;
        let _ = state.alerts.publish_alert(&alert).await;
    }
    if let Some(alert) = state.alerts.check_definition(&result.site_id, result.definition_d50, result.clarity_c50) {
        let _ = state.store.insert_alert(&alert).await;
        let _ = state.alerts.publish_alert(&alert).await;
    }

    let _ = state.store.insert_intelligibility(&result).await;
    Json(ApiResponse::ok(result))
}

async fn run_wave_field_simulation(
    State(state): State<Arc<AppState>>,
    Json(params): Json<SimulationParams>,
) -> Json<ApiResponse<SoundFieldSnapshot>> {
    info!("Running wave field simulation for site: {}", params.site_id);

    let site_params = match SiteAcousticParams::from_id(&params.site_id) {
        Some(p) => p,
        None => return Json(ApiResponse::error("无效的场所ID")),
    };

    let simulator = AcousticSimulator::new(site_params);
    let field = simulator.compute_wave_sound_field(&params, 64);
    let _ = state.store.insert_sound_field(&field).await;

    Json(ApiResponse::ok(field))
}

async fn get_stats(State(state): State<Arc<AppState>>) -> Json<ApiResponse<serde_json::Value>> {
    let (m, p, i, a, f) = state.store.stats();
    let stats = serde_json::json!({
        "measurements_count": m,
        "sound_paths_count": p,
        "intelligibility_count": i,
        "alerts_count": a,
        "sound_fields_count": f,
        "clickhouse_url": "http://localhost:8123",
        "mqtt_topic": "tiantan/acoustics",
    });
    Json(ApiResponse::ok(stats))
}
