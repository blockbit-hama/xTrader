#!/bin/bash

# xTrader 통합 실행 스크립트
# 백엔드 (7000), 프론트엔드 (7001), 시뮬레이터를 한 번에 실행합니다.

set -e  # 오류 발생 시 스크립트 중단

# 색상 정의
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# 로그 함수
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_header() {
    echo -e "${CYAN}========================================${NC}"
    echo -e "${CYAN} $1${NC}"
    echo -e "${CYAN}========================================${NC}"
}

# 프로세스 정리 함수
cleanup() {
    log_warn "정리 중..."

    # 백그라운드 작업 종료
    jobs -p | xargs -r kill 2>/dev/null || true

    # 포트 점유 프로세스 종료
    lsof -ti:7000 | xargs -r kill -9 2>/dev/null || true
    lsof -ti:7001 | xargs -r kill -9 2>/dev/null || true

    log_info "정리 완료"
    exit 0
}

# Ctrl+C 핸들링
trap cleanup INT TERM

# 메인 실행
main() {
    log_header "🚀 xTrader 거래소 시스템 시작"

    # 의존성 확인
    log_info "의존성 확인 중..."

    if ! command -v cargo &> /dev/null; then
        log_error "Rust/Cargo가 설치되지 않았습니다."
        exit 1
    fi

    if ! command -v node &> /dev/null; then
        log_error "Node.js가 설치되지 않았습니다."
        exit 1
    fi

    if ! command -v npm &> /dev/null; then
        log_error "npm이 설치되지 않았습니다."
        exit 1
    fi

    log_success "모든 의존성 확인됨"

    # 기존 프로세스 정리
    log_info "기존 프로세스 정리 중..."
    lsof -ti:7000 | xargs -r kill -9 2>/dev/null || true
    lsof -ti:7001 | xargs -r kill -9 2>/dev/null || true
    sleep 2

    # 백엔드 빌드
    log_header "🔧 백엔드 빌드"
    log_info "Rust 백엔드 컴파일 중..."
    cargo build --release
    if [ $? -eq 0 ]; then
        log_success "백엔드 빌드 완료"
    else
        log_error "백엔드 빌드 실패"
        exit 1
    fi

    # 시뮬레이터 빌드
    log_header "🤖 시뮬레이터 빌드"
    log_info "시뮬레이터 컴파일 중..."
    cd simulator
    cargo build --release
    if [ $? -eq 0 ]; then
        log_success "시뮬레이터 빌드 완료"
    else
        log_error "시뮬레이터 빌드 실패"
        exit 1
    fi
    cd ..

    # 프론트엔드 의존성 설치
    log_header "📦 프론트엔드 의존성 설치"
    cd trader_front
    if [ ! -d "node_modules" ] || [ ! -f "package-lock.json" ]; then
        log_info "프론트엔드 의존성 설치 중..."
        npm install
        if [ $? -eq 0 ]; then
            log_success "프론트엔드 의존성 설치 완료"
        else
            log_error "프론트엔드 의존성 설치 실패"
            exit 1
        fi
    else
        log_success "프론트엔드 의존성 이미 설치됨"
    fi
    cd ..

    # 가짜 데이터 확인
    log_header "📊 데이터 초기화"
    if [ -f "data/fake_dataset.json" ]; then
        log_success "가짜 데이터셋 발견: $(wc -l < data/fake_dataset.json) 줄"
    else
        log_warn "가짜 데이터셋이 없습니다. 서버에서 기본값을 생성합니다."
    fi

    # 서비스 시작
    log_header "🚀 서비스 시작"

    # 1. 백엔드 서버 시작 (포트 7000)
    log_info "백엔드 서버 시작 중... (포트 7000)"
    ./target/release/xTrader > logs/backend.log 2>&1 &
    BACKEND_PID=$!
    sleep 3

    if kill -0 $BACKEND_PID 2>/dev/null; then
        log_success "✅ 백엔드 서버 실행 중 (PID: $BACKEND_PID)"
        log_info "📊 REST API: http://localhost:7000"
        log_info "🔌 WebSocket: ws://localhost:7001"
    else
        log_error "백엔드 서버 시작 실패"
        cat logs/backend.log
        exit 1
    fi

    # 2. 프론트엔드 서버 시작 (포트 7001)
    log_info "프론트엔드 서버 시작 중... (포트 7001)"
    cd trader_front
    npm start > ../logs/frontend.log 2>&1 &
    FRONTEND_PID=$!
    cd ..
    sleep 5

    if kill -0 $FRONTEND_PID 2>/dev/null; then
        log_success "✅ 프론트엔드 서버 실행 중 (PID: $FRONTEND_PID)"
        log_info "🌐 웹 인터페이스: http://localhost:7001"
    else
        log_error "프론트엔드 서버 시작 실패"
        cat logs/frontend.log
        exit 1
    fi

    # 3. 시뮬레이터 시작 (3초 후)
    log_info "시뮬레이터 시작 대기 중... (3초 후 시작)"
    sleep 3

    log_info "시뮬레이터 시작 중..."
    cd simulator
    ./target/release/xtrader-simulator > ../logs/simulator.log 2>&1 &
    SIMULATOR_PID=$!
    cd ..
    sleep 2

    if kill -0 $SIMULATOR_PID 2>/dev/null; then
        log_success "✅ 시뮬레이터 실행 중 (PID: $SIMULATOR_PID)"
    else
        log_error "시뮬레이터 시작 실패"
        cat logs/simulator.log
        exit 1
    fi

    # 시작 완료
    log_header "🎉 xTrader 거래소 시스템 실행 완료!"
    echo ""
    log_success "📊 백엔드 서버:  http://localhost:7000  (PID: $BACKEND_PID)"
    log_success "🌐 프론트엔드:   http://localhost:7001  (PID: $FRONTEND_PID)"
    log_success "🤖 시뮬레이터:   활성화됨              (PID: $SIMULATOR_PID)"
    echo ""
    log_info "💡 웹 브라우저에서 http://localhost:7001 을 열어보세요!"
    log_info "📜 로그 파일: logs/ 폴더에서 확인 가능"
    echo ""
    log_warn "🛑 종료하려면 Ctrl+C를 누르세요"
    echo ""

    # 로그 모니터링
    log_info "실시간 로그 모니터링 시작..."
    echo ""

    # 무한 대기 (로그 출력)
    tail -f logs/backend.log logs/frontend.log logs/simulator.log &
    TAIL_PID=$!

    # 프로세스 모니터링
    while true; do
        # 백엔드 상태 확인
        if ! kill -0 $BACKEND_PID 2>/dev/null; then
            log_error "백엔드 서버 종료됨!"
            break
        fi

        # 프론트엔드 상태 확인
        if ! kill -0 $FRONTEND_PID 2>/dev/null; then
            log_error "프론트엔드 서버 종료됨!"
            break
        fi

        # 시뮬레이터 상태 확인
        if ! kill -0 $SIMULATOR_PID 2>/dev/null; then
            log_warn "시뮬레이터 종료됨 (재시작 시도)"
            cd simulator
            ./target/release/xtrader-simulator > ../logs/simulator.log 2>&1 &
            SIMULATOR_PID=$!
            cd ..
        fi

        sleep 10
    done
}

# 로그 디렉토리 생성
mkdir -p logs

# 메인 함수 실행
main "$@"