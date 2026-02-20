'use client';

import { formatCents } from '@/utils/format';
import { Badge, Box, Group, Stack, Text } from '@mantine/core';

export interface TrackedOrder {
  id: string;
  symbol: string;
  side: 'BUY' | 'SELL';
  type: string;
  price: number;     // in cents
  quantity: number;
  status: 'ACCEPTED' | 'FILLED' | 'REJECTED' | 'PENDING';
  timestamp: number;
}

interface OpenOrdersProps {
  orders: TrackedOrder[];
}

export function OpenOrders({ orders }: OpenOrdersProps) {
  const openOrders = orders.filter((o) => o.status === 'ACCEPTED' || o.status === 'PENDING');

  if (openOrders.length === 0) {
    return (
      <Stack align="center" justify="center" py="lg" gap={4}>
        <Text fz={11} c="dark.3">No open orders</Text>
        <Text fz={10} c="dark.3">Limit orders will appear here</Text>
      </Stack>
    );
  }

  return (
    <Stack gap={0}>
      {openOrders.map((order) => {
        const isBuy = order.side === 'BUY';
        return (
          <Box
            key={order.id}
            px="sm"
            py={6}
            style={{ borderBottom: '1px solid var(--border-subtle)' }}
          >
            <Group justify="space-between" wrap="nowrap">
              <Group gap={8} wrap="nowrap" style={{ minWidth: 0 }}>
                <Badge
                  size="xs"
                  variant="light"
                  color={isBuy ? 'green' : 'red'}
                  w={32}
                  fz={9}
                  style={{ textAlign: 'center', flexShrink: 0 }}
                >
                  {order.side}
                </Badge>
                <Box style={{ minWidth: 0 }}>
                  <Text fz={11} fw={500} truncate>{order.symbol}</Text>
                  <Text fz={9} c="dark.3">
                    {order.type} &middot; {new Date(order.timestamp).toLocaleTimeString()}
                  </Text>
                </Box>
              </Group>
              <Box ta="right" style={{ flexShrink: 0 }}>
                <Text className="mono" fz={11}>
                  {order.quantity} @ {formatCents(order.price)}
                </Text>
                <Badge size="xs" variant="dot" color="gold">
                  {order.status}
                </Badge>
              </Box>
            </Group>
          </Box>
        );
      })}
    </Stack>
  );
}
