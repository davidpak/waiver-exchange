'use client';

import { Box, Tabs } from '@mantine/core';
import {
  IconBook,
  IconChartCandle,
  IconLayoutList,
  IconWallet,
} from '@tabler/icons-react';
import { ReactNode, useState } from 'react';
import { Header } from './Header';
import { TickerBar } from './TickerBar';

const TICKER_H = 36;

interface AppShellLayoutProps {
  left?: ReactNode;
  center: ReactNode;
  right?: ReactNode;
}

const mobileTabs = [
  { value: 'chart', label: 'Chart', icon: IconChartCandle },
  { value: 'book', label: 'Book', icon: IconBook },
  { value: 'portfolio', label: 'Portfolio', icon: IconWallet },
  { value: 'market', label: 'Market', icon: IconLayoutList },
];

export function AppShellLayout({
  left,
  center,
  right,
}: AppShellLayoutProps) {
  const [mobileTab, setMobileTab] = useState('chart');

  const getMobileContent = () => {
    switch (mobileTab) {
      case 'chart': return center;
      case 'book': return right;
      case 'portfolio': return left;
      case 'market': return center;
      default: return center;
    }
  };

  return (
    <Box style={{ height: '100vh', display: 'flex', flexDirection: 'column' }}>
      <Header />

      {/* Desktop: 3-column grid */}
      <Box
        visibleFrom="md"
        style={{
          flex: 1,
          display: 'grid',
          gridTemplateColumns: '260px 1fr 280px',
          minHeight: 0,
          marginBottom: TICKER_H,
        }}
      >
        {/* Left: Account + Holdings */}
        <Box
          className="hide-scrollbar"
          style={{
            overflow: 'auto',
            display: 'flex',
            flexDirection: 'column',
            borderRight: '1px solid var(--border-subtle)',
          }}
        >
          {left}
        </Box>

        {/* Center: Chart + OrderEntry + Orders/Trades tabs */}
        <Box
          className="hide-scrollbar"
          style={{
            overflow: 'auto',
            display: 'flex',
            flexDirection: 'column',
          }}
        >
          {center}
        </Box>

        {/* Right: Order Book (full height) */}
        <Box
          className="hide-scrollbar"
          style={{
            overflow: 'auto',
            display: 'flex',
            flexDirection: 'column',
            borderLeft: '1px solid var(--border-subtle)',
          }}
        >
          {right}
        </Box>
      </Box>

      {/* Mobile: single view + bottom tabs */}
      <Box
        hiddenFrom="md"
        style={{
          flex: 1,
          display: 'flex',
          flexDirection: 'column',
          minHeight: 0,
        }}
      >
        <Box
          className="hide-scrollbar"
          style={{ flex: 1, overflow: 'auto' }}
        >
          {getMobileContent()}
        </Box>

        <Tabs
          value={mobileTab}
          onChange={(v) => setMobileTab(v || 'chart')}
          variant="unstyled"
        >
          <Tabs.List
            grow
            bg="dark.9"
            style={{
              display: 'flex',
              justifyContent: 'space-around',
              padding: '6px 0',
              borderTop: '1px solid var(--border-subtle)',
            }}
          >
            {mobileTabs.map((tab) => (
              <Tabs.Tab
                key={tab.value}
                value={tab.value}
                c={mobileTab === tab.value ? 'gold.3' : 'dimmed'}
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  alignItems: 'center',
                  gap: 2,
                  padding: '6px 12px',
                  border: 'none',
                  fontSize: 10,
                  fontWeight: mobileTab === tab.value ? 600 : 400,
                }}
              >
                <tab.icon size={18} stroke={1.5} />
                {tab.label}
              </Tabs.Tab>
            ))}
          </Tabs.List>
        </Tabs>
      </Box>

      {/* Bottom ticker — desktop only */}
      <Box visibleFrom="md">
        <TickerBar />
      </Box>
    </Box>
  );
}
