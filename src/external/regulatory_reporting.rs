//! 규제 기관 보고 시스템
//!
//! 이 모듈은 금융감독원, FATCA, CRS 등 규제 기관에
//! 필요한 거래 보고서를 자동으로 생성하고 전송합니다.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use log::{info, error, warn, debug};
use tokio::time::{sleep, interval};

/// 규제 기관 타입
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RegulatoryAgency {
    FSC,        // 금융감독원 (Financial Supervisory Service)
    FATCA,      // Foreign Account Tax Compliance Act
    CRS,        // Common Reporting Standard
    AML,        // Anti-Money Laundering
    KYC,        // Know Your Customer
    GDPR,       // General Data Protection Regulation
}

/// 보고서 타입
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReportType {
    TransactionReport,      // 거래 보고서
    SuspiciousActivity,     // 의심스러운 활동 보고서
    LargeTransaction,        // 대규모 거래 보고서
    CustomerReport,         // 고객 정보 보고서
    ComplianceReport,       // 규정 준수 보고서
    AuditReport,           // 감사 보고서
}

impl std::fmt::Display for ReportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReportType::TransactionReport => write!(f, "TransactionReport"),
            ReportType::SuspiciousActivity => write!(f, "SuspiciousActivity"),
            ReportType::LargeTransaction => write!(f, "LargeTransaction"),
            ReportType::CustomerReport => write!(f, "CustomerReport"),
            ReportType::ComplianceReport => write!(f, "ComplianceReport"),
            ReportType::AuditReport => write!(f, "AuditReport"),
        }
    }
}

/// 거래 데이터 구조
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionData {
    pub transaction_id: String,
    pub user_id: String,
    pub symbol: String,
    pub side: String, // "buy" or "sell"
    pub amount: f64,
    pub price: f64,
    pub total_value: f64,
    pub timestamp: u64,
    pub user_country: String,
    pub user_tax_id: Option<String>,
    pub ip_address: String,
    pub device_fingerprint: String,
}

/// 의심스러운 활동 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspiciousActivityData {
    pub activity_id: String,
    pub user_id: String,
    pub activity_type: String,
    pub risk_score: f64,
    pub description: String,
    pub evidence: Vec<String>,
    pub timestamp: u64,
    pub severity: String, // "low", "medium", "high", "critical"
}

/// 규제 보고서 구조
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegulatoryReport {
    pub report_id: String,
    pub agency: RegulatoryAgency,
    pub report_type: ReportType,
    pub period_start: u64,
    pub period_end: u64,
    pub generated_at: u64,
    pub data: serde_json::Value,
    pub status: ReportStatus,
    pub submission_attempts: u32,
    pub last_submission_attempt: Option<u64>,
    pub submission_error: Option<String>,
}

/// 보고서 상태
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReportStatus {
    Pending,        // 대기 중
    Generated,      // 생성 완료
    Submitted,      // 제출 완료
    Failed,         // 제출 실패
    Acknowledged,   // 접수 확인
}

/// 규제 보고 설정
#[derive(Debug, Clone)]
pub struct RegulatoryReportingConfig {
    pub agencies: Vec<RegulatoryAgency>,
    pub report_intervals: HashMap<ReportType, u64>, // 밀리초
    pub large_transaction_threshold: f64,
    pub suspicious_activity_threshold: f64,
    pub enable_automatic_submission: bool,
    pub retry_attempts: u32,
    pub retry_interval_ms: u64,
}

impl Default for RegulatoryReportingConfig {
    fn default() -> Self {
        let mut report_intervals = HashMap::new();
        report_intervals.insert(ReportType::TransactionReport, 86400000); // 24시간
        report_intervals.insert(ReportType::SuspiciousActivity, 3600000); // 1시간
        report_intervals.insert(ReportType::LargeTransaction, 0); // 즉시
        report_intervals.insert(ReportType::CustomerReport, 604800000); // 7일
        report_intervals.insert(ReportType::ComplianceReport, 2592000000); // 30일
        report_intervals.insert(ReportType::AuditReport, 31536000000); // 1년

        Self {
            agencies: vec![
                RegulatoryAgency::FSC,
                RegulatoryAgency::FATCA,
                RegulatoryAgency::CRS,
                RegulatoryAgency::AML,
            ],
            report_intervals,
            large_transaction_threshold: 10000000.0, // 1천만원
            suspicious_activity_threshold: 0.8, // 80% 위험도
            enable_automatic_submission: true,
            retry_attempts: 3,
            retry_interval_ms: 300000, // 5분
        }
    }
}

/// 규제 보고 관리자
pub struct RegulatoryReportingManager {
    /// 설정
    config: RegulatoryReportingConfig,
    /// 거래 데이터 저장소
    transactions: Arc<RwLock<Vec<TransactionData>>>,
    /// 의심스러운 활동 저장소
    suspicious_activities: Arc<RwLock<Vec<SuspiciousActivityData>>>,
    /// 생성된 보고서 저장소
    reports: Arc<RwLock<Vec<RegulatoryReport>>>,
    /// 보고 활성화 상태
    is_reporting: Arc<Mutex<bool>>,
}

impl RegulatoryReportingManager {
    /// 새 규제 보고 관리자 생성
    pub fn new(config: RegulatoryReportingConfig) -> Self {
        Self {
            config,
            transactions: Arc::new(RwLock::new(Vec::new())),
            suspicious_activities: Arc::new(RwLock::new(Vec::new())),
            reports: Arc::new(RwLock::new(Vec::new())),
            is_reporting: Arc::new(Mutex::new(false)),
        }
    }

    /// 규제 보고 시작
    pub async fn start_reporting(&self) {
        let mut is_reporting = self.is_reporting.lock().await;
        if *is_reporting {
            warn!("규제 보고가 이미 실행 중입니다");
            return;
        }
        *is_reporting = true;
        drop(is_reporting);

        info!("규제 기관 보고 시스템 시작");

        let transactions = self.transactions.clone();
        let suspicious_activities = self.suspicious_activities.clone();
        let reports = self.reports.clone();
        let config = self.config.clone();
        let is_reporting = self.is_reporting.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(60000)); // 1분마다 체크

            loop {
                interval.tick().await;

                // 보고 중단 확인
                {
                    let reporting = is_reporting.lock().await;
                    if !*reporting {
                        break;
                    }
                }

                // 각 보고서 타입별로 생성 및 제출
                for (report_type, interval_ms) in &config.report_intervals {
                    if *interval_ms == 0 {
                        // 즉시 보고 (대규모 거래 등)
                        continue;
                    }

                    if let Err(e) = Self::generate_and_submit_report(
                        report_type,
                        &transactions,
                        &suspicious_activities,
                        &reports,
                        &config,
                    ).await {
                        error!("{} 보고서 생성/제출 실패: {}", report_type, e);
                    }
                }
            }

            info!("규제 기관 보고 시스템 종료");
        });
    }

    /// 규제 보고 중단
    pub async fn stop_reporting(&self) {
        let mut is_reporting = self.is_reporting.lock().await;
        *is_reporting = false;
        info!("규제 기관 보고 시스템 중단 요청");
    }

    /// 거래 데이터 추가
    pub async fn add_transaction(&self, transaction: TransactionData) -> Result<(), String> {
        // 대규모 거래 확인
        if transaction.total_value >= self.config.large_transaction_threshold {
            self.generate_large_transaction_report(&transaction).await?;
        }

        // 의심스러운 활동 확인
        if let Some(suspicious_activity) = self.detect_suspicious_activity(&transaction).await {
            self.add_suspicious_activity(suspicious_activity).await?;
        }

        // 거래 데이터 저장
        {
            let mut transactions = self.transactions.write().await;
            transactions.push(transaction);
            
            // 최대 10000개 거래만 유지
            if transactions.len() > 10000 {
                transactions.drain(0..transactions.len() - 10000);
            }
        }

        Ok(())
    }

    /// 의심스러운 활동 추가
    pub async fn add_suspicious_activity(&self, activity: SuspiciousActivityData) -> Result<(), String> {
        // 즉시 보고서 생성
        if activity.risk_score >= self.config.suspicious_activity_threshold {
            self.generate_suspicious_activity_report(&activity).await?;
        }

        // 활동 데이터 저장
        {
            let mut activities = self.suspicious_activities.write().await;
            activities.push(activity);
            
            // 최대 1000개 활동만 유지
            if activities.len() > 1000 {
                activities.drain(0..activities.len() - 1000);
            }
        }

        Ok(())
    }

    /// 보고서 생성 및 제출
    async fn generate_and_submit_report(
        report_type: &ReportType,
        transactions: &Arc<RwLock<Vec<TransactionData>>>,
        suspicious_activities: &Arc<RwLock<Vec<SuspiciousActivityData>>>,
        reports: &Arc<RwLock<Vec<RegulatoryReport>>>,
        config: &RegulatoryReportingConfig,
    ) -> Result<(), String> {
        // 보고서 생성
        let report = Self::generate_report(
            report_type,
            transactions,
            suspicious_activities,
            config,
        ).await?;

        // 보고서 저장
        {
            let mut reports_guard = reports.write().await;
            reports_guard.push(report.clone());
            
            // 최대 1000개 보고서만 유지
            if reports_guard.len() > 1000 {
                let excess = reports_guard.len() - 1000;
                reports_guard.drain(0..excess);
            }
        }

        // 자동 제출 활성화된 경우 제출
        if config.enable_automatic_submission {
            Self::submit_report(&report, config).await?;
        }

        Ok(())
    }

    /// 보고서 생성
    async fn generate_report(
        report_type: &ReportType,
        transactions: &Arc<RwLock<Vec<TransactionData>>>,
        suspicious_activities: &Arc<RwLock<Vec<SuspiciousActivityData>>>,
        config: &RegulatoryReportingConfig,
    ) -> Result<RegulatoryReport, String> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let period_end = current_time;
        let period_start = period_end - 86400000; // 24시간 전

        let report_id = format!("{}_{}_{}", report_type, period_start, period_end);

        // 보고서 데이터 생성
        let data = match report_type {
            ReportType::TransactionReport => {
                let transactions_guard = transactions.read().await;
                let period_transactions: Vec<_> = transactions_guard
                    .iter()
                    .filter(|t| t.timestamp >= period_start && t.timestamp <= period_end)
                    .collect();
                
                serde_json::json!({
                    "total_transactions": period_transactions.len(),
                    "total_volume": period_transactions.iter().map(|t| t.total_value).sum::<f64>(),
                    "transactions": period_transactions
                })
            }
            ReportType::SuspiciousActivity => {
                let activities_guard = suspicious_activities.read().await;
                let period_activities: Vec<_> = activities_guard
                    .iter()
                    .filter(|a| a.timestamp >= period_start && a.timestamp <= period_end)
                    .collect();
                
                serde_json::json!({
                    "total_activities": period_activities.len(),
                    "high_risk_activities": period_activities.iter().filter(|a| a.risk_score >= 0.8).count(),
                    "activities": period_activities
                })
            }
            _ => serde_json::json!({
                "message": "Report data not implemented for this type"
            })
        };

        Ok(RegulatoryReport {
            report_id,
            agency: RegulatoryAgency::FSC, // 기본값
            report_type: report_type.clone(),
            period_start,
            period_end,
            generated_at: current_time,
            data,
            status: ReportStatus::Generated,
            submission_attempts: 0,
            last_submission_attempt: None,
            submission_error: None,
        })
    }

    /// 대규모 거래 보고서 생성
    async fn generate_large_transaction_report(&self, transaction: &TransactionData) -> Result<(), String> {
        let report = RegulatoryReport {
            report_id: format!("large_transaction_{}", transaction.transaction_id),
            agency: RegulatoryAgency::FSC,
            report_type: ReportType::LargeTransaction,
            period_start: transaction.timestamp,
            period_end: transaction.timestamp,
            generated_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            data: serde_json::to_value(transaction).unwrap(),
            status: ReportStatus::Generated,
            submission_attempts: 0,
            last_submission_attempt: None,
            submission_error: None,
        };

        {
            let mut reports = self.reports.write().await;
            reports.push(report.clone());
        }

        // 즉시 제출
        if self.config.enable_automatic_submission {
            Self::submit_report(&report, &self.config).await?;
        }

        info!("대규모 거래 보고서 생성: {}", transaction.transaction_id);
        Ok(())
    }

    /// 의심스러운 활동 보고서 생성
    async fn generate_suspicious_activity_report(&self, activity: &SuspiciousActivityData) -> Result<(), String> {
        let report = RegulatoryReport {
            report_id: format!("suspicious_activity_{}", activity.activity_id),
            agency: RegulatoryAgency::AML,
            report_type: ReportType::SuspiciousActivity,
            period_start: activity.timestamp,
            period_end: activity.timestamp,
            generated_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            data: serde_json::to_value(activity).unwrap(),
            status: ReportStatus::Generated,
            submission_attempts: 0,
            last_submission_attempt: None,
            submission_error: None,
        };

        {
            let mut reports = self.reports.write().await;
            reports.push(report.clone());
        }

        // 즉시 제출
        if self.config.enable_automatic_submission {
            Self::submit_report(&report, &self.config).await?;
        }

        info!("의심스러운 활동 보고서 생성: {}", activity.activity_id);
        Ok(())
    }

    /// 의심스러운 활동 탐지
    async fn detect_suspicious_activity(&self, transaction: &TransactionData) -> Option<SuspiciousActivityData> {
        let mut risk_score = 0.0;
        let mut evidence = Vec::new();

        // 대규모 거래 체크
        if transaction.total_value >= self.config.large_transaction_threshold {
            risk_score += 0.3;
            evidence.push("Large transaction amount".to_string());
        }

        // 비정상적인 거래 패턴 체크 (Mock)
        if transaction.amount > 1000.0 && transaction.price < 100.0 {
            risk_score += 0.2;
            evidence.push("Unusual trading pattern".to_string());
        }

        // 고위험 국가 체크 (Mock)
        let high_risk_countries = ["XX", "YY", "ZZ"];
        if high_risk_countries.contains(&transaction.user_country.as_str()) {
            risk_score += 0.4;
            evidence.push("High-risk country".to_string());
        }

        // 위험도가 임계값 이상인 경우
        if risk_score >= 0.5 {
            Some(SuspiciousActivityData {
                activity_id: uuid::Uuid::new_v4().to_string(),
                user_id: transaction.user_id.clone(),
                activity_type: "Suspicious Transaction".to_string(),
                risk_score,
                description: format!("Suspicious transaction detected: {}", transaction.transaction_id),
                evidence,
                timestamp: transaction.timestamp,
                severity: if risk_score >= 0.8 { "high" } else { "medium" }.to_string(),
            })
        } else {
            None
        }
    }

    /// 보고서 제출
    async fn submit_report(report: &RegulatoryReport, config: &RegulatoryReportingConfig) -> Result<(), String> {
        // Mock: 실제로는 규제 기관 API에 제출
        sleep(Duration::from_millis(100 + fastrand::u32(0..200) as u64)).await;

        // 90% 성공률로 시뮬레이션
        if fastrand::f32() < 0.9 {
            info!("보고서 제출 성공: {} ({})", report.report_id, report.report_type);
            Ok(())
        } else {
            let error_msg = "Submission failed due to network error".to_string();
            error!("보고서 제출 실패: {} - {}", report.report_id, error_msg);
            Err(error_msg)
        }
    }

    /// 보고서 조회
    pub async fn get_reports(&self, limit: Option<usize>) -> Vec<RegulatoryReport> {
        let reports = self.reports.read().await;
        if let Some(limit) = limit {
            reports.iter().rev().take(limit).cloned().collect()
        } else {
            reports.clone()
        }
    }

    /// 특정 상태의 보고서 조회
    pub async fn get_reports_by_status(&self, status: ReportStatus) -> Vec<RegulatoryReport> {
        let reports = self.reports.read().await;
        reports.iter().filter(|r| r.status == status).cloned().collect()
    }

    /// 보고 통계 조회
    pub async fn get_reporting_stats(&self) -> RegulatoryReportingStats {
        let transactions = self.transactions.read().await;
        let suspicious_activities = self.suspicious_activities.read().await;
        let reports = self.reports.read().await;

        let mut status_counts = HashMap::new();
        for report in reports.iter() {
            *status_counts.entry(report.status.clone()).or_insert(0) += 1;
        }

        let mut agency_counts = HashMap::new();
        for report in reports.iter() {
            *agency_counts.entry(report.agency.clone()).or_insert(0) += 1;
        }

        RegulatoryReportingStats {
            total_transactions: transactions.len(),
            total_suspicious_activities: suspicious_activities.len(),
            total_reports: reports.len(),
            status_counts,
            agency_counts,
            is_reporting: *self.is_reporting.lock().await,
        }
    }
}

/// 규제 보고 통계
#[derive(Debug, Clone)]
pub struct RegulatoryReportingStats {
    pub total_transactions: usize,
    pub total_suspicious_activities: usize,
    pub total_reports: usize,
    pub status_counts: HashMap<ReportStatus, usize>,
    pub agency_counts: HashMap<RegulatoryAgency, usize>,
    pub is_reporting: bool,
}

// fastrand 의존성을 위해 간단한 랜덤 함수 구현
mod fastrand {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn f32() -> f32 {
        let mut hasher = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        let hash = hasher.finish();
        (hash % 1000) as f32 / 1000.0
    }

    pub fn u32(range: std::ops::Range<u32>) -> u32 {
        let mut hasher = DefaultHasher::new();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        let hash = hasher.finish();
        range.start + ((hash % (range.end - range.start) as u64) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_regulatory_reporting_manager_creation() {
        let config = RegulatoryReportingConfig::default();
        let manager = RegulatoryReportingManager::new(config);
        
        let stats = manager.get_reporting_stats().await;
        assert_eq!(stats.total_transactions, 0);
        assert_eq!(stats.total_suspicious_activities, 0);
        assert_eq!(stats.total_reports, 0);
        assert!(!stats.is_reporting);
    }

    #[tokio::test]
    async fn test_transaction_addition() {
        let config = RegulatoryReportingConfig::default();
        let manager = RegulatoryReportingManager::new(config);
        
        let transaction = TransactionData {
            transaction_id: "test_tx_1".to_string(),
            user_id: "user_1".to_string(),
            symbol: "BTC-KRW".to_string(),
            side: "buy".to_string(),
            amount: 0.1,
            price: 50000000.0,
            total_value: 5000000.0,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
            user_country: "KR".to_string(),
            user_tax_id: Some("123-45-67890".to_string()),
            ip_address: "192.168.1.1".to_string(),
            device_fingerprint: "device_123".to_string(),
        };
        
        let result = manager.add_transaction(transaction).await;
        assert!(result.is_ok());
        
        let stats = manager.get_reporting_stats().await;
        assert_eq!(stats.total_transactions, 1);
    }

    #[tokio::test]
    async fn test_large_transaction_detection() {
        let config = RegulatoryReportingConfig {
            large_transaction_threshold: 1000000.0, // 100만원
            ..Default::default()
        };
        let manager = RegulatoryReportingManager::new(config);
        
        let transaction = TransactionData {
            transaction_id: "large_tx_1".to_string(),
            user_id: "user_1".to_string(),
            symbol: "BTC-KRW".to_string(),
            side: "buy".to_string(),
            amount: 0.1,
            price: 50000000.0,
            total_value: 5000000.0, // 500만원 (임계값 초과)
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
            user_country: "KR".to_string(),
            user_tax_id: Some("123-45-67890".to_string()),
            ip_address: "192.168.1.1".to_string(),
            device_fingerprint: "device_123".to_string(),
        };
        
        let result = manager.add_transaction(transaction).await;
        assert!(result.is_ok());
        
        let reports = manager.get_reports(None).await;
        assert!(!reports.is_empty());
        
        let large_transaction_reports: Vec<_> = reports.iter()
            .filter(|r| r.report_type == ReportType::LargeTransaction)
            .collect();
        assert!(!large_transaction_reports.is_empty());
    }

    #[tokio::test]
    async fn test_suspicious_activity_detection() {
        let config = RegulatoryReportingConfig::default();
        let manager = RegulatoryReportingManager::new(config);
        
        let transaction = TransactionData {
            transaction_id: "suspicious_tx_1".to_string(),
            user_id: "user_1".to_string(),
            symbol: "BTC-KRW".to_string(),
            side: "buy".to_string(),
            amount: 2000.0, // 비정상적으로 큰 수량
            price: 50.0,    // 비정상적으로 낮은 가격
            total_value: 100000.0,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
            user_country: "XX".to_string(), // 고위험 국가
            user_tax_id: Some("123-45-67890".to_string()),
            ip_address: "192.168.1.1".to_string(),
            device_fingerprint: "device_123".to_string(),
        };
        
        let result = manager.add_transaction(transaction).await;
        assert!(result.is_ok());
        
        let stats = manager.get_reporting_stats().await;
        assert!(stats.total_suspicious_activities > 0);
    }

    #[tokio::test]
    async fn test_report_generation() {
        let config = RegulatoryReportingConfig::default();
        let manager = RegulatoryReportingManager::new(config);
        
        // 테스트 거래 추가
        for i in 0..5 {
            let transaction = TransactionData {
                transaction_id: format!("test_tx_{}", i),
                user_id: format!("user_{}", i),
                symbol: "BTC-KRW".to_string(),
                side: "buy".to_string(),
                amount: 0.1,
                price: 50000000.0,
                total_value: 5000000.0,
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
                user_country: "KR".to_string(),
                user_tax_id: Some("123-45-67890".to_string()),
                ip_address: "192.168.1.1".to_string(),
                device_fingerprint: "device_123".to_string(),
            };
            
            manager.add_transaction(transaction).await.unwrap();
        }
        
        let reports = manager.get_reports(None).await;
        assert!(!reports.is_empty());
        
        let transaction_reports: Vec<_> = reports.iter()
            .filter(|r| r.report_type == ReportType::TransactionReport)
            .collect();
        assert!(!transaction_reports.is_empty());
    }
}