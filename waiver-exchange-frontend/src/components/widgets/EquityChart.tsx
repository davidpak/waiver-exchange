'use client';

import { apiClient } from '@/lib/api-client';
import { useAuthStore } from '@/stores/authStore';
import type { EquityHistoryResponse } from '@/types/api';
import { formatCents } from '@/utils/format';
import { Box, Button, Group, Skeleton, Text } from '@mantine/core';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';

const TIME_RANGES = ['1W', '1M', '3M', '1Y', 'ALL'] as const;

function getRangeStartDate(range: string): string | undefined {
  const now = new Date();
  switch (range) {
    case '1W': now.setDate(now.getDate() - 7); break;
    case '1M': now.setMonth(now.getMonth() - 1); break;
    case '3M': now.setMonth(now.getMonth() - 3); break;
    case '1Y': now.setFullYear(now.getFullYear() - 1); break;
    case 'ALL': return undefined;
    default: return undefined;
  }
  return now.toISOString().split('T')[0];
}

export function EquityChart() {
  const { accountId: authAccountId } = useAuthStore();
  const currentAccountId = authAccountId ? parseInt(authAccountId) : 1;
  const [range, setRange] = useState<string>('1M');
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<any>(null);

  const startDate = getRangeStartDate(range);

  const initialLoadDone = useRef(false);
  const { data, isLoading } = useQuery<EquityHistoryResponse>({
    queryKey: ['equity-history', currentAccountId, range],
    queryFn: () => apiClient.rest.getEquityHistory(currentAccountId, startDate),
    staleTime: 30000,
    placeholderData: keepPreviousData,
    enabled: !!currentAccountId,
  });
  if (!isLoading) initialLoadDone.current = true;

  useEffect(() => {
    if (!containerRef.current || !data?.snapshots?.length) return;

    let chart: any;

    const init = async () => {
      const { createChart, AreaSeries } = await import('lightweight-charts');

      if (!containerRef.current) return;

      chart = createChart(containerRef.current, {
        layout: {
          background: { color: 'transparent' },
          textColor: '#6e737c',
          fontFamily: 'var(--font-mono)',
          fontSize: 10,
        },
        grid: {
          vertLines: { visible: false },
          horzLines: { color: 'rgba(255,255,255,0.03)' },
        },
        crosshair: {
          vertLine: { color: 'rgba(255,255,255,0.15)', width: 1, style: 2 },
          horzLine: { visible: false },
        },
        timeScale: {
          borderColor: '#252830',
        },
        rightPriceScale: {
          borderColor: '#252830',
        },
        handleScroll: false,
        handleScale: false,
      });

      chartRef.current = chart;

      const snapshots = data.snapshots;
      const first = snapshots[0]?.total_equity ?? 0;
      const last = snapshots[snapshots.length - 1]?.total_equity ?? 0;
      const isProfit = last >= first;

      const lineColor = isProfit ? '#0ECB81' : '#F6465D';
      const topGradient = isProfit ? 'rgba(14,203,129,0.2)' : 'rgba(246,70,93,0.2)';

      const series = chart.addSeries(AreaSeries, {
        lineColor,
        topColor: topGradient,
        bottomColor: 'transparent',
        lineWidth: 2,
        crosshairMarkerRadius: 3,
        crosshairMarkerBackgroundColor: lineColor,
      });

      const chartData = snapshots.map((s) => ({
        time: s.date as any,
        value: s.total_equity / 100,
      }));

      series.setData(chartData);
      chart.timeScale().fitContent();

      const ro = new ResizeObserver(() => {
        if (containerRef.current && chart) {
          chart.applyOptions({
            width: containerRef.current.clientWidth,
            height: containerRef.current.clientHeight,
          });
        }
      });
      ro.observe(containerRef.current);

      return () => ro.disconnect();
    };

    init();

    return () => {
      if (chart) chart.remove();
    };
  }, [data]);

  const first = data?.snapshots?.[0]?.total_equity;
  const last = data?.snapshots?.[data.snapshots.length - 1]?.total_equity;
  const change = first != null && last != null ? last - first : null;

  return (
    <Box>
      {/* Header */}
      <Group justify="space-between" px="sm" py={6}>
        <Text fz={10} c="dark.2">
          {change != null && (
            <Text
              component="span"
              className="mono"
              fz={10}
              style={{ color: change >= 0 ? 'var(--color-profit)' : 'var(--color-loss)' }}
            >
              {change >= 0 ? '+' : ''}{formatCents(change)} ({range})
            </Text>
          )}
        </Text>
        <Group gap={2}>
          {TIME_RANGES.map((r) => (
            <Button
              key={r}
              variant={range === r ? 'filled' : 'subtle'}
              color={range === r ? 'gold' : 'gray'}
              size="compact-xs"
              fz={9}
              fw={600}
              px={6}
              onClick={() => setRange(r)}
            >
              {r}
            </Button>
          ))}
        </Group>
      </Group>

      {/* Chart */}
      <Box style={{ height: 120 }} px={4}>
        {!initialLoadDone.current ? (
          <Skeleton height={120} />
        ) : (
          <div ref={containerRef} style={{ width: '100%', height: '100%' }} />
        )}
      </Box>
    </Box>
  );
}
