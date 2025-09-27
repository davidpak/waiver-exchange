'use client';

import { useAutoAnimate } from '@/hooks/useAutoAnimate';
import { AppShell, Box, Grid, Stack, Tabs, useMantineTheme } from '@mantine/core';
import { useState } from 'react';
import { AccountSummary } from '../trading/AccountSummary';
import { Header } from './Header';

interface TradingLayoutProps {
  onNavigate?: (route: string) => void;
  onToggleTheme?: () => void;
}

/**
 * Professional trading layout with smooth animations and proper component separation
 * Main layout component that orchestrates the entire trading dashboard
 */
export function TradingLayout({ 
  onNavigate,
  onToggleTheme 
}: TradingLayoutProps) {
  const [animateRef] = useAutoAnimate();
  const [activeTab, setActiveTab] = useState<string>('trading');
  const theme = useMantineTheme();

  return (
    <AppShell
      header={{ height: 60 }}
      padding={0}
      styles={{
        main: {
          backgroundColor: 'var(--site-bg)',
        },
      }}
    >
      <Header 
        onNavigate={onNavigate}
        onToggleTheme={onToggleTheme}
      />

      <AppShell.Main ref={animateRef} p="md">
        {/* Desktop Layout - Grid */}
        <Box visibleFrom="md">
          <Grid>
            {/* Left Column */}
            <Grid.Col span={4}>
              <Stack gap="xs">
                {/* Account Summary Component */}
                <AccountSummary />
                
                {/* Holdings List Component */}
                <div style={{ 
                  height: '220px', 
                  backgroundColor: 'var(--mantine-color-body)', 
                  borderRadius: '8px',
                  border: '1px solid var(--mantine-color-default-border)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  color: 'var(--mantine-color-text)'
                }}>
                  Holdings List
                </div>
              </Stack>
            </Grid.Col>

            {/* Center Column - Symbol View & Analytics (spans full height) */}
            <Grid.Col span={5}>
              <div style={{ 
                height: '580px', // 350px + 220px + 10px gap
                backgroundColor: 'var(--mantine-color-body)', 
                borderRadius: '8px',
                border: '1px solid var(--mantine-color-default-border)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                color: 'var(--mantine-color-text)'
              }}>
                Symbol View & Analytics
              </div>
            </Grid.Col>

            {/* Right Column */}
            <Grid.Col span={3}>
              <Stack gap="xs">
                {/* Order Book Component */}
                <div style={{ 
                  height: '350px', 
                  backgroundColor: 'var(--mantine-color-body)', 
                  borderRadius: '8px',
                  border: '1px solid var(--mantine-color-default-border)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  color: 'var(--mantine-color-text)'
                }}>
                  Order Book
                </div>
                
                {/* News Feed Component */}
                <div style={{ 
                  height: '220px', 
                  backgroundColor: 'var(--mantine-color-body)', 
                  borderRadius: '8px',
                  border: '1px solid var(--mantine-color-default-border)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  color: 'var(--mantine-color-text)'
                }}>
                  News Feed
                </div>
              </Stack>
            </Grid.Col>
          </Grid>
        </Box>

        {/* Mobile Layout - Tabs */}
        <Box hiddenFrom="md">
          <Tabs 
            value={activeTab} 
            onChange={(value) => setActiveTab(value || 'trading')}
            styles={{
              list: {
                borderBottom: '1px solid var(--mantine-color-default-border)',
              },
              tab: {
                color: 'var(--mantine-color-text)',
                '&[data-active]': {
                  color: 'var(--mantine-color-blue-6)',
                  borderBottomColor: 'var(--mantine-color-blue-6)',
                },
                '&:hover': {
                  color: 'var(--mantine-color-blue-6)',
                  backgroundColor: 'var(--mantine-color-default-hover)',
                },
              },
            }}
          >
            <Tabs.List>
              <Tabs.Tab value="trading">Trading</Tabs.Tab>
              <Tabs.Tab value="holdings">Holdings</Tabs.Tab>
              <Tabs.Tab value="account">Account</Tabs.Tab>
            </Tabs.List>

            <Tabs.Panel value="trading" pt="md">
              <Stack gap="md">
                {/* Symbol View & Analytics */}
                <div style={{ 
                  height: '400px', 
                  backgroundColor: 'var(--mantine-color-body)', 
                  borderRadius: '8px',
                  border: '1px solid var(--mantine-color-default-border)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  color: 'var(--mantine-color-text)'
                }}>
                  Symbol View & Analytics
                </div>
                
                {/* Order Book */}
                <div style={{ 
                  height: '300px', 
                  backgroundColor: 'var(--mantine-color-body)', 
                  borderRadius: '8px',
                  border: '1px solid var(--mantine-color-default-border)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  color: 'var(--mantine-color-text)'
                }}>
                  Order Book
                </div>
              </Stack>
            </Tabs.Panel>

            <Tabs.Panel value="holdings" pt="md">
              <div style={{ 
                height: '500px', 
                backgroundColor: 'var(--mantine-color-body)', 
                borderRadius: '8px',
                border: '1px solid var(--mantine-color-default-border)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                color: 'var(--mantine-color-text)'
              }}>
                Holdings List
              </div>
            </Tabs.Panel>

            <Tabs.Panel value="account" pt="md">
              <AccountSummary />
            </Tabs.Panel>
          </Tabs>
        </Box>
      </AppShell.Main>
    </AppShell>
  );
}