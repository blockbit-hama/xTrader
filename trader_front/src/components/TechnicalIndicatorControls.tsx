import React from 'react';
import styled from 'styled-components';

interface TechnicalIndicatorControlsProps {
  onToggleIndicator: (indicator: string, enabled: boolean) => void;
  enabledIndicators: Set<string>;
}

const ControlsContainer = styled.div`
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  padding: 12px;
  background: #f8f9fa;
  border: 1px solid #e9ecef;
  border-radius: 8px;
  margin-bottom: 16px;
`;

const IndicatorButton = styled.button<{ active: boolean }>`
  padding: 8px 12px;
  border: 1px solid ${props => props.active ? '#007bff' : '#dee2e6'};
  border-radius: 6px;
  background: ${props => props.active ? '#007bff' : '#ffffff'};
  color: ${props => props.active ? '#ffffff' : '#495057'};
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
  
  &:hover {
    background: ${props => props.active ? '#0056b3' : '#f8f9fa'};
    border-color: ${props => props.active ? '#0056b3' : '#adb5bd'};
  }
  
  &:active {
    transform: translateY(1px);
  }
`;

const SectionTitle = styled.div`
  font-size: 14px;
  font-weight: 600;
  color: #495057;
  margin-bottom: 8px;
  width: 100%;
`;

const SectionGroup = styled.div`
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 8px;
`;

const TechnicalIndicatorControls: React.FC<TechnicalIndicatorControlsProps> = ({
  onToggleIndicator,
  enabledIndicators
}) => {
  const indicators = {
    '이동평균': [
      { key: 'ma5', label: 'MA5', color: '#ff6b6b' },
      { key: 'ma10', label: 'MA10', color: '#4ecdc4' },
      { key: 'ma20', label: 'MA20', color: '#45b7d1' },
      { key: 'ma50', label: 'MA50', color: '#96ceb4' },
      { key: 'ma200', label: 'MA200', color: '#feca57' },
    ],
    '지수이동평균': [
      { key: 'ema12', label: 'EMA12', color: '#ff9ff3' },
      { key: 'ema26', label: 'EMA26', color: '#54a0ff' },
    ],
    '오실레이터': [
      { key: 'rsi', label: 'RSI', color: '#5f27cd' },
      { key: 'stoch', label: 'Stoch', color: '#00d2d3' },
    ],
    '추세': [
      { key: 'macd', label: 'MACD', color: '#ff6348' },
      { key: 'bb', label: 'Bollinger', color: '#2ed573' },
    ],
    '거래량': [
      { key: 'vol_ma', label: 'Vol MA', color: '#ffa502' },
    ]
  };

  return (
    <ControlsContainer>
      {Object.entries(indicators).map(([category, categoryIndicators]) => (
        <div key={category}>
          <SectionTitle>{category}</SectionTitle>
          <SectionGroup>
            {categoryIndicators.map((indicator) => (
              <IndicatorButton
                key={indicator.key}
                active={enabledIndicators.has(indicator.key)}
                onClick={() => onToggleIndicator(indicator.key, !enabledIndicators.has(indicator.key))}
                style={{ borderColor: enabledIndicators.has(indicator.key) ? indicator.color : undefined }}
              >
                {indicator.label}
              </IndicatorButton>
            ))}
          </SectionGroup>
        </div>
      ))}
    </ControlsContainer>
  );
};

export default TechnicalIndicatorControls;