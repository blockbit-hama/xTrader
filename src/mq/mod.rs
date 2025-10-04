//! Message Queue 통합 모듈
//!
//! 이 모듈은 Redis Streams, Apache Kafka, RabbitMQ를 통합하여
//! 고성능 메시지 처리를 제공합니다.

pub mod redis_streams;
pub mod redis_consumer;
pub mod kafka_producer;
pub mod kafka_consumer;
pub mod rabbitmq_producer;
pub mod rabbitmq_consumer;
pub mod backup_queue;
pub mod health_monitor;
pub mod recovery_manager;

pub use redis_streams::{RedisStreamsProducer, ExecutionMessage};
pub use redis_consumer::{RedisConsumerWorker, RedisConsumerManager, ConsumerConfig};
pub use kafka_producer::{KafkaProducer, MarketDataMessage, MarketStatisticsMessage, OrderBookUpdateMessage, ProducerStats};
pub use kafka_consumer::{KafkaConsumerWorker, KafkaConsumerConfig, MDPConsumer, ExternalExchangeConsumer, RegulatoryConsumer, AnalyticsConsumer};
pub use rabbitmq_producer::{RabbitMQProducer, WebSocketNotificationMessage, RabbitMQError, ProducerStats as RabbitMQProducerStats, RoutingPatterns};
pub use rabbitmq_consumer::{RabbitMQConsumerWorker, RabbitMQConsumerConfig, WebSocketServerConsumer, LoadBalancerConsumer, DeadLetterQueueConsumer, ServerStatus};
pub use backup_queue::{LocalBackupQueue, BackupMessage, MQType, BackupMessageBuilder, BackupQueueStats};
pub use health_monitor::{MQHealthMonitor, HealthStatus, MQHealthStatus, ConnectionStatus, HealthCheckConfig};
pub use recovery_manager::{RecoveryManager, RecoveryStats, RecoveryStatus, RecoveryConfig};