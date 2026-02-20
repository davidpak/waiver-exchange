'use client';

import { ErrorBoundary } from '@/components/common/ErrorBoundary';
import { AppShellLayout } from '@/components/layout/AppShell';
import { AccountSummary } from '@/components/widgets/AccountSummary';
import { CommandPalette } from '@/components/widgets/CommandPalette';
import { EquityChart } from '@/components/widgets/EquityChart';
import { Holdings } from '@/components/widgets/Holdings';
import { OpenOrders, type TrackedOrder } from '@/components/widgets/OpenOrders';
import { OrderBook } from '@/components/widgets/OrderBook';
import { OrderEntry } from '@/components/widgets/OrderEntry';
import {
  OrderNotification,
  useOrderNotifications,
} from '@/components/widgets/OrderNotification';
import { SymbolView } from '@/components/widgets/SymbolView';
import { TradeHistory } from '@/components/widgets/TradeHistory';
import { useTrading } from '@/contexts/TradingContext';
import { Box, Tabs, Text } from '@mantine/core';
import { useCallback, useState } from 'react';

export default function Home() {
  const {
    setSelectedSymbolId,
    searchOpen,
    closeSearch,
  } = useTrading();
  const { notifications, addNotification, dismissNotification } = useOrderNotifications();
  const [bottomTab, setBottomTab] = useState('orders');
  const [trackedOrders, setTrackedOrders] = useState<TrackedOrder[]>([]);

  const handleOrderPlaced = useCallback(
    (status: 'success' | 'error', message: string, orderDetails?: {
      orderId: string;
      symbol: string;
      side: 'BUY' | 'SELL';
      type: string;
      price: number;
      quantity: number;
      orderStatus: string;
    }) => {
      addNotification(status, message);

      // Track accepted orders
      if (orderDetails && orderDetails.orderStatus !== 'REJECTED') {
        setTrackedOrders((prev) => [
          {
            id: orderDetails.orderId,
            symbol: orderDetails.symbol,
            side: orderDetails.side,
            type: orderDetails.type,
            price: orderDetails.price,
            quantity: orderDetails.quantity,
            status: orderDetails.orderStatus as TrackedOrder['status'],
            timestamp: Date.now(),
          },
          ...prev,
        ]);
      }
    },
    [addNotification]
  );

  return (
    <ErrorBoundary>
      <AppShellLayout
        left={
          <>
            <AccountSummary />
            <Box style={{ borderTop: '1px solid var(--border-subtle)' }}>
              <EquityChart />
            </Box>
            <Holdings />
          </>
        }
        center={
          <Box style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
            {/* Chart area — takes majority of space */}
            <Box style={{ flex: '1 1 55%', minHeight: 250 }}>
              <SymbolView onOrderPlaced={handleOrderPlaced} />
            </Box>

            {/* Quick order entry bar */}
            <OrderEntry onOrderPlaced={handleOrderPlaced} />

            {/* Bottom tabs: Orders / Trades */}
            <Box style={{ flex: '1 1 25%', minHeight: 100, borderTop: '1px solid var(--border-subtle)' }}>
              <Tabs
                value={bottomTab}
                onChange={(v) => setBottomTab(v || 'orders')}
                variant="unstyled"
              >
                <Tabs.List
                  style={{
                    borderBottom: '1px solid var(--border-subtle)',
                    display: 'flex',
                    gap: 0,
                  }}
                >
                  <Tabs.Tab
                    value="orders"
                    px="sm"
                    py={6}
                    fz={11}
                    fw={600}
                    c={bottomTab === 'orders' ? 'gold.3' : 'dark.2'}
                    style={{
                      borderBottom: bottomTab === 'orders'
                        ? '2px solid var(--mantine-color-gold-3)'
                        : '2px solid transparent',
                    }}
                  >
                    Open Orders
                    {trackedOrders.filter(o => o.status === 'ACCEPTED' || o.status === 'PENDING').length > 0 && (
                      <Text component="span" fz={9} ml={4} c="gold.3">
                        ({trackedOrders.filter(o => o.status === 'ACCEPTED' || o.status === 'PENDING').length})
                      </Text>
                    )}
                  </Tabs.Tab>
                  <Tabs.Tab
                    value="trades"
                    px="sm"
                    py={6}
                    fz={11}
                    fw={600}
                    c={bottomTab === 'trades' ? 'gold.3' : 'dark.2'}
                    style={{
                      borderBottom: bottomTab === 'trades'
                        ? '2px solid var(--mantine-color-gold-3)'
                        : '2px solid transparent',
                    }}
                  >
                    Trade History
                  </Tabs.Tab>
                </Tabs.List>

                <Tabs.Panel value="orders" style={{ overflow: 'auto' }}>
                  <OpenOrders orders={trackedOrders} />
                </Tabs.Panel>

                <Tabs.Panel value="trades" style={{ overflow: 'auto' }}>
                  <TradeHistory />
                </Tabs.Panel>
              </Tabs>
            </Box>
          </Box>
        }
        right={
          <OrderBook />
        }
      />
      <CommandPalette
        opened={searchOpen}
        onClose={closeSearch}
        onSelect={setSelectedSymbolId}
      />
      <OrderNotification
        notifications={notifications}
        onDismiss={dismissNotification}
      />
    </ErrorBoundary>
  );
}
