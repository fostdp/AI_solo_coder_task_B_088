use crate::models::{AlarmEvent, AcousticAlert, AlertThresholds};
use crate::storage::ClickHouseStore;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};

pub struct AlarmTask {
    mqtt_client: AsyncClient,
    base_topic: String,
    thresholds: AlertThresholds,
    rx: mpsc::Receiver<AlarmEvent>,
    store: Arc<ClickHouseStore>,
    alert_history: Arc<Mutex<Vec<AcousticAlert>>>,
}

impl AlarmTask {
    pub fn new(
        broker_host: &str,
        broker_port: u16,
        client_id: &str,
        base_topic: &str,
        thresholds: AlertThresholds,
        rx: mpsc::Receiver<AlarmEvent>,
        store: Arc<ClickHouseStore>,
    ) -> Self {
        let mut mqttoptions = MqttOptions::new(client_id, broker_host, broker_port);
        mqttoptions.set_keep_alive(Duration::from_secs(30));
        mqttoptions.set_clean_start(true);

        let (mqtt_client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

        let _client_clone = mqtt_client.clone();
        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(_) => {}
                    Err(e) => {
                        error!("MQTT eventloop error: {:?}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        });

        Self {
            mqtt_client,
            base_topic: base_topic.to_string(),
            thresholds,
            rx,
            store,
            alert_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn run(&mut self) {
        while let Some(event) = self.rx.recv().await {
            match event {
                AlarmEvent::CheckMeasurement { site_id, reverb_t60, spl } => {
                    if let Some(alert) = self.check_reverb(&site_id, reverb_t60) {
                        let _ = self.publish_alert(&alert).await;
                    }
                    if let Some(alert) = self.check_spl(&site_id, spl) {
                        let _ = self.publish_alert(&alert).await;
                    }
                }
                AlarmEvent::CheckIntelligibility { site_id, sti, d50, c50 } => {
                    if let Some(alert) = self.check_sti(&site_id, sti) {
                        let _ = self.publish_alert(&alert).await;
                    }
                    if let Some(alert) = self.check_definition(&site_id, d50, c50) {
                        let _ = self.publish_alert(&alert).await;
                    }
                }
            }
        }
        warn!("AlarmTask channel closed, exiting run loop");
    }

    fn check_sti(&self, site_id: &str, sti_value: f64) -> Option<AcousticAlert> {
        if sti_value < self.thresholds.sti_min {
            let severity = if sti_value < 0.30 { "critical" } else if sti_value < 0.45 { "warning" } else { "info" };
            Some(self.create_alert(
                site_id,
                "sti_degradation",
                severity,
                "sti_value",
                sti_value,
                self.thresholds.sti_min,
                &format!("语音清晰度STI={:.3}低于阈值{:.3}，语音可懂度{}",
                    sti_value, self.thresholds.sti_min,
                    if sti_value < 0.30 { "严重退化" } else if sti_value < 0.45 { "明显下降" } else { "轻度下降" }
                ),
            ))
        } else { None }
    }

    fn check_reverb(&self, site_id: &str, t60: f64) -> Option<AcousticAlert> {
        if t60 > self.thresholds.reverb_t60_max {
            Some(self.create_alert(
                site_id,
                "reverb_too_high",
                "warning",
                "reverb_time_t60",
                t60,
                self.thresholds.reverb_t60_max,
                &format!("混响时间T60={:.2}s超过上限{:.2}s，语音清晰度将受影响", t60, self.thresholds.reverb_t60_max),
            ))
        } else if t60 < self.thresholds.reverb_t60_min {
            Some(self.create_alert(
                site_id,
                "reverb_too_low",
                "info",
                "reverb_time_t60",
                t60,
                self.thresholds.reverb_t60_min,
                &format!("混响时间T60={:.2}s低于下限{:.2}s，声学特性异常", t60, self.thresholds.reverb_t60_min),
            ))
        } else { None }
    }

    fn check_spl(&self, site_id: &str, spl: f64) -> Option<AcousticAlert> {
        if spl > self.thresholds.spl_max_db {
            Some(self.create_alert(
                site_id,
                "spl_too_high",
                "warning",
                "sound_pressure_level",
                spl,
                self.thresholds.spl_max_db,
                &format!("声压级SPL={:.1}dB超过安全上限{:.1}dB，存在听力损伤风险", spl, self.thresholds.spl_max_db),
            ))
        } else if spl < self.thresholds.spl_min_db {
            Some(self.create_alert(
                site_id,
                "spl_too_low",
                "info",
                "sound_pressure_level",
                spl,
                self.thresholds.spl_min_db,
                &format!("声压级SPL={:.1}dB低于正常值{:.1}dB，传感器可能异常", spl, self.thresholds.spl_min_db),
            ))
        } else { None }
    }

    fn check_definition(&self, site_id: &str, d50: f64, c50: f64) -> Option<AcousticAlert> {
        if d50 < self.thresholds.definition_d50_min {
            Some(self.create_alert(
                site_id,
                "definition_low",
                "warning",
                "definition_d50",
                d50,
                self.thresholds.definition_d50_min,
                &format!("语言清晰度D50={:.1}%低于阈值{:.1}%", d50, self.thresholds.definition_d50_min),
            ))
        } else if c50 < self.thresholds.clarity_c50_min {
            Some(self.create_alert(
                site_id,
                "clarity_low",
                "info",
                "clarity_c50",
                c50,
                self.thresholds.clarity_c50_min,
                &format!("语言清晰度C50={:.1}dB低于阈值{:.1}dB", c50, self.thresholds.clarity_c50_min),
            ))
        } else { None }
    }

    fn create_alert(
        &self,
        site_id: &str,
        alert_type: &str,
        severity: &str,
        metric_name: &str,
        current_value: f64,
        threshold_value: f64,
        description: &str,
    ) -> AcousticAlert {
        let mqtt_topic = format!("{}/{}/{}", self.base_topic, site_id, alert_type);
        AcousticAlert {
            timestamp: chrono::Utc::now(),
            alert_id: uuid::Uuid::new_v4(),
            site_id: site_id.to_string(),
            alert_type: alert_type.to_string(),
            severity: severity.to_string(),
            metric_name: metric_name.to_string(),
            current_value,
            threshold_value,
            description: description.to_string(),
            mqtt_topic,
            acknowledged: 0,
        }
    }

    pub async fn publish_alert(&self, alert: &AcousticAlert) -> anyhow::Result<()> {
        let topic = format!("{}/{}/{}", self.base_topic, alert.site_id, alert.alert_type);
        let payload = json!({
            "alert_id": alert.alert_id.to_string(),
            "timestamp": alert.timestamp.to_rfc3339(),
            "site_id": alert.site_id,
            "alert_type": alert.alert_type,
            "severity": alert.severity,
            "metric_name": alert.metric_name,
            "current_value": alert.current_value,
            "threshold_value": alert.threshold_value,
            "description": alert.description,
            "acknowledged": alert.acknowledged,
        });

        let payload_str = serde_json::to_string(&payload)?;
        self.mqtt_client.publish(&topic, QoS::AtLeastOnce, false, payload_str).await?;

        info!("Published alert to MQTT topic [{}]: {}", topic, alert.description);

        let mut history = self.alert_history.lock().await;
        history.push(alert.clone());
        if history.len() > 1000 {
            history.drain(0..history.len() - 1000);
        }

        Ok(())
    }

    pub async fn get_recent_alerts(&self, limit: usize) -> Vec<AcousticAlert> {
        let history = self.alert_history.lock().await;
        history.iter().rev().take(limit).cloned().collect()
    }
}
