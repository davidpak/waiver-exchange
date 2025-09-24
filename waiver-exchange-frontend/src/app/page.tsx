import { ErrorBoundary } from '@/components/common/ErrorBoundary';
import { TradingLayout } from '@/components/layout/TradingLayout';

export default function Home() {
  return (
    <ErrorBoundary>
      <TradingLayout />
    </ErrorBoundary>
  );
}