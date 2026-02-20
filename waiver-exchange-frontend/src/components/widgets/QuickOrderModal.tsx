'use client';

import { useTrading } from '@/contexts/TradingContext';
import { apiClient } from '@/lib/api-client';
import { useAuthStore } from '@/stores/authStore';
import type { AccountSummaryResponse, OrderType, SymbolInfoResponse } from '@/types/api';
import { formatCents } from '@/utils/format';
import {
  Box,
  Button,
  Divider,
  Group,
  Modal,
  NumberInput,
  SegmentedControl,
  Stack,
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

interface QuickOrderModalProps {
  side: 'BUY' | 'SELL';
  symbolInfo: SymbolInfoResponse;
  currentPrice: number | null;
  onClose: () => void;
  onOrderPlaced?: (status: 'success' | 'error', message: string, orderDetails?: OrderDetails) => void;
}

export function QuickOrderModal({
  side,
  symbolInfo,
  currentPrice,
  onClose,
  onOrderPlaced,
}: QuickOrderModalProps) {
  const { fillPrice } = useTrading();
  const { accountId: authAccountId } = useAuthStore();
  const currentAccountId = authAccountId ? parseInt(authAccountId) : 1;

  const [orderType, setOrderType] = useState<OrderType>('LIMIT');
  const [price, setPrice] = useState<number | ''>(
    currentPrice ? currentPrice / 100 : ''
  );
  const [quantity, setQuantity] = useState<number | ''>(1);
  const [submitting, setSubmitting] = useState(false);

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

  const handleSubmit = useCallback(async () => {
    if (typeof price !== 'number' || typeof quantity !== 'number') return;

    setSubmitting(true);
    try {
      const result = await apiClient.ws.placeOrder({
        symbol: symbolInfo.name,
        side,
        type: orderType,
        price: Math.round(price * 100),
        quantity,
      });

      onClose();
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
      onClose();
      onOrderPlaced?.('error', err.message || 'Failed to place order');
    } finally {
      setSubmitting(false);
    }
  }, [price, quantity, symbolInfo, side, orderType, onClose, onOrderPlaced]);

  const isBuy = side === 'BUY';
  const sideColor = isBuy ? 'var(--color-profit)' : 'var(--color-loss)';

  return (
    <Modal
      opened
      onClose={onClose}
      title={
        <Group gap={8}>
          <Box
            style={{
              width: 8,
              height: 8,
              borderRadius: '50%',
              backgroundColor: sideColor,
            }}
          />
          <Text fw={600} fz={14}>
            {side} {symbolInfo.name}
          </Text>
        </Group>
      }
      centered
      size="sm"
    >
      <Stack gap="md">
        {/* Current price indicator */}
        <Group justify="space-between">
          <Text fz={12} c="dark.2">Current Price</Text>
          <Text className="mono" fz={13} fw={500} c="dark.0">
            {formatCents(currentPrice)}
          </Text>
        </Group>

        <Divider color="dark.4" />

        {/* Order type */}
        <Box>
          <Text fz={11} c="dark.2" fw={500} mb={6}>Order Type</Text>
          <SegmentedControl
            value={orderType}
            onChange={(v) => setOrderType(v as OrderType)}
            data={[
              { label: 'Limit', value: 'LIMIT' },
              { label: 'Market', value: 'MARKET' },
              { label: 'IOC', value: 'IOC' },
            ]}
            size="xs"
            fullWidth
          />
        </Box>

        {/* Price */}
        {orderType !== 'MARKET' && (
          <NumberInput
            label={<Text fz={11} c="dark.2" fw={500}>Price</Text>}
            value={price}
            onChange={(v) => setPrice(typeof v === 'number' ? v : '')}
            prefix="$"
            decimalScale={2}
            fixedDecimalScale
            min={0.01}
            step={0.01}
            size="sm"
            classNames={{ input: 'mono' }}
          />
        )}

        {/* Quantity */}
        <NumberInput
          label={<Text fz={11} c="dark.2" fw={500}>Shares</Text>}
          value={quantity}
          onChange={(v) => setQuantity(typeof v === 'number' ? v : '')}
          min={1}
          step={1}
          size="sm"
          classNames={{ input: 'mono' }}
        />

        <Divider color="dark.4" />

        {/* Estimated total */}
        <Group justify="space-between">
          <Text fz={12} c="dark.2">Estimated Total</Text>
          <Text className="mono" fz={14} fw={600} c="dark.0">
            ${estimatedTotal.toFixed(2)}
          </Text>
        </Group>

        {/* Buying power */}
        {account && (
          <Group justify="space-between">
            <Text fz={11} c="dark.3">Available</Text>
            <Text className="mono" fz={11} c="dark.2">
              {formatCents(account.buying_power)}
            </Text>
          </Group>
        )}

        {/* Submit */}
        <Button
          fullWidth
          size="md"
          radius="md"
          loading={submitting}
          disabled={!price || !quantity}
          onClick={handleSubmit}
          style={{
            backgroundColor: isBuy ? 'rgba(52, 211, 153, 0.15)' : 'rgba(248, 113, 113, 0.15)',
            color: sideColor,
            border: `1px solid ${isBuy ? 'rgba(52, 211, 153, 0.25)' : 'rgba(248, 113, 113, 0.25)'}`,
          }}
        >
          {side} {symbolInfo.name}
        </Button>
      </Stack>
    </Modal>
  );
}
