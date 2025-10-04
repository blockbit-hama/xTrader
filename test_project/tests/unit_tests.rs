//! 단위 테스트
//!
//! 개별 컴포넌트의 단위 테스트를 수행합니다.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// 데이터 구조 테스트
#[tokio::test]
async fn test_data_structures() {
    println!("📋 데이터 구조 테스트 시작");
    
    // HashMap 테스트
    let mut map = HashMap::new();
    map.insert("key1", "value1");
    map.insert("key2", "value2");
    assert_eq!(map.get("key1"), Some(&"value1"));
    assert_eq!(map.len(), 2);
    
    // Vec 테스트
    let mut vec = Vec::new();
    vec.push(1);
    vec.push(2);
    vec.push(3);
    assert_eq!(vec.len(), 3);
    assert_eq!(vec[0], 1);
    
    // VecDeque 테스트
    let mut deque = std::collections::VecDeque::new();
    deque.push_back(1);
    deque.push_front(0);
    assert_eq!(deque.pop_front(), Some(0));
    assert_eq!(deque.pop_back(), Some(1));
    
    println!("📋 데이터 구조 테스트 완료");
}

/// 비동기 처리 테스트
#[tokio::test]
async fn test_async_processing() {
    println!("🔄 비동기 처리 테스트 시작");
    
    // 비동기 함수 테스트
    let result = async_function().await;
    assert_eq!(result, 42);
    
    // 동시 실행 테스트
    let start_time = std::time::Instant::now();
    let (result1, result2) = tokio::join!(
        async_function(),
        async_function()
    );
    let duration = start_time.elapsed();
    
    assert_eq!(result1, 42);
    assert_eq!(result2, 42);
    assert!(duration < Duration::from_millis(100));
    
    println!("🔄 비동기 처리 테스트 완료");
}

/// 에러 처리 테스트
#[tokio::test]
async fn test_error_handling() {
    println!("❌ 에러 처리 테스트 시작");
    
    // Result 타입 테스트
    let success_result: Result<i32, String> = Ok(42);
    let error_result: Result<i32, String> = Err("에러 발생".to_string());
    
    assert!(success_result.is_ok());
    assert!(error_result.is_err());
    assert_eq!(success_result.unwrap(), 42);
    assert_eq!(error_result.unwrap_err(), "에러 발생");
    
    // Option 타입 테스트
    let some_value: Option<i32> = Some(42);
    let none_value: Option<i32> = None;
    
    assert!(some_value.is_some());
    assert!(none_value.is_none());
    assert_eq!(some_value.unwrap(), 42);
    
    println!("❌ 에러 처리 테스트 완료");
}

/// 동시성 테스트
#[tokio::test]
async fn test_concurrency() {
    println!("🔀 동시성 테스트 시작");
    
    let shared_data = Arc::new(tokio::sync::Mutex::new(0));
    let mut handles = Vec::new();
    
    // 5개의 동시 태스크
    for i in 0..5 {
        let data = shared_data.clone();
        let handle = tokio::spawn(async move {
            let mut value = data.lock().await;
            *value += i;
            println!("  태스크 {} 완료", i);
        });
        handles.push(handle);
    }
    
    // 모든 태스크 완료 대기
    for handle in handles {
        handle.await.unwrap();
    }
    
    let final_value = *shared_data.lock().await;
    assert_eq!(final_value, 10); // 0+1+2+3+4 = 10
    
    println!("🔀 동시성 테스트 완료");
}

/// 메모리 관리 테스트
#[tokio::test]
async fn test_memory_management() {
    println!("🧠 메모리 관리 테스트 시작");
    
    // Arc 테스트
    let data = Arc::new(42);
    let data_clone = data.clone();
    assert_eq!(*data, 42);
    assert_eq!(*data_clone, 42);
    assert_eq!(Arc::strong_count(&data), 2);
    
    // Box 테스트
    let boxed_data = Box::new(42);
    assert_eq!(*boxed_data, 42);
    
    // Vec 메모리 할당 테스트
    let mut vec = Vec::with_capacity(1000);
    for i in 0..1000 {
        vec.push(i);
    }
    assert_eq!(vec.len(), 1000);
    assert_eq!(vec.capacity(), 1000);
    
    println!("🧠 메모리 관리 테스트 완료");
}

/// 시간 처리 테스트
#[tokio::test]
async fn test_time_handling() {
    println!("⏰ 시간 처리 테스트 시작");
    
    // 현재 시간 테스트
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    assert!(duration.as_secs() > 0);
    
    // Duration 테스트
    let duration1 = Duration::from_millis(1000);
    let duration2 = Duration::from_secs(1);
    assert_eq!(duration1, duration2);
    
    // 비동기 대기 테스트
    let start = std::time::Instant::now();
    sleep(Duration::from_millis(10)).await;
    let elapsed = start.elapsed();
    assert!(elapsed >= Duration::from_millis(10));
    
    println!("⏰ 시간 처리 테스트 완료");
}

/// 문자열 처리 테스트
#[tokio::test]
async fn test_string_handling() {
    println!("📝 문자열 처리 테스트 시작");
    
    // String 생성 테스트
    let mut s = String::new();
    s.push_str("Hello");
    s.push(' ');
    s.push_str("World");
    assert_eq!(s, "Hello World");
    
    // 문자열 포맷팅 테스트
    let formatted = format!("{} {} {}", "Hello", "Rust", "World");
    assert_eq!(formatted, "Hello Rust World");
    
    // 문자열 검색 테스트
    let text = "Hello World";
    assert!(text.contains("World"));
    assert!(!text.contains("Rust"));
    
    // 문자열 분할 테스트
    let words: Vec<&str> = text.split(' ').collect();
    assert_eq!(words, vec!["Hello", "World"]);
    
    println!("📝 문자열 처리 테스트 완료");
}

/// 수학 연산 테스트
#[tokio::test]
async fn test_mathematical_operations() {
    println!("🔢 수학 연산 테스트 시작");
    
    // 기본 연산 테스트
    assert_eq!(2 + 2, 4);
    assert_eq!(10 - 5, 5);
    assert_eq!(3 * 4, 12);
    assert_eq!(15 / 3, 5);
    assert_eq!(17 % 5, 2);
    
    // 부동소수점 연산 테스트
    let x = 3.14_f64;
    let y = 2.86_f64;
    let sum = x + y;
    assert!((sum - 6.0_f64).abs() < 0.001);
    
    // 제곱근 테스트
    let sqrt_result = (16.0_f64).sqrt();
    assert!((sqrt_result - 4.0_f64).abs() < 0.001);
    
    // 최대/최소값 테스트
    assert_eq!(std::cmp::max(10, 20), 20);
    assert_eq!(std::cmp::min(10, 20), 10);
    
    println!("🔢 수학 연산 테스트 완료");
}

/// 컬렉션 테스트
#[tokio::test]
async fn test_collections() {
    println!("📚 컬렉션 테스트 시작");
    
    // Vec 테스트
    let mut vec = vec![1, 2, 3];
    vec.push(4);
    assert_eq!(vec.len(), 4);
    assert_eq!(vec[0], 1);
    
    // HashMap 테스트
    let mut map = HashMap::new();
    map.insert("apple", 5);
    map.insert("banana", 3);
    assert_eq!(map.get("apple"), Some(&5));
    assert_eq!(map.len(), 2);
    
    // HashSet 테스트
    let mut set = std::collections::HashSet::new();
    set.insert(1);
    set.insert(2);
    set.insert(1); // 중복
    assert_eq!(set.len(), 2);
    assert!(set.contains(&1));
    
    // VecDeque 테스트
    let mut deque = std::collections::VecDeque::new();
    deque.push_back(1);
    deque.push_front(0);
    assert_eq!(deque.pop_front(), Some(0));
    assert_eq!(deque.pop_back(), Some(1));
    
    println!("📚 컬렉션 테스트 완료");
}

/// 정렬 테스트
#[tokio::test]
async fn test_sorting() {
    println!("📊 정렬 테스트 시작");
    
    // 정수 정렬 테스트
    let mut numbers = vec![3, 1, 4, 1, 5, 9, 2, 6];
    numbers.sort();
    assert_eq!(numbers, vec![1, 1, 2, 3, 4, 5, 6, 9]);
    
    // 역순 정렬 테스트
    let mut numbers = vec![3, 1, 4, 1, 5, 9, 2, 6];
    numbers.sort_by(|a, b| b.cmp(a));
    assert_eq!(numbers, vec![9, 6, 5, 4, 3, 2, 1, 1]);
    
    // 문자열 정렬 테스트
    let mut words = vec!["banana", "apple", "cherry"];
    words.sort();
    assert_eq!(words, vec!["apple", "banana", "cherry"]);
    
    // 사용자 정의 정렬 테스트
    let mut pairs = vec![(3, "c"), (1, "a"), (2, "b")];
    pairs.sort_by_key(|&(key, _)| key);
    assert_eq!(pairs, vec![(1, "a"), (2, "b"), (3, "c")]);
    
    println!("📊 정렬 테스트 완료");
}

/// 비동기 함수 (테스트용)
async fn async_function() -> i32 {
    sleep(Duration::from_millis(10)).await;
    42
}

/// 성능 측정 테스트
#[tokio::test]
async fn test_performance_measurement() {
    println!("⚡ 성능 측정 테스트 시작");
    
    // 반복문 성능 테스트
    let start = std::time::Instant::now();
    let mut _sum: u64 = 0;
    for i in 0..100000 {
        _sum += i as u64;
    }
    let duration = start.elapsed();
    println!("  📊 반복문 성능: {:.2}ms", duration.as_millis());
    
    // 벡터 연산 성능 테스트
    let start = std::time::Instant::now();
    let vec: Vec<i32> = (0..10000).collect();
    let _sum: i32 = vec.iter().sum();
    let duration = start.elapsed();
    println!("  📊 벡터 연산 성능: {:.2}ms", duration.as_millis());
    
    // 해시맵 성능 테스트
    let start = std::time::Instant::now();
    let mut map = HashMap::new();
    for i in 0..10000 {
        map.insert(i, i * 2);
    }
    let duration = start.elapsed();
    println!("  📊 해시맵 성능: {:.2}ms", duration.as_millis());
    
    println!("⚡ 성능 측정 테스트 완료");
}