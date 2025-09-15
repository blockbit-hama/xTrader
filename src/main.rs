mod api;
mod data;
mod matching_engine;
mod mdp;
mod sequencer;
mod server;
mod util;

use server::{start_server, ServerConfig};
use data::DataLoader;
use serde_json;



#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 로깅 초기화
    env_logger::init();
    
    println!("xTrader 거래소 시스템 시작");
    
    // 실전적인 가짜 데이터셋 로드
    match DataLoader::load_dataset("data/fake_dataset.json") {
        Ok(dataset) => {
            println!("✅ 데이터셋 로드 성공: {} 심볼 지원", dataset.market_data.len());

            // 초기 주문서 데이터 로드 및 주문 생성
            if let Ok(json_content) = std::fs::read_to_string("data/fake_dataset.json") {
                if let Ok(json_data) = serde_json::from_str::<serde_json::Value>(&json_content) {
                    let initial_orders = DataLoader::create_initial_orders(&json_data);
                    println!("📋 초기 주문서 생성: {} 개 주문", initial_orders.len());

                    // TODO: 초기 주문을 매칭 엔진에 추가하는 로직
                    // for order in initial_orders {
                    //     // 매칭 엔진에 주문 추가 (서버 시작 후)
                    // }
                }
            }

            // 가짜 사용자 정보 로드
            let _fake_users = DataLoader::load_fake_users("data/fake_dataset.json");
        },
        Err(e) => {
            println!("⚠️ 데이터셋 로드 실패: {}, 기본값으로 진행", e);
        }
    }
    
    // 서버 설정
    let config = ServerConfig::default();
    
    // 서버 시작
    start_server(config).await?;
    
    Ok(())
}

