//! 모니터링 대시보드
//!
//! 이 모듈은 실시간 시스템 상태, 성능 메트릭, 알림을
//! 웹 대시보드로 제공합니다.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use log::{info, error, warn, debug};
use tokio::time::{sleep, interval};
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};

/// 대시보드 위젯 타입
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WidgetType {
    SystemHealth,      // 시스템 헬스
    ServiceStatus,     // 서비스 상태
    PerformanceChart,  // 성능 차트
    AlertList,         // 알림 목록
    MetricGauge,       // 메트릭 게이지
    LogViewer,         // 로그 뷰어
    Custom,            // 사용자 정의
}

/// 대시보드 위젯
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardWidget {
    pub id: String,
    pub title: String,
    pub widget_type: WidgetType,
    pub position: (u32, u32), // (x, y)
    pub size: (u32, u32),     // (width, height)
    pub config: HashMap<String, String>,
    pub data: serde_json::Value,
    pub last_updated: u64,
}

/// 대시보드 레이아웃
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardLayout {
    pub id: String,
    pub name: String,
    pub description: String,
    pub widgets: Vec<DashboardWidget>,
    pub created_at: u64,
    pub updated_at: u64,
    pub is_default: bool,
}

/// 대시보드 설정
#[derive(Debug, Clone)]
pub struct DashboardConfig {
    pub refresh_interval_ms: u64,
    pub max_widgets_per_dashboard: usize,
    pub enable_real_time_updates: bool,
    pub enable_websocket: bool,
    pub websocket_port: u16,
    pub enable_export: bool,
    pub export_formats: Vec<String>,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            refresh_interval_ms: 1000, // 1초
            max_widgets_per_dashboard: 20,
            enable_real_time_updates: true,
            enable_websocket: true,
            websocket_port: 8080,
            enable_export: true,
            export_formats: vec!["json".to_string(), "csv".to_string()],
        }
    }
}

/// 대시보드 서버
pub struct DashboardServer {
    config: DashboardConfig,
    layouts: Arc<RwLock<HashMap<String, DashboardLayout>>>,
    current_layout: Arc<RwLock<String>>,
    is_running: Arc<Mutex<bool>>,
    websocket_clients: Arc<RwLock<HashMap<String, WebSocketClient>>>,
}

/// WebSocket 클라이언트
#[derive(Debug, Clone)]
pub struct WebSocketClient {
    pub id: String,
    pub connected_at: u64,
    pub last_activity: u64,
    pub subscribed_widgets: Vec<String>,
}

/// 대시보드 데이터 제공자
pub struct DashboardDataProvider {
    system_health_data: Arc<RwLock<Option<serde_json::Value>>>,
    performance_data: Arc<RwLock<Vec<serde_json::Value>>>,
    alert_data: Arc<RwLock<Vec<serde_json::Value>>>,
    metric_data: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl DashboardDataProvider {
    pub fn new() -> Self {
        Self {
            system_health_data: Arc::new(RwLock::new(None)),
            performance_data: Arc::new(RwLock::new(Vec::new())),
            alert_data: Arc::new(RwLock::new(Vec::new())),
            metric_data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 시스템 헬스 데이터 업데이트
    pub async fn update_system_health(&self, health_data: serde_json::Value) {
        let mut data = self.system_health_data.write().await;
        *data = Some(health_data);
    }

    /// 성능 데이터 업데이트
    pub async fn update_performance_data(&self, perf_data: serde_json::Value) {
        let mut data = self.performance_data.write().await;
        data.push(perf_data);
        
        // 최대 1000개 데이터만 유지
        if data.len() > 1000 {
            data.drain(0..data.len() - 1000);
        }
    }

    /// 알림 데이터 업데이트
    pub async fn update_alert_data(&self, alert_data: serde_json::Value) {
        let mut data = self.alert_data.write().await;
        data.push(alert_data);
        
        // 최대 500개 알림만 유지
        if data.len() > 500 {
            data.drain(0..data.len() - 500);
        }
    }

    /// 메트릭 데이터 업데이트
    pub async fn update_metric_data(&self, metric_name: String, metric_data: serde_json::Value) {
        let mut data = self.metric_data.write().await;
        data.insert(metric_name, metric_data);
    }

    /// 위젯 데이터 조회
    pub async fn get_widget_data(&self, widget_type: &WidgetType, widget_id: &str) -> serde_json::Value {
        match widget_type {
            WidgetType::SystemHealth => {
                let data = self.system_health_data.read().await;
                data.clone().unwrap_or(serde_json::Value::Null)
            }
            WidgetType::PerformanceChart => {
                let data = self.performance_data.read().await;
                serde_json::to_value(data.clone()).unwrap_or(serde_json::Value::Null)
            }
            WidgetType::AlertList => {
                let data = self.alert_data.read().await;
                serde_json::to_value(data.clone()).unwrap_or(serde_json::Value::Null)
            }
            WidgetType::MetricGauge => {
                let data = self.metric_data.read().await;
                if let Some(metric_data) = data.get(widget_id) {
                    metric_data.clone()
                } else {
                    serde_json::Value::Null
                }
            }
            _ => serde_json::Value::Null,
        }
    }
}

impl DashboardServer {
    /// 새 대시보드 서버 생성
    pub fn new(config: DashboardConfig) -> Self {
        let mut server = Self {
            config,
            layouts: Arc::new(RwLock::new(HashMap::new())),
            current_layout: Arc::new(RwLock::new("default".to_string())),
            is_running: Arc::new(Mutex::new(false)),
            websocket_clients: Arc::new(RwLock::new(HashMap::new())),
        };

        // 기본 레이아웃 생성
        server.create_default_layout();
        server
    }

    /// 기본 레이아웃 생성
    fn create_default_layout(&mut self) {
        let default_layout = DashboardLayout {
            id: "default".to_string(),
            name: "기본 대시보드".to_string(),
            description: "시스템 모니터링 기본 대시보드".to_string(),
            widgets: vec![
                DashboardWidget {
                    id: "system_health".to_string(),
                    title: "시스템 헬스".to_string(),
                    widget_type: WidgetType::SystemHealth,
                    position: (0, 0),
                    size: (4, 2),
                    config: HashMap::new(),
                    data: serde_json::Value::Null,
                    last_updated: 0,
                },
                DashboardWidget {
                    id: "service_status".to_string(),
                    title: "서비스 상태".to_string(),
                    widget_type: WidgetType::ServiceStatus,
                    position: (4, 0),
                    size: (4, 2),
                    config: HashMap::new(),
                    data: serde_json::Value::Null,
                    last_updated: 0,
                },
                DashboardWidget {
                    id: "performance_chart".to_string(),
                    title: "성능 차트".to_string(),
                    widget_type: WidgetType::PerformanceChart,
                    position: (0, 2),
                    size: (8, 3),
                    config: HashMap::new(),
                    data: serde_json::Value::Null,
                    last_updated: 0,
                },
                DashboardWidget {
                    id: "alert_list".to_string(),
                    title: "알림 목록".to_string(),
                    widget_type: WidgetType::AlertList,
                    position: (8, 0),
                    size: (4, 5),
                    config: HashMap::new(),
                    data: serde_json::Value::Null,
                    last_updated: 0,
                },
            ],
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            updated_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            is_default: true,
        };

        let mut layouts = self.layouts.try_write().unwrap();
        layouts.insert("default".to_string(), default_layout);
    }

    /// 대시보드 서버 시작
    pub async fn start(&self, data_provider: Arc<DashboardDataProvider>) {
        let mut is_running = self.is_running.lock().await;
        if *is_running {
            warn!("대시보드 서버가 이미 실행 중입니다");
            return;
        }
        *is_running = true;
        drop(is_running);

        info!("대시보드 서버 시작: 포트={}", self.config.websocket_port);

        let layouts = self.layouts.clone();
        let current_layout = self.current_layout.clone();
        let websocket_clients = self.websocket_clients.clone();
        let config = self.config.clone();
        let is_running = self.is_running.clone();

        // 대시보드 업데이트 태스크
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.refresh_interval_ms));

            loop {
                interval.tick().await;

                // 실행 중단 확인
                {
                    let running = is_running.lock().await;
                    if !*running {
                        break;
                    }
                }

                // 위젯 데이터 업데이트
                Self::update_widget_data(&layouts, &current_layout, &data_provider).await;

                // WebSocket 클라이언트에게 업데이트 전송
                Self::broadcast_updates(&websocket_clients, &layouts, &current_layout).await;
            }

            info!("대시보드 서버 종료");
        });
    }

    /// 대시보드 서버 중단
    pub async fn stop(&self) {
        let mut is_running = self.is_running.lock().await;
        *is_running = false;
        info!("대시보드 서버 중단 요청");
    }

    /// 위젯 데이터 업데이트
    async fn update_widget_data(
        layouts: &Arc<RwLock<HashMap<String, DashboardLayout>>>,
        current_layout: &Arc<RwLock<String>>,
        data_provider: &Arc<DashboardDataProvider>,
    ) {
        let layouts_guard = layouts.read().await;
        let current_layout_name = current_layout.read().await;
        
        if let Some(layout) = layouts_guard.get(&*current_layout_name) {
            let mut updated_layout = layout.clone();
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            for widget in &mut updated_layout.widgets {
                widget.data = data_provider.get_widget_data(&widget.widget_type, &widget.id).await;
                widget.last_updated = current_time;
            }

            // 레이아웃 업데이트
            drop(layouts_guard);
            let mut layouts_guard = layouts.write().await;
            layouts_guard.insert(current_layout_name.clone(), updated_layout);
        }
    }

    /// WebSocket 클라이언트에게 업데이트 브로드캐스트
    async fn broadcast_updates(
        websocket_clients: &Arc<RwLock<HashMap<String, WebSocketClient>>>,
        layouts: &Arc<RwLock<HashMap<String, DashboardLayout>>>,
        current_layout: &Arc<RwLock<String>>,
    ) {
        let clients = websocket_clients.read().await;
        let layouts_guard = layouts.read().await;
        let current_layout_name = current_layout.read().await;

        if let Some(layout) = layouts_guard.get(&*current_layout_name) {
            let update_data = serde_json::to_value(layout).unwrap_or(serde_json::Value::Null);

            for client in clients.values() {
                // Mock: 실제로는 WebSocket을 통한 데이터 전송
                debug!("WebSocket 업데이트 전송: {} -> {}", client.id, update_data);
            }
        }
    }

    /// 레이아웃 조회
    pub async fn get_layout(&self, layout_id: &str) -> Option<DashboardLayout> {
        let layouts = self.layouts.read().await;
        layouts.get(layout_id).cloned()
    }

    /// 현재 레이아웃 조회
    pub async fn get_current_layout(&self) -> Option<DashboardLayout> {
        let current_layout_name = self.current_layout.read().await;
        self.get_layout(&current_layout_name).await
    }

    /// 레이아웃 변경
    pub async fn switch_layout(&self, layout_id: &str) -> Result<(), String> {
        let layouts = self.layouts.read().await;
        if !layouts.contains_key(layout_id) {
            return Err(format!("레이아웃을 찾을 수 없습니다: {}", layout_id));
        }

        let mut current_layout = self.current_layout.write().await;
        *current_layout = layout_id.to_string();

        info!("대시보드 레이아웃 변경: {}", layout_id);
        Ok(())
    }

    /// 새 레이아웃 생성
    pub async fn create_layout(&self, layout: DashboardLayout) -> Result<(), String> {
        let mut layouts = self.layouts.write().await;
        
        if layouts.contains_key(&layout.id) {
            return Err(format!("레이아웃이 이미 존재합니다: {}", layout.id));
        }

        if layouts.len() >= 10 {
            return Err("최대 레이아웃 수를 초과했습니다".to_string());
        }

        layouts.insert(layout.id.clone(), layout);
        info!("새 레이아웃 생성: {}", layout.id);
        Ok(())
    }

    /// 레이아웃 삭제
    pub async fn delete_layout(&self, layout_id: &str) -> Result<(), String> {
        if layout_id == "default" {
            return Err("기본 레이아웃은 삭제할 수 없습니다".to_string());
        }

        let mut layouts = self.layouts.write().await;
        let mut current_layout = self.current_layout.write().await;

        if layouts.remove(layout_id).is_none() {
            return Err(format!("레이아웃을 찾을 수 없습니다: {}", layout_id));
        }

        // 현재 레이아웃이 삭제된 경우 기본 레이아웃으로 변경
        if *current_layout == layout_id {
            *current_layout = "default".to_string();
        }

        info!("레이아웃 삭제: {}", layout_id);
        Ok(())
    }

    /// 위젯 추가
    pub async fn add_widget(&self, layout_id: &str, widget: DashboardWidget) -> Result<(), String> {
        let mut layouts = self.layouts.write().await;
        
        if let Some(layout) = layouts.get_mut(layout_id) {
            if layout.widgets.len() >= self.config.max_widgets_per_dashboard {
                return Err("최대 위젯 수를 초과했습니다".to_string());
            }

            // 위젯 ID 중복 체크
            if layout.widgets.iter().any(|w| w.id == widget.id) {
                return Err(format!("위젯 ID가 중복됩니다: {}", widget.id));
            }

            layout.widgets.push(widget);
            layout.updated_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            info!("위젯 추가: {} -> {}", widget.id, layout_id);
            Ok(())
        } else {
            Err(format!("레이아웃을 찾을 수 없습니다: {}", layout_id))
        }
    }

    /// 위젯 제거
    pub async fn remove_widget(&self, layout_id: &str, widget_id: &str) -> Result<(), String> {
        let mut layouts = self.layouts.write().await;
        
        if let Some(layout) = layouts.get_mut(layout_id) {
            let initial_len = layout.widgets.len();
            layout.widgets.retain(|w| w.id != widget_id);
            
            if layout.widgets.len() == initial_len {
                return Err(format!("위젯을 찾을 수 없습니다: {}", widget_id));
            }

            layout.updated_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            info!("위젯 제거: {} -> {}", widget_id, layout_id);
            Ok(())
        } else {
            Err(format!("레이아웃을 찾을 수 없습니다: {}", layout_id))
        }
    }

    /// WebSocket 클라이언트 연결
    pub async fn connect_websocket_client(&self, client_id: String) -> Result<(), String> {
        let mut clients = self.websocket_clients.write().await;
        
        if clients.contains_key(&client_id) {
            return Err(format!("클라이언트가 이미 연결되어 있습니다: {}", client_id));
        }

        let client = WebSocketClient {
            id: client_id.clone(),
            connected_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            last_activity: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            subscribed_widgets: Vec::new(),
        };

        clients.insert(client_id.clone(), client);
        info!("WebSocket 클라이언트 연결: {}", client_id);
        Ok(())
    }

    /// WebSocket 클라이언트 연결 해제
    pub async fn disconnect_websocket_client(&self, client_id: &str) -> Result<(), String> {
        let mut clients = self.websocket_clients.write().await;
        
        if clients.remove(client_id).is_none() {
            return Err(format!("클라이언트를 찾을 수 없습니다: {}", client_id));
        }

        info!("WebSocket 클라이언트 연결 해제: {}", client_id);
        Ok(())
    }

    /// 연결된 클라이언트 수 조회
    pub async fn get_connected_clients_count(&self) -> usize {
        let clients = self.websocket_clients.read().await;
        clients.len()
    }

    /// 레이아웃 목록 조회
    pub async fn get_layouts(&self) -> Vec<DashboardLayout> {
        let layouts = self.layouts.read().await;
        layouts.values().cloned().collect()
    }

    /// 대시보드 통계 조회
    pub async fn get_dashboard_stats(&self) -> DashboardStats {
        let layouts = self.layouts.read().await;
        let clients = self.websocket_clients.read().await;
        
        let total_widgets: usize = layouts.values().map(|l| l.widgets.len()).sum();
        let total_layouts = layouts.len();
        let connected_clients = clients.len();

        DashboardStats {
            total_layouts,
            total_widgets,
            connected_clients,
            active_layout: self.current_layout.read().await.clone(),
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }
}

/// 대시보드 통계
#[derive(Debug, Clone)]
pub struct DashboardStats {
    pub total_layouts: usize,
    pub total_widgets: usize,
    pub connected_clients: usize,
    pub active_layout: String,
    pub last_updated: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dashboard_server_creation() {
        let config = DashboardConfig::default();
        let server = DashboardServer::new(config);
        
        let stats = server.get_dashboard_stats().await;
        assert_eq!(stats.total_layouts, 1); // 기본 레이아웃
        assert_eq!(stats.total_widgets, 4); // 기본 위젯들
    }

    #[tokio::test]
    async fn test_layout_operations() {
        let config = DashboardConfig::default();
        let server = DashboardServer::new(config);
        
        // 새 레이아웃 생성
        let new_layout = DashboardLayout {
            id: "test_layout".to_string(),
            name: "테스트 레이아웃".to_string(),
            description: "테스트용 레이아웃".to_string(),
            widgets: Vec::new(),
            created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
            updated_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
            is_default: false,
        };
        
        let result = server.create_layout(new_layout).await;
        assert!(result.is_ok());
        
        // 레이아웃 조회
        let layout = server.get_layout("test_layout").await;
        assert!(layout.is_some());
        
        // 레이아웃 변경
        let result = server.switch_layout("test_layout").await;
        assert!(result.is_ok());
        
        // 레이아웃 삭제
        let result = server.delete_layout("test_layout").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_widget_operations() {
        let config = DashboardConfig::default();
        let server = DashboardServer::new(config);
        
        let widget = DashboardWidget {
            id: "test_widget".to_string(),
            title: "테스트 위젯".to_string(),
            widget_type: WidgetType::Custom,
            position: (0, 0),
            size: (2, 2),
            config: HashMap::new(),
            data: serde_json::Value::Null,
            last_updated: 0,
        };
        
        // 위젯 추가
        let result = server.add_widget("default", widget).await;
        assert!(result.is_ok());
        
        // 위젯 제거
        let result = server.remove_widget("default", "test_widget").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_websocket_client_operations() {
        let config = DashboardConfig::default();
        let server = DashboardServer::new(config);
        
        // 클라이언트 연결
        let result = server.connect_websocket_client("client1".to_string()).await;
        assert!(result.is_ok());
        
        // 연결된 클라이언트 수 확인
        let count = server.get_connected_clients_count().await;
        assert_eq!(count, 1);
        
        // 클라이언트 연결 해제
        let result = server.disconnect_websocket_client("client1").await;
        assert!(result.is_ok());
        
        // 연결된 클라이언트 수 확인
        let count = server.get_connected_clients_count().await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_data_provider() {
        let provider = DashboardDataProvider::new();
        
        // 시스템 헬스 데이터 업데이트
        let health_data = serde_json::json!({
            "status": "healthy",
            "services": 10,
            "uptime": 3600
        });
        provider.update_system_health(health_data.clone()).await;
        
        // 위젯 데이터 조회
        let widget_data = provider.get_widget_data(&WidgetType::SystemHealth, "system_health").await;
        assert_eq!(widget_data, health_data);
    }
}