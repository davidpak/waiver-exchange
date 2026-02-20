'use client';

import { useTrading } from '@/contexts/TradingContext';
import { apiClient } from '@/lib/api-client';
import { useAuthStore } from '@/stores/authStore';
import type { AccountSummaryResponse, OrderType, SymbolInfoResponse } from '@/types/api';
import { formatCents } from '@/utils/format';
import {
  Box,
  Button,
  Group,
  NumberInput,
  SegmentedControl,
  Text,
} from '@mantine/core';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useEffect, useState } from 'react';

interface OrderDetails {
  orderId: string;
  symbol: string;
  side: 'BUY' | 'SELL';
  type: string;
  price: number;
  quantity: number;
  orderStatus: string;
}

interface OrderEntryProps {
  onOrderPlaced?: (status: 'success' | 'error', message: string, orderDetails?: OrderDetails) => void;
}

export function OrderEntry({ onOrderPlaced }: OrderEntryProps) {
  const { selectedSymbolId, fillPrice } = useTrading();
  const { accountId: authAccountId } = useAuthStore();
  const currentAccountId = authAccountId ? parseInt(authAccountId) : 1;

  const [orderType, setOrderType] = useState<OrderType>('LIMIT');
  const [price, setPrice] = useState<number | ''>('');
  const [quantity, setQuantity] = useState<number | ''>(1);
  const [submitting, setSubmitting] = useState(false);

  const { data: symbolInfo } = useQuery<SymbolInfoResponse>({
    queryKey: ['symbol-info', selectedSymbolId],
    queryFn: () => apiClient.rest.getSymbolInfo(selectedSymbolId),
    staleTime: 15000,
  });

  const { data: account } = useQuery<AccountSummaryResponse>({
    queryKey: ['account-summary', currentAccountId],
    queryFn: () => apiClient.rest.getAccountSummary(currentAccountId),
    refetchInterval: 5000,
    staleTime: 2500,
  });

  // Fill price from order book click
  useEffect(() => {
    if (fillPrice != null) {
      setPrice(fillPrice / 100);
    }
  }, [fillPrice]);

  const estimatedTotal = typeof price === 'number' && typeof quantity === 'number'
    ? price * quantity
    : 0;

  const handleOrder = useCallback(async (side: 'BUY' | 'SELL') => {
    if (typeof price !== 'number' || typeof quantity !== 'number' || !symbolInfo) return;

    setSubmitting(true);
    try {
      const result = await apiClient.ws.placeOrder({
        symbol: symbolInfo.name,
        side,
        type: orderType,
        price: Math.round(price * 100),
        quantity,
      });

      onOrderPlaced?.(
        result.status === 'REJECTED' ? 'error' : 'success',
        result.status === 'REJECTED'
          ? 'Order rejected'
          : `Order ${result.status.toLowerCase()}: ${side} ${quantity} ${symbolInfo.name}`,
        {
          orderId: result.order_id,
          symbol: symbolInfo.name,
          side,
          type: orderType,
          price: Math.round(price * 100),
          quantity,
          orderStatus: result.status,
        }
      );
    } catch (err: any) {
      onOrderPlaced?.('error', err.message || 'Failed to place order');
    } finally {
      setSubmitting(false);
    }
  }, [price, quantity, symbolInfo, orderType, onOrderPlaced]);

  const canSubmit = typeof price === 'number' && typeof quantity === 'number' && !!symbolInfo;
  const playerLastName = symbolInfo?.name?.split(' ').pop() || '';

  return (
    <Box
      px="sm"
      py={10}
      style={{ borderTop: '1px solid var(--border-subtle)' }}
    >
      <Group gap="sm" align="flex-end" wrap="nowrap">
        {/* Order type */}
        <Box style={{ flex: '0 0 auto' }}>
          <Text fz={10} c="dark.2" fw={500} mb={4}>Type</Text>
          <SegmentedControl
            value={orderType}
            onChange={(v) => setOrderType(v as OrderType)}
            data={[
              { label: 'Limit', value: 'LIMIT' },
              { label: 'Market', value: 'MARKET' },
              { label: 'IOC', value: 'IOC' },
            ]}
            size="xs"
          />
        </Box>

        {/* Price input */}
        {orderType !== 'MARKET' && (
          <Box style={{ flex: '1 1 100px', minWidth: 90 }}>
            <Text fz={10} c="dark.2" fw={500} mb={4}>Price</Text>
            <NumberInput
              value={price}
              onChange={(v) => setPrice(typeof v === 'number' ? v : '')}
              prefix="$"
              decimalScale={2}
              fixedDecimalScale
              min={0.01}
              step={0.01}
              size="xs"
              classNames={{ input: 'mono' }}
            />
          </Box>
        )}

        {/* Quantity */}
        <Box style={{ flex: '1 1 80px', minWidth: 70 }}>
          <Text fz={10} c="dark.2" fw={500} mb={4}>Qty</Text>
          <NumberInput
            value={quantity}
            onChange={(v) => setQuantity(typeof v === 'number' ? v : '')}
            min={1}
            step={1}
            size="xs"
            classNames={{ input: 'mono' }}
          />
        </Box>

        {/* Total */}
        <Box style={{ flex: '0 0 auto', textAlign: 'right', minWidth: 60 }}>
          <Text fz={10} c="dark.2" fw={500} mb={4}>Total</Text>
          <Text className="mono" fz={13} fw={500} c="dark.0" lh="28px">
            ${estimatedTotal.toFixed(2)}
          </Text>
        </Box>

        {/* Buy + Sell buttons side by side */}
        <Group gap={6} style={{ flex: '0 0 auto' }}>
          <Button
            size="xs"
            px="md"
            radius="md"
            loading={submitting}
            disabled={!canSubmit}
            onClick={() => handleOrder('BUY')}
            style={{
              backgroundColor: 'rgba(52, 211, 153, 0.12)',
              color: 'var(--color-profit)',
              border: '1px solid rgba(52, 211, 153, 0.20)',
            }}
          >
            Buy {playerLastName}
          </Button>
          <Button
            size="xs"
            px="md"
            radius="md"
            loading={submitting}
            disabled={!canSubmit}
            onClick={() => handleOrder('SELL')}
            style={{
              backgroundColor: 'rgba(248, 113, 113, 0.12)',
              color: 'var(--color-loss)',
              border: '1px solid rgba(248, 113, 113, 0.20)',
            }}
          >
            Sell {playerLastName}
          </Button>
        </Group>
      </Group>

      {/* Buying power */}
      {account && (
        <Group justify="flex-end" mt={4}>
          <Text fz={10} c="dark.3">
            Available: {formatCents(account.buying_power)}
          </Text>
        </Group>
      )}
    </Box>
  );
}
