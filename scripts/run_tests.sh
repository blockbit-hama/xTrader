#!/bin/bash

# 테스트 실행 스크립트
# 다양한 테스트를 실행하고 결과를 정리합니다.

echo "🧪 테스트 실행 스크립트 시작"
echo "================================"

# 색상 정의
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 테스트 결과 저장
TEST_RESULTS=""

# 함수: 테스트 실행 및 결과 저장
run_test() {
    local test_name="$1"
    local test_command="$2"
    
    echo -e "${BLUE}🔍 $test_name 실행 중...${NC}"
    
    if eval "$test_command"; then
        echo -e "${GREEN}✅ $test_name 성공${NC}"
        TEST_RESULTS="$TEST_RESULTS✅ $test_name\n"
    else
        echo -e "${RED}❌ $test_name 실패${NC}"
        TEST_RESULTS="$TEST_RESULTS❌ $test_name\n"
    fi
    
    echo ""
}

# 함수: 테스트 요약 출력
print_summary() {
    echo "================================"
    echo -e "${YELLOW}📊 테스트 요약${NC}"
    echo "================================"
    echo -e "$TEST_RESULTS"
}

# 독립적인 테스트 프로젝트로 이동
echo -e "${BLUE}🔧 테스트 프로젝트로 이동...${NC}"
if [ -d "test_project" ]; then
    cd test_project
    echo -e "${GREEN}✅ 테스트 프로젝트 디렉토리 발견${NC}"
else
    echo -e "${RED}❌ 테스트 프로젝트 디렉토리를 찾을 수 없습니다${NC}"
    exit 1
fi
echo ""

# 단위 테스트 실행
run_test "단위 테스트" "cargo test --test unit_tests"

# 모든 테스트 실행
echo -e "${BLUE}🔍 모든 테스트 실행 중...${NC}"
if cargo test; then
    echo -e "${GREEN}✅ 모든 테스트 성공${NC}"
    TEST_RESULTS="$TEST_RESULTS✅ 모든 테스트\n"
else
    echo -e "${RED}❌ 일부 테스트 실패${NC}"
    TEST_RESULTS="$TEST_RESULTS❌ 일부 테스트\n"
fi
echo ""

# 테스트 커버리지 체크 (선택사항)
if command -v cargo-tarpaulin &> /dev/null; then
    echo -e "${BLUE}📊 테스트 커버리지 분석 중...${NC}"
    cargo tarpaulin --out Html --output-dir coverage
    echo -e "${GREEN}✅ 커버리지 리포트 생성 완료${NC}"
    echo ""
fi

# 테스트 요약 출력
print_summary

# 성능 벤치마크 실행 (선택사항)
if [ "$1" = "--bench" ]; then
    echo -e "${BLUE}🏃 성능 벤치마크 실행 중...${NC}"
    cargo bench
    echo -e "${GREEN}✅ 벤치마크 완료${NC}"
fi

echo -e "${GREEN}🎉 테스트 실행 완료${NC}"