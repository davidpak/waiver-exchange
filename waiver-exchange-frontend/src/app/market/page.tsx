'use client';

import { ErrorBoundary } from '@/components/common/ErrorBoundary';
import { Header } from '@/components/layout/Header';
import { MarketPage } from '@/components/trading/MarketPage';
import { useNavigation } from '@/contexts/NavigationContext';
import { useAutoAnimate } from '@/hooks/useAutoAnimate';
import { useCustomTheme } from '@/hooks/useCustomTheme';
import { AppShell, Tabs } from '@mantine/core';
import { useState } from 'react';

/**
 * Dedicated Market Overview page
 * Shows all available players and their current market prices
 */
export default function MarketOverviewPage() {
  const { navigate } = useNavigation();
  const { toggleTheme } = useCustomTheme();
  const [animateRef] = useAutoAnimate();
  const [activeTab, setActiveTab] = useState<string>('overview');

  const handleNavigate = (route: string) => {
    navigate(route);
  };

  return (
    <ErrorBoundary>
      <AppShell
        header={{ height: 60 }}
        padding="md"
        style={{
          backgroundColor: 'var(--site-bg)',
          color: 'var(--text-primary)',
        }}
      >
        <AppShell.Header>
          <Header
            onNavigate={handleNavigate}
            onToggleTheme={toggleTheme}
            currentRoute="market"
          />
        </AppShell.Header>
        
        <AppShell.Main ref={animateRef} style={{ 
          backgroundColor: 'var(--site-bg)',
          minHeight: 'calc(100vh - 60px)',
          padding: '20px 0'
        }}>
          <Tabs value={activeTab} onChange={(value) => setActiveTab(value || 'overview')}>
            <Tabs.List>
              <Tabs.Tab value="overview">Overview</Tabs.Tab>
              <Tabs.Tab value="trading-data">Trading Data</Tabs.Tab>
            </Tabs.List>

            <Tabs.Panel value="overview" pt="md">
              <MarketPage />
            </Tabs.Panel>

            <Tabs.Panel value="trading-data" pt="md">
              <div style={{ textAlign: 'center', padding: '40px' }}>
                <p>Trading Data content coming soon...</p>
              </div>
            </Tabs.Panel>
          </Tabs>
        </AppShell.Main>
      </AppShell>
    </ErrorBoundary>
  );
}
