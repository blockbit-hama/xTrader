//! 로그 분석 시스템
//!
//! 이 모듈은 구조화된 로그를 분석하고 검색하여
//! 시스템 문제를 조기 감지하고 분석합니다.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use log::{info, error, warn, debug};
use tokio::time::{sleep, interval};
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};

/// 로그 레벨
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

/// 로그 엔트리
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub timestamp: u64,
    pub level: LogLevel,
    pub message: String,
    pub module: String,
    pub thread_id: String,
    pub file: String,
    pub line: u32,
    pub metadata: HashMap<String, String>,
    pub tags: Vec<String>,
}

/// 로그 검색 쿼리
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogQuery {
    pub text: Option<String>,
    pub level: Option<LogLevel>,
    pub module: Option<String>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub tags: Option<Vec<String>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// 로그 검색 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSearchResult {
    pub entries: Vec<LogEntry>,
    pub total_count: usize,
    pub query_time_ms: u64,
    pub has_more: bool,
}

/// 로그 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogStats {
    pub total_entries: u64,
    pub entries_by_level: HashMap<String, u64>,
    pub entries_by_module: HashMap<String, u64>,
    pub entries_by_hour: HashMap<String, u64>,
    pub error_rate: f64,
    pub warning_rate: f64,
    pub average_message_length: f64,
    pub unique_modules: usize,
    pub last_updated: u64,
}

/// 로그 패턴
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogPattern {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub description: String,
    pub severity: LogLevel,
    pub is_active: bool,
    pub match_count: u64,
    pub last_matched: Option<u64>,
}

/// 로그 분석 설정
#[derive(Debug, Clone)]
pub struct LogAnalyzerConfig {
    pub max_entries: usize,
    pub retention_days: u32,
    pub enable_pattern_matching: bool,
    pub enable_real_time_analysis: bool,
    pub analysis_interval_ms: u64,
    pub max_search_results: usize,
    pub enable_indexing: bool,
    pub index_update_interval_ms: u64,
}

impl Default for LogAnalyzerConfig {
    fn default() -> Self {
        Self {
            max_entries: 100000, // 10만개
            retention_days: 7,
            analysis_interval_ms: 5000, // 5초
            enable_pattern_matching: true,
            enable_real_time_analysis: true,
            max_search_results: 1000,
            enable_indexing: true,
            index_update_interval_ms: 10000, // 10초
        }
    }
}

/// 로그 분석기
pub struct LogAnalyzer {
    config: LogAnalyzerConfig,
    log_entries: Arc<RwLock<VecDeque<LogEntry>>>,
    stats: Arc<RwLock<LogStats>>,
    patterns: Arc<RwLock<HashMap<String, LogPattern>>>,
    indexes: Arc<RwLock<LogIndexes>>,
    is_running: Arc<Mutex<bool>>,
}

/// 로그 인덱스
#[derive(Debug, Clone)]
pub struct LogIndexes {
    pub by_level: HashMap<String, Vec<String>>, // level -> entry_ids
    pub by_module: HashMap<String, Vec<String>>, // module -> entry_ids
    pub by_timestamp: VecDeque<String>, // entry_ids ordered by timestamp
    pub by_text: HashMap<String, Vec<String>>, // text -> entry_ids
    pub last_indexed: u64,
}

impl LogIndexes {
    fn new() -> Self {
        Self {
            by_level: HashMap::new(),
            by_module: HashMap::new(),
            by_timestamp: VecDeque::new(),
            by_text: HashMap::new(),
            last_indexed: 0,
        }
    }
}

impl LogAnalyzer {
    /// 새 로그 분석기 생성
    pub fn new(config: LogAnalyzerConfig) -> Self {
        Self {
            config,
            log_entries: Arc::new(RwLock::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(LogStats {
                total_entries: 0,
                entries_by_level: HashMap::new(),
                entries_by_module: HashMap::new(),
                entries_by_hour: HashMap::new(),
                error_rate: 0.0,
                warning_rate: 0.0,
                average_message_length: 0.0,
                unique_modules: 0,
                last_updated: 0,
            })),
            patterns: Arc::new(RwLock::new(HashMap::new())),
            indexes: Arc::new(RwLock::new(LogIndexes::new())),
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    /// 로그 분석기 시작
    pub async fn start(&self) {
        let mut is_running = self.is_running.lock().await;
        if *is_running {
            warn!("로그 분석기가 이미 실행 중입니다");
            return;
        }
        *is_running = true;
        drop(is_running);

        info!("로그 분석기 시작");

        // 기본 패턴 등록
        self.register_default_patterns().await;

        let log_entries = self.log_entries.clone();
        let stats = self.stats.clone();
        let patterns = self.patterns.clone();
        let indexes = self.indexes.clone();
        let config = self.config.clone();
        let is_running = self.is_running.clone();

        // 로그 분석 태스크
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.analysis_interval_ms));

            loop {
                interval.tick().await;

                // 실행 중단 확인
                {
                    let running = is_running.lock().await;
                    if !*running {
                        break;
                    }
                }

                // 통계 업데이트
                Self::update_stats(&log_entries, &stats).await;

                // 패턴 매칭
                if config.enable_pattern_matching {
                    Self::match_patterns(&log_entries, &patterns).await;
                }

                // 인덱스 업데이트
                if config.enable_indexing {
                    Self::update_indexes(&log_entries, &indexes).await;
                }

                // 오래된 로그 정리
                Self::cleanup_old_logs(&log_entries, &config).await;
            }

            info!("로그 분석기 종료");
        });
    }

    /// 로그 분석기 중단
    pub async fn stop(&self) {
        let mut is_running = self.is_running.lock().await;
        *is_running = false;
        info!("로그 분석기 중단 요청");
    }

    /// 로그 엔트리 추가
    pub async fn add_log_entry(&self, entry: LogEntry) {
        let mut entries = self.log_entries.write().await;
        
        // 최대 엔트리 수 제한
        if entries.len() >= self.config.max_entries {
            entries.pop_front();
        }
        
        entries.push_back(entry);
    }

    /// 로그 검색
    pub async fn search_logs(&self, query: LogQuery) -> LogSearchResult {
        let start_time = SystemTime::now();
        let entries = self.log_entries.read().await;
        let indexes = self.indexes.read().await;

        let mut results = Vec::new();
        let mut total_count = 0;

        // 시간 범위 필터링
        let start_time_filter = query.start_time.unwrap_or(0);
        let end_time_filter = query.end_time.unwrap_or(u64::MAX);

        for entry in entries.iter() {
            // 시간 범위 체크
            if entry.timestamp < start_time_filter || entry.timestamp > end_time_filter {
                continue;
            }

            // 레벨 필터링
            if let Some(ref level) = query.level {
                if entry.level != *level {
                    continue;
                }
            }

            // 모듈 필터링
            if let Some(ref module) = query.module {
                if entry.module != *module {
                    continue;
                }
            }

            // 텍스트 검색
            if let Some(ref text) = query.text {
                if !entry.message.contains(text) && 
                   !entry.module.contains(text) &&
                   !entry.file.contains(text) {
                    continue;
                }
            }

            // 태그 필터링
            if let Some(ref tags) = query.tags {
                let mut matches = false;
                for tag in tags {
                    if entry.tags.contains(tag) {
                        matches = true;
                        break;
                    }
                }
                if !matches {
                    continue;
                }
            }

            total_count += 1;

            // 오프셋과 리미트 적용
            let offset = query.offset.unwrap_or(0);
            let limit = query.limit.unwrap_or(self.config.max_search_results);

            if results.len() >= limit {
                break;
            }

            if total_count > offset {
                results.push(entry.clone());
            }
        }

        let query_time = start_time.elapsed().unwrap().as_millis() as u64;
        let has_more = total_count > results.len();

        LogSearchResult {
            entries: results,
            total_count,
            query_time_ms: query_time,
            has_more,
        }
    }

    /// 통계 업데이트
    async fn update_stats(
        log_entries: &Arc<RwLock<VecDeque<LogEntry>>>,
        stats: &Arc<RwLock<LogStats>>,
    ) {
        let entries = log_entries.read().await;
        let mut stats_guard = stats.write().await;

        stats_guard.total_entries = entries.len() as u64;
        stats_guard.entries_by_level.clear();
        stats_guard.entries_by_module.clear();
        stats_guard.entries_by_hour.clear();

        let mut total_message_length = 0;
        let mut error_count = 0;
        let mut warning_count = 0;
        let mut unique_modules = std::collections::HashSet::new();

        for entry in entries.iter() {
            // 레벨별 통계
            let level_key = Self::level_to_string(&entry.level);
            *stats_guard.entries_by_level.entry(level_key).or_insert(0) += 1;

            // 모듈별 통계
            *stats_guard.entries_by_module.entry(entry.module.clone()).or_insert(0) += 1;
            unique_modules.insert(entry.module.clone());

            // 시간별 통계 (시간 단위)
            let hour_key = Self::timestamp_to_hour(entry.timestamp);
            *stats_guard.entries_by_hour.entry(hour_key).or_insert(0) += 1;

            // 메시지 길이
            total_message_length += entry.message.len();

            // 에러/경고 카운트
            match entry.level {
                LogLevel::Error | LogLevel::Fatal => error_count += 1,
                LogLevel::Warn => warning_count += 1,
                _ => {}
            }
        }

        // 에러율 계산
        stats_guard.error_rate = if stats_guard.total_entries > 0 {
            error_count as f64 / stats_guard.total_entries as f64 * 100.0
        } else {
            0.0
        };

        // 경고율 계산
        stats_guard.warning_rate = if stats_guard.total_entries > 0 {
            warning_count as f64 / stats_guard.total_entries as f64 * 100.0
        } else {
            0.0
        };

        // 평균 메시지 길이
        stats_guard.average_message_length = if stats_guard.total_entries > 0 {
            total_message_length as f64 / stats_guard.total_entries as f64
        } else {
            0.0
        };

        stats_guard.unique_modules = unique_modules.len();
        stats_guard.last_updated = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
    }

    /// 패턴 매칭
    async fn match_patterns(
        log_entries: &Arc<RwLock<VecDeque<LogEntry>>>,
        patterns: &Arc<RwLock<HashMap<String, LogPattern>>>,
    ) {
        let entries = log_entries.read().await;
        let mut patterns_guard = patterns.write().await;

        for entry in entries.iter() {
            for pattern in patterns_guard.values_mut() {
                if !pattern.is_active {
                    continue;
                }

                if entry.message.contains(&pattern.pattern) {
                    pattern.match_count += 1;
                    pattern.last_matched = Some(entry.timestamp);

                    // 패턴 매칭 알림 (Mock)
                    debug!("패턴 매칭: {} - {}", pattern.name, entry.message);
                }
            }
        }
    }

    /// 인덱스 업데이트
    async fn update_indexes(
        log_entries: &Arc<RwLock<VecDeque<LogEntry>>>,
        indexes: &Arc<RwLock<LogIndexes>>,
    ) {
        let entries = log_entries.read().await;
        let mut indexes_guard = indexes.write().await;

        // 인덱스 초기화
        indexes_guard.by_level.clear();
        indexes_guard.by_module.clear();
        indexes_guard.by_timestamp.clear();
        indexes_guard.by_text.clear();

        for entry in entries.iter() {
            // 레벨별 인덱스
            let level_key = Self::level_to_string(&entry.level);
            indexes_guard.by_level.entry(level_key).or_insert_with(Vec::new).push(entry.id.clone());

            // 모듈별 인덱스
            indexes_guard.by_module.entry(entry.module.clone()).or_insert_with(Vec::new).push(entry.id.clone());

            // 시간순 인덱스
            indexes_guard.by_timestamp.push_back(entry.id.clone());

            // 텍스트 인덱스 (단어별)
            let words: Vec<&str> = entry.message.split_whitespace().collect();
            for word in words {
                if word.len() > 2 { // 2글자 이상만 인덱싱
                    indexes_guard.by_text.entry(word.to_lowercase()).or_insert_with(Vec::new).push(entry.id.clone());
                }
            }
        }

        indexes_guard.last_indexed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
    }

    /// 오래된 로그 정리
    async fn cleanup_old_logs(
        log_entries: &Arc<RwLock<VecDeque<LogEntry>>>,
        config: &LogAnalyzerConfig,
    ) {
        let cutoff_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() - (config.retention_days as u64 * 24 * 60 * 60);

        let mut entries = log_entries.write().await;
        entries.retain(|entry| entry.timestamp > cutoff_time);
    }

    /// 기본 패턴 등록
    async fn register_default_patterns(&self) {
        let mut patterns = self.patterns.write().await;

        let default_patterns = vec![
            LogPattern {
                id: "error_pattern".to_string(),
                name: "에러 패턴".to_string(),
                pattern: "error".to_string(),
                description: "에러 메시지 패턴".to_string(),
                severity: LogLevel::Error,
                is_active: true,
                match_count: 0,
                last_matched: None,
            },
            LogPattern {
                id: "timeout_pattern".to_string(),
                name: "타임아웃 패턴".to_string(),
                pattern: "timeout".to_string(),
                description: "타임아웃 관련 메시지".to_string(),
                severity: LogLevel::Warn,
                is_active: true,
                match_count: 0,
                last_matched: None,
            },
            LogPattern {
                id: "connection_pattern".to_string(),
                name: "연결 패턴".to_string(),
                pattern: "connection".to_string(),
                description: "연결 관련 메시지".to_string(),
                severity: LogLevel::Info,
                is_active: true,
                match_count: 0,
                last_matched: None,
            },
        ];

        for pattern in default_patterns {
            patterns.insert(pattern.id.clone(), pattern);
        }
    }

    /// 패턴 추가
    pub async fn add_pattern(&self, pattern: LogPattern) -> Result<(), String> {
        let mut patterns = self.patterns.write().await;
        
        if patterns.contains_key(&pattern.id) {
            return Err(format!("패턴이 이미 존재합니다: {}", pattern.id));
        }

        patterns.insert(pattern.id.clone(), pattern);
        info!("새 패턴 추가: {}", pattern.id);
        Ok(())
    }

    /// 패턴 제거
    pub async fn remove_pattern(&self, pattern_id: &str) -> Result<(), String> {
        let mut patterns = self.patterns.write().await;
        
        if patterns.remove(pattern_id).is_none() {
            return Err(format!("패턴을 찾을 수 없습니다: {}", pattern_id));
        }

        info!("패턴 제거: {}", pattern_id);
        Ok(())
    }

    /// 패턴 활성화/비활성화
    pub async fn toggle_pattern(&self, pattern_id: &str) -> Result<(), String> {
        let mut patterns = self.patterns.write().await;
        
        if let Some(pattern) = patterns.get_mut(pattern_id) {
            pattern.is_active = !pattern.is_active;
            info!("패턴 상태 변경: {} -> {}", pattern_id, pattern.is_active);
            Ok(())
        } else {
            Err(format!("패턴을 찾을 수 없습니다: {}", pattern_id))
        }
    }

    /// 통계 조회
    pub async fn get_stats(&self) -> LogStats {
        self.stats.read().await.clone()
    }

    /// 패턴 목록 조회
    pub async fn get_patterns(&self) -> Vec<LogPattern> {
        let patterns = self.patterns.read().await;
        patterns.values().cloned().collect()
    }

    /// 로그 엔트리 수 조회
    pub async fn get_entry_count(&self) -> usize {
        let entries = self.log_entries.read().await;
        entries.len()
    }

    /// 레벨을 문자열로 변환
    fn level_to_string(level: &LogLevel) -> String {
        match level {
            LogLevel::Trace => "Trace".to_string(),
            LogLevel::Debug => "Debug".to_string(),
            LogLevel::Info => "Info".to_string(),
            LogLevel::Warn => "Warn".to_string(),
            LogLevel::Error => "Error".to_string(),
            LogLevel::Fatal => "Fatal".to_string(),
        }
    }

    /// 타임스탬프를 시간 단위로 변환
    fn timestamp_to_hour(timestamp: u64) -> String {
        let datetime = SystemTime::UNIX_EPOCH + Duration::from_millis(timestamp);
        let datetime = chrono::DateTime::<chrono::Utc>::from(datetime);
        datetime.format("%Y-%m-%d %H:00").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_log_analyzer_creation() {
        let config = LogAnalyzerConfig::default();
        let analyzer = LogAnalyzer::new(config);
        
        let stats = analyzer.get_stats().await;
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.error_rate, 0.0);
    }

    #[tokio::test]
    async fn test_log_entry_addition() {
        let config = LogAnalyzerConfig::default();
        let analyzer = LogAnalyzer::new(config);
        
        let entry = LogEntry {
            id: "test_entry".to_string(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
            level: LogLevel::Info,
            message: "테스트 메시지".to_string(),
            module: "test_module".to_string(),
            thread_id: "thread_1".to_string(),
            file: "test.rs".to_string(),
            line: 10,
            metadata: HashMap::new(),
            tags: vec!["test".to_string()],
        };
        
        analyzer.add_log_entry(entry).await;
        
        let count = analyzer.get_entry_count().await;
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_log_search() {
        let config = LogAnalyzerConfig::default();
        let analyzer = LogAnalyzer::new(config);
        
        // 테스트 로그 엔트리 추가
        let entry1 = LogEntry {
            id: "entry1".to_string(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
            level: LogLevel::Info,
            message: "정보 메시지".to_string(),
            module: "module1".to_string(),
            thread_id: "thread_1".to_string(),
            file: "test.rs".to_string(),
            line: 10,
            metadata: HashMap::new(),
            tags: vec!["info".to_string()],
        };
        
        let entry2 = LogEntry {
            id: "entry2".to_string(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
            level: LogLevel::Error,
            message: "에러 메시지".to_string(),
            module: "module2".to_string(),
            thread_id: "thread_2".to_string(),
            file: "error.rs".to_string(),
            line: 20,
            metadata: HashMap::new(),
            tags: vec!["error".to_string()],
        };
        
        analyzer.add_log_entry(entry1).await;
        analyzer.add_log_entry(entry2).await;
        
        // 레벨별 검색
        let query = LogQuery {
            level: Some(LogLevel::Error),
            ..Default::default()
        };
        
        let result = analyzer.search_logs(query).await;
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].level, LogLevel::Error);
    }

    #[tokio::test]
    async fn test_pattern_operations() {
        let config = LogAnalyzerConfig::default();
        let analyzer = LogAnalyzer::new(config);
        
        analyzer.start().await;
        
        // 패턴 추가
        let pattern = LogPattern {
            id: "test_pattern".to_string(),
            name: "테스트 패턴".to_string(),
            pattern: "test".to_string(),
            description: "테스트용 패턴".to_string(),
            severity: LogLevel::Info,
            is_active: true,
            match_count: 0,
            last_matched: None,
        };
        
        let result = analyzer.add_pattern(pattern).await;
        assert!(result.is_ok());
        
        // 패턴 목록 조회
        let patterns = analyzer.get_patterns().await;
        assert!(patterns.len() > 0);
        
        analyzer.stop().await;
    }
}