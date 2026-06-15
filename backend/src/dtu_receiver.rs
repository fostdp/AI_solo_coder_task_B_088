use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;
use tracing::{info, warn};

use crate::models::{AcousticMeasurement, DtuEvent, AlarmEvent, AcousticConfig};
use crate::storage::ClickHouseStore;

pub struct DtuReceiver {
    pub config: AcousticConfig,
    pub dtu_tx: tokio::sync::mpsc::Sender<DtuEvent>,
    pub alarm_tx: tokio::sync::mpsc::Sender<AlarmEvent>,
    pub store: Arc<ClickHouseStore>,
}

impl DtuReceiver {
    pub fn new(
        config: AcousticConfig,
        dtu_tx: tokio::sync::mpsc::Sender<DtuEvent>,
        alarm_tx: tokio::sync::mpsc::Sender<AlarmEvent>,
        store: Arc<ClickHouseStore>,
    ) -> Self {
        Self {
            config,
            dtu_tx,
            alarm_tx,
            store,
        }
    }

    pub async fn receive_measurement(
        &self,
        mut measurement: AcousticMeasurement,
    ) -> anyhow::Result<AcousticMeasurement> {
        if !self.config.valid_site_ids.contains(&measurement.site_id) {
            warn!("Invalid site_id: {}", measurement.site_id);
            return Err(anyhow::anyhow!("Invalid site_id: {}", measurement.site_id));
        }

        if !self.config.valid_sensor_ids.contains(&measurement.sensor_id) {
            warn!("Invalid sensor_id: {}", measurement.sensor_id);
            return Err(anyhow::anyhow!("Invalid sensor_id: {}", measurement.sensor_id));
        }

        if measurement.reverb_time_t60 <= 0.0 {
            warn!("Invalid reverb_time_t60: {}", measurement.reverb_time_t60);
            return Err(anyhow::anyhow!("reverb_time_t60 must be > 0"));
        }

        if !measurement.sound_pressure_level.is_finite() {
            warn!("Invalid sound_pressure_level: {}", measurement.sound_pressure_level);
            return Err(anyhow::anyhow!("sound_pressure_level must be finite"));
        }

        measurement.timestamp = Utc::now();
        measurement.measurement_id = Uuid::new_v4();

        self.dtu_tx.send(DtuEvent::Measurement(measurement.clone())).await?;
        self.alarm_tx.send(AlarmEvent::CheckMeasurement {
            site_id: measurement.site_id.clone(),
            reverb_t60: measurement.reverb_time_t60,
            spl: measurement.sound_pressure_level,
        }).await?;

        self.store.insert_measurement(&measurement).await?;

        info!(
            "Received measurement {} from sensor {} at site {}",
            measurement.measurement_id, measurement.sensor_id, measurement.site_id
        );

        Ok(measurement)
    }
}
