//! MDP REST API 엔드포인트
//!
//! 이 모듈은 봉차트와 시장 통계 데이터를 제공하는
//! REST API 엔드포인트를 구현합니다.

use axum::{
    extract::{Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use log::{info, error, debug};
use crate::mdp::consumer::{MDPConsumer, CandlestickData, MarketStatistics, MDPConsumerStats};

/// API 응답 구조
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: u64,
}

/// 봉차트 조회 요청
#[derive(Debug, Deserialize)]
pub struct CandlestickRequest {
    pub symbol: String,
    pub timeframe: String,
    pub limit: Option<u32>,
}

/// 시장 통계 조회 요청
#[derive(Debug, Deserialize)]
pub struct MarketStatisticsRequest {
    pub symbol: Option<String>,
}

/// 봉차트 응답
#[derive(Debug, Serialize, Deserialize)]
pub struct CandlestickResponse {
    pub symbol: String,
    pub timeframe: String,
    pub candlestick: CandlestickData,
}

/// 시장 통계 응답
#[derive(Debug, Serialize, Deserialize)]
pub struct MarketStatisticsResponse {
    pub statistics: HashMap<String, MarketStatistics>,
}

/// MDP API 서버
pub struct MDPApiServer {
    consumer: Arc<MDPConsumer>,
    router: Router,
}

impl MDPApiServer {
    /// 새 MDP API 서버 생성
    pub fn new(consumer: Arc<MDPConsumer>) -> Self {
        let router = Router::new()
            .route("/health", get(Self::health_check))
            .route("/stats", get(Self::get_consumer_stats))
            .route("/candlestick/:symbol/:timeframe", get(Self::get_candlestick))
            .route("/candlesticks", post(Self::get_candlesticks))
            .route("/statistics", get(Self::get_market_statistics))
            .route("/statistics/:symbol", get(Self::get_symbol_statistics));

        Self { consumer, router }
    }

    /// 라우터 반환
    pub fn router(self) -> Router {
        self.router
    }

    /// 헬스체크 엔드포인트
    async fn health_check() -> Json<ApiResponse<String>> {
        let response = ApiResponse {
            success: true,
            data: Some("MDP API 서버가 정상적으로 실행 중입니다.".to_string()),
            error: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        
        Json(response)
    }

    /// Consumer 통계 조회
    async fn get_consumer_stats(
        consumer: Arc<MDPConsumer>,
    ) -> Result<Json<ApiResponse<MDPConsumerStats>>, StatusCode> {
        let stats = consumer.get_consumer_stats().await;
        
        let response = ApiResponse {
            success: true,
            data: Some(stats),
            error: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        
        Ok(Json(response))
    }

    /// 특정 심볼의 특정 타임프레임 봉차트 조회
    async fn get_candlestick(
        Path((symbol, timeframe)): Path<(String, String)>,
        consumer: Arc<MDPConsumer>,
    ) -> Result<Json<ApiResponse<CandlestickResponse>>, StatusCode> {
        debug!("봉차트 조회 요청: {} {}", symbol, timeframe);
        
        match consumer.get_candlestick_data(&symbol, &timeframe).await {
            Some(candlestick) => {
                let response = CandlestickResponse {
                    symbol: symbol.clone(),
                    timeframe: timeframe.clone(),
                    candlestick,
                };
                
                let api_response = ApiResponse {
                    success: true,
                    data: Some(response),
                    error: None,
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                };
                
                Ok(Json(api_response))
            }
            None => {
                let response: ApiResponse<CandlestickResponse> = ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("봉차트 데이터를 찾을 수 없습니다: {} {}", symbol, timeframe)),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                };
                
                Err(StatusCode::NOT_FOUND)
            }
        }
    }

    /// 여러 봉차트 조회
    async fn get_candlesticks(
        Query(request): Query<CandlestickRequest>,
        consumer: Arc<MDPConsumer>,
    ) -> Result<Json<ApiResponse<Vec<CandlestickResponse>>>, StatusCode> {
        debug!("봉차트 조회 요청: {:?}", request);
        
        let mut responses = Vec::new();
        
        // 특정 심볼과 타임프레임 조회
        if let Some(candlestick) = consumer.get_candlestick_data(&request.symbol, &request.timeframe).await {
            responses.push(CandlestickResponse {
                symbol: request.symbol.clone(),
                timeframe: request.timeframe.clone(),
                candlestick,
            });
        }
        
        let api_response = ApiResponse {
            success: true,
            data: Some(responses),
            error: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        
        Ok(Json(api_response))
    }

    /// 모든 시장 통계 조회
    async fn get_market_statistics(
        consumer: Arc<MDPConsumer>,
    ) -> Result<Json<ApiResponse<MarketStatisticsResponse>>, StatusCode> {
        debug!("시장 통계 조회 요청");
        
        let statistics = consumer.get_all_market_statistics().await;
        
        let response = MarketStatisticsResponse { statistics };
        
        let api_response = ApiResponse {
            success: true,
            data: Some(response),
            error: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        
        Ok(Json(api_response))
    }

    /// 특정 심볼의 시장 통계 조회
    async fn get_symbol_statistics(
        Path(symbol): Path<String>,
        consumer: Arc<MDPConsumer>,
    ) -> Result<Json<ApiResponse<MarketStatistics>>, StatusCode> {
        debug!("심볼 통계 조회 요청: {}", symbol);
        
        match consumer.get_market_statistics(&symbol).await {
            Some(statistics) => {
                let api_response = ApiResponse {
                    success: true,
                    data: Some(statistics),
                    error: None,
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                };
                
                Ok(Json(api_response))
            }
            None => {
                let response: ApiResponse<MarketStatistics> = ApiResponse {
                    success: false,
                    data: None,
                    error: Some(format!("시장 통계를 찾을 수 없습니다: {}", symbol)),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                };
                
                Err(StatusCode::NOT_FOUND)
            }
        }
    }
}

/// MDP API 서버 빌더
pub struct MDPApiServerBuilder {
    consumer: Option<Arc<MDPConsumer>>,
    port: u16,
    host: String,
}

impl MDPApiServerBuilder {
    /// 새 빌더 생성
    pub fn new() -> Self {
        Self {
            consumer: None,
            port: 3001,
            host: "0.0.0.0".to_string(),
        }
    }

    /// Consumer 설정
    pub fn consumer(mut self, consumer: Arc<MDPConsumer>) -> Self {
        self.consumer = Some(consumer);
        self
    }

    /// 포트 설정
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// 호스트 설정
    pub fn host(mut self, host: String) -> Self {
        self.host = host;
        self
    }

    /// API 서버 빌드
    pub fn build(self) -> Result<MDPApiServer, String> {
        let consumer = self.consumer.ok_or("Consumer가 설정되지 않았습니다")?;
        
        Ok(MDPApiServer::new(consumer))
    }

    /// API 서버 실행
    pub async fn run(self) -> Result<(), String> {
        let host = self.host.clone();
        let port = self.port;
        let server = self.build()?;
        let router = server.router();
        
        let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port))
            .await
            .map_err(|e| format!("서버 바인딩 실패: {}", e))?;
        
        info!("MDP API 서버 시작: {}:{}", host, port);
        
        axum::serve(listener, router)
            .await
            .map_err(|e| format!("서버 실행 실패: {}", e))?;
        
        Ok(())
    }
}

impl Default for MDPApiServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// MDP API 클라이언트 (Mock)
pub struct MDPApiClient {
    base_url: String,
}

impl MDPApiClient {
    /// 새 API 클라이언트 생성
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }

    /// 헬스체크 (Mock)
    pub async fn health_check(&self) -> Result<ApiResponse<String>, String> {
        let response = ApiResponse {
            success: true,
            data: Some("MDP API 서버가 정상적으로 실행 중입니다.".to_string()),
            error: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        
        Ok(response)
    }

    /// 봉차트 조회 (Mock)
    pub async fn get_candlestick(&self, symbol: &str, timeframe: &str) -> Result<ApiResponse<CandlestickResponse>, String> {
        let candlestick = CandlestickData {
            symbol: symbol.to_string(),
            timeframe: timeframe.to_string(),
            open: 50000000,
            high: 51000000,
            low: 49000000,
            close: 50500000,
            volume: 1000,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            trade_count: 10,
        };
        
        let response = CandlestickResponse {
            symbol: symbol.to_string(),
            timeframe: timeframe.to_string(),
            candlestick,
        };
        
        let api_response = ApiResponse {
            success: true,
            data: Some(response),
            error: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        
        Ok(api_response)
    }

    /// 시장 통계 조회 (Mock)
    pub async fn get_market_statistics(&self) -> Result<ApiResponse<MarketStatisticsResponse>, String> {
        let mut statistics = HashMap::new();
        statistics.insert("BTC-KRW".to_string(), MarketStatistics {
            symbol: "BTC-KRW".to_string(),
            price_change_24h: 5.5,
            volume_24h: 1000000,
            high_24h: 52000000,
            low_24h: 48000000,
            last_price: 50500000,
            bid_price: 50400000,
            ask_price: 50600000,
            spread: 200000,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        });
        
        let response = MarketStatisticsResponse { statistics };
        
        let api_response = ApiResponse {
            success: true,
            data: Some(response),
            error: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        
        Ok(api_response)
    }

    /// 특정 심볼 통계 조회 (Mock)
    pub async fn get_symbol_statistics(&self, symbol: &str) -> Result<ApiResponse<MarketStatistics>, String> {
        let statistics = MarketStatistics {
            symbol: symbol.to_string(),
            price_change_24h: 5.5,
            volume_24h: 1000000,
            high_24h: 52000000,
            low_24h: 48000000,
            last_price: 50500000,
            bid_price: 50400000,
            ask_price: 50600000,
            spread: 200000,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        
        let api_response = ApiResponse {
            success: true,
            data: Some(statistics),
            error: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        
        Ok(api_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mdp::consumer::MDPConsumerConfig;

    #[tokio::test]
    async fn test_api_response_creation() {
        let response = ApiResponse {
            success: true,
            data: Some("test".to_string()),
            error: None,
            timestamp: 1234567890,
        };
        
        assert!(response.success);
        assert_eq!(response.data, Some("test".to_string()));
        assert!(response.error.is_none());
        assert_eq!(response.timestamp, 1234567890);
    }

    #[tokio::test]
    async fn test_candlestick_request_deserialization() {
        let json = r#"{"symbol": "BTC-KRW", "timeframe": "1m", "limit": 100}"#;
        let request: CandlestickRequest = serde_json::from_str(json).unwrap();
        
        assert_eq!(request.symbol, "BTC-KRW");
        assert_eq!(request.timeframe, "1m");
        assert_eq!(request.limit, Some(100));
    }

    #[tokio::test]
    async fn test_mdp_api_server_builder() {
        let config = MDPConsumerConfig::default();
        let consumer = Arc::new(MDPConsumer::new(config));
        
        let builder = MDPApiServerBuilder::new()
            .consumer(consumer)
            .port(3001)
            .host("127.0.0.1".to_string());
        
        let server = builder.build();
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_mdp_api_client_creation() {
        let client = MDPApiClient::new("http://localhost:3001".to_string());
        assert_eq!(client.base_url, "http://localhost:3001");
    }
}