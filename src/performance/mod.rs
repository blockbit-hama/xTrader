//! 성능 최적화 모듈
//!
//! 이 모듈은 배치 처리, 병렬 소비, 캐시 최적화, 메트릭 수집을 통해
//! 고성능 시스템을 구축합니다.

pub mod batch_processor;
pub mod parallel_consumer;
pub mod cache_optimizer;
pub mod metrics_collector;

pub use batch_processor::*;
pub use parallel_consumer::*;
pub use cache_optimizer::*;
pub use metrics_collector::*;