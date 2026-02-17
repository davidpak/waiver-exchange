'use client';

import { ErrorBoundary } from '@/components/common/ErrorBoundary';
import { TradingLayout } from '@/components/layout/TradingLayout';
import { useAutoAnimate } from '@/hooks/useAutoAnimate';

export default function Home() {
  const [animateRef] = useAutoAnimate();

  return (
    <ErrorBoundary>
      <div ref={animateRef} style={{ height: '100vh' }}>
        <TradingLayout />
      </div>
    </ErrorBoundary>
  );
}