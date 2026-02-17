'use client';

import { useNavigation } from '@/contexts/NavigationContext';
import { useAutoAnimate } from '@/hooks/useAutoAnimate';
import { AppShell, Box, Stack, Tabs, Text, useMantineTheme } from '@mantine/core';
import { useState } from 'react';
import { AccountSummary } from '../trading/AccountSummary';
import { SymbolView } from '../trading/SymbolView';
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
  const [activeTab, setActiveTab] = useState<string>('symbol');
  const [selectedSymbolId, setSelectedSymbolId] = useState<number>(764); // Default to Josh Allen
  const { currentRoute, navigate, isNavigating } = useNavigation();
  const theme = useMantineTheme();

  // Handle navigation from header
  const handleNavigation = (route: string) => {
    navigate(route);
    // Also call the prop for backward compatibility
    onNavigate?.(route);
  };

  return (
    <AppShell
      header={{ height: 60 }}
      padding={0}
      styles={{
        root: {
          height: '100vh',
        },
        main: {
          backgroundColor: 'var(--site-bg)',
          height: 'calc(100vh - 60px)', // Full height minus header
          overflow: 'hidden', // Prevent scrolling on main container
        },
      }}
    >
      <Header 
        onNavigate={handleNavigation}
        onToggleTheme={onToggleTheme}
      />

      <AppShell.Main ref={animateRef} p="md" style={{ height: '100%' }}>
        {/* Route-based rendering */}
        {/* Desktop Layout - CSS Grid */}
          <Box visibleFrom="md" style={{ 
            height: '100%',
            display: 'grid',
            gridTemplateColumns: '1fr 1.5fr 1fr', // Left: 1, Center: 1.5, Right: 1
            gridTemplateRows: '1fr', // Single row taking full height
            gap: '12px'
          }}>
          {/* Left Column - Account Summary + Holdings (50/50 split) */}
          <div style={{ 
            display: 'grid',
            gridTemplateRows: '1fr 1fr', // Exactly 50/50 split
            gap: '6px',
            minHeight: 0 // Allow grid items to shrink below content size
          }}>
            <div 
              className="hide-scrollbar"
              style={{ 
                minHeight: 0, // Critical for proper overflow behavior
                overflow: 'auto', // Allow scrolling if content overflows
                height: '100%', // Ensure the container takes full height
                scrollbarWidth: 'none', // Firefox
                msOverflowStyle: 'none', // IE/Edge
              }}
            >
              <AccountSummary />
            </div>
            <div style={{ 
              backgroundColor: 'var(--mantine-color-body)', 
              borderRadius: '8px',
              border: '1px solid var(--mantine-color-default-border)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              color: 'var(--mantine-color-text)',
              minHeight: 0 // Critical for proper overflow behavior
            }}>
              Holdings List
            </div>
          </div>

          {/* Center Column - Symbol View (full height) */}
          <div 
            className="hide-scrollbar"
            style={{ 
              minHeight: 0, // Critical for proper overflow behavior
              overflow: 'auto', // Allow scrolling if content overflows
              height: '100%', // Ensure the container takes full height
              scrollbarWidth: 'none', // Firefox
              msOverflowStyle: 'none', // IE/Edge
            }}
          >
            <SymbolView 
              symbolId={selectedSymbolId} 
              onSymbolChange={setSelectedSymbolId}
              style={{ height: '100%' }} 
            />
          </div>

          {/* Right Column - Order Book + News Feed (50/50 split) */}
          <div style={{ 
            display: 'grid',
            gridTemplateRows: '1fr 1fr', // Exactly 50/50 split
            gap: '12px',
            minHeight: 0 // Allow grid items to shrink below content size
          }}>
            <div style={{ 
              backgroundColor: 'var(--mantine-color-body)', 
              borderRadius: '8px',
              border: '1px solid var(--mantine-color-default-border)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              color: 'var(--mantine-color-text)',
              minHeight: 0, // Critical for proper overflow behavior
              overflow: 'auto' // Allow scrolling if content overflows
            }}>
              Order Book
            </div>
            <div style={{ 
              backgroundColor: 'var(--mantine-color-body)', 
              borderRadius: '8px',
              border: '1px solid var(--mantine-color-default-border)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              color: 'var(--mantine-color-text)',
              minHeight: 0, // Critical for proper overflow behavior
              overflow: 'auto' // Allow scrolling if content overflows
            }}>
              News Feed
            </div>
          </div>
          </Box>

        {/* Mobile Layout - Tabs */}
        <Box hiddenFrom="md" style={{ height: '100%' }}>
          <Tabs 
            value={activeTab} 
            onChange={(value) => setActiveTab(value || 'trading')}
            style={{ height: '100%' }}
            styles={{
              root: {
                height: '100%',
                display: 'grid',
                gridTemplateRows: 'auto 1fr', // Tab list auto, content takes remaining
              },
              list: {
                borderBottom: '1px solid var(--mantine-color-default-border)',
              },
              panel: {
                overflow: 'auto', // Allow scrolling within panels
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
              <Tabs.Tab value="symbol">Symbol</Tabs.Tab>
              <Tabs.Tab value="market">Market</Tabs.Tab>
              <Tabs.Tab value="trading">Trading</Tabs.Tab>
              <Tabs.Tab value="holdings">Holdings</Tabs.Tab>
              <Tabs.Tab value="account">Account</Tabs.Tab>
            </Tabs.List>

            <Tabs.Panel value="symbol" pt="md" style={{ height: '100%' }}>
              <div 
                className="hide-scrollbar"
                style={{ 
                  minHeight: 0, // Critical for proper overflow behavior
                  overflow: 'auto', // Allow scrolling if content overflows
                  height: '100%', // Ensure the container takes full height
                  scrollbarWidth: 'none', // Firefox
                  msOverflowStyle: 'none', // IE/Edge
                }}
              >
                <SymbolView 
                  symbolId={selectedSymbolId} 
                  onSymbolChange={setSelectedSymbolId}
                  style={{ minHeight: '100%' }} 
                />
              </div>
            </Tabs.Panel>

            <Tabs.Panel value="market" pt="md" style={{ height: '100%' }}>
              <div style={{ 
                height: '100%', 
                display: 'flex', 
                alignItems: 'center', 
                justifyContent: 'center',
                flexDirection: 'column',
                gap: '20px'
              }}>
                <Text size="lg" c="dimmed" ta="center">
                  Market Overview has moved to its own page
                </Text>
                <a 
                  href="/market" 
                  style={{ 
                    color: 'var(--text-primary)', 
                    textDecoration: 'underline',
                    fontSize: '16px',
                    fontWeight: 500
                  }}
                >
                  Go to Market Overview →
                </a>
              </div>
            </Tabs.Panel>

            <Tabs.Panel value="trading" pt="md" style={{ height: '100%' }}>
              <div style={{ 
                height: '100%',
                display: 'grid',
                gridTemplateRows: '1fr 1fr', // Exactly 50/50 split
                gap: '12px',
                minHeight: 0 // Allow grid items to shrink below content size
              }}>
                <div 
                  className="hide-scrollbar"
                  style={{ 
                    backgroundColor: 'var(--mantine-color-body)', 
                    borderRadius: '8px',
                    border: '1px solid var(--mantine-color-default-border)',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    color: 'var(--mantine-color-text)',
                    minHeight: 0, // Critical for proper overflow behavior
                    overflow: 'auto', // Allow scrolling if content overflows
                    scrollbarWidth: 'none', // Firefox
                    msOverflowStyle: 'none', // IE/Edge
                  }}
                >
                  Order Book
                </div>
                <div 
                  className="hide-scrollbar"
                  style={{ 
                    backgroundColor: 'var(--mantine-color-body)', 
                    borderRadius: '8px',
                    border: '1px solid var(--mantine-color-default-border)',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    color: 'var(--mantine-color-text)',
                    minHeight: 0, // Critical for proper overflow behavior
                    overflow: 'auto', // Allow scrolling if content overflows
                    scrollbarWidth: 'none', // Firefox
                    msOverflowStyle: 'none', // IE/Edge
                  }}
                >
                  News Feed
                </div>
              </div>
            </Tabs.Panel>

            <Tabs.Panel value="holdings" pt="md" style={{ height: '100%' }}>
              <div 
                className="hide-scrollbar"
                style={{ 
                  minHeight: 0, // Critical for proper overflow behavior
                  overflow: 'auto', // Allow scrolling if content overflows
                  height: '100%', // Ensure the container takes full height
                  scrollbarWidth: 'none', // Firefox
                  msOverflowStyle: 'none', // IE/Edge
                }}
              >
                <div style={{ 
                  backgroundColor: 'var(--mantine-color-body)', 
                  borderRadius: '8px',
                  border: '1px solid var(--mantine-color-default-border)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  color: 'var(--mantine-color-text)',
                  minHeight: '100%', // Take full height
                  padding: '2rem',
                }}>
                  <Stack gap="xs" align="center">
                    <Text size="lg" fw={500}>Holdings List</Text>
                    <Text c="dimmed" size="sm" ta="center">
                      Your current positions and holdings will be displayed here
                    </Text>
                  </Stack>
                </div>
              </div>
            </Tabs.Panel>

            <Tabs.Panel value="account" pt="md" style={{ height: '100%' }}>
              <div 
                className="hide-scrollbar"
                style={{ 
                  minHeight: 0, // Critical for proper overflow behavior
                  overflow: 'auto', // Allow scrolling if content overflows
                  height: '100%', // Ensure the container takes full height
                  scrollbarWidth: 'none', // Firefox
                  msOverflowStyle: 'none', // IE/Edge
                }}
              >
                <AccountSummary />
              </div>
            </Tabs.Panel>
          </Tabs>
        </Box>
      </AppShell.Main>
    </AppShell>
  );
}