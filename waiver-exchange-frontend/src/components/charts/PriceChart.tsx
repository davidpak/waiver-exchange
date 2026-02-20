'use client';

import type { CandleData } from '@/types/api';
import { Box, Group, Text } from '@mantine/core';
import { useEffect, useRef, useState } from 'react';

interface PriceChartProps {
  candles: CandleData[];
  style?: React.CSSProperties;
}

export function PriceChart({ candles, style }: PriceChartProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<any>(null);
  const seriesRef = useRef<any>(null);
  const volumeRef = useRef<any>(null);
  const [crosshair, setCrosshair] = useState<{
    time: string;
    open: number;
    high: number;
    low: number;
    close: number;
  } | null>(null);

  useEffect(() => {
    if (!containerRef.current || candles.length === 0) return;

    let chart: any;

    const init = async () => {
      const { createChart, CandlestickSeries, HistogramSeries } = await import('lightweight-charts');

      if (!containerRef.current) return;

      chart = createChart(containerRef.current, {
        layout: {
          background: { color: 'transparent' },
          textColor: '#6e737c',
          fontFamily: 'var(--font-mono)',
          fontSize: 10,
        },
        grid: {
          vertLines: { color: 'rgba(255,255,255,0.02)' },
          horzLines: { color: 'rgba(255,255,255,0.02)' },
        },
        crosshair: {
          vertLine: { color: 'rgba(232,185,49,0.3)', width: 1, style: 2 },
          horzLine: { color: 'rgba(232,185,49,0.3)', width: 1, style: 2 },
        },
        timeScale: {
          borderColor: '#252830',
          timeVisible: true,
          secondsVisible: false,
        },
        rightPriceScale: {
          borderColor: '#252830',
        },
        handleScroll: { mouseWheel: true, pressedMouseMove: true },
        handleScale: { axisPressedMouseMove: true, mouseWheel: true },
      });

      chartRef.current = chart;

      // Candlestick series
      const candleSeries = chart.addSeries(CandlestickSeries, {
        upColor: '#00DC82',
        downColor: '#FF4757',
        borderUpColor: '#00DC82',
        borderDownColor: '#FF4757',
        wickUpColor: '#00DC82',
        wickDownColor: '#FF4757',
      });
      seriesRef.current = candleSeries;

      // Volume histogram
      const volumeSeries = chart.addSeries(HistogramSeries, {
        priceFormat: { type: 'volume' },
        priceScaleId: 'volume',
      });
      volumeRef.current = volumeSeries;

      chart.priceScale('volume').applyOptions({
        scaleMargins: { top: 0.85, bottom: 0 },
      });

      // Transform data: ISO timestamps to Unix, cents to dollars
      const chartData = candles
        .map((c) => ({
          time: Math.floor(new Date(c.timestamp).getTime() / 1000) as any,
          open: c.open / 100,
          high: c.high / 100,
          low: c.low / 100,
          close: c.close / 100,
        }))
        .sort((a: any, b: any) => a.time - b.time);

      const volumeData = candles
        .map((c) => ({
          time: Math.floor(new Date(c.timestamp).getTime() / 1000) as any,
          value: c.volume,
          color: c.close >= c.open ? 'rgba(0,220,130,0.25)' : 'rgba(255,71,87,0.25)',
        }))
        .sort((a: any, b: any) => a.time - b.time);

      candleSeries.setData(chartData);
      volumeSeries.setData(volumeData);
      chart.timeScale().fitContent();

      // Crosshair tooltip
      chart.subscribeCrosshairMove((param: any) => {
        if (!param || !param.time || !param.seriesData) {
          setCrosshair(null);
          return;
        }
        const data = param.seriesData.get(candleSeries);
        if (data) {
          setCrosshair({
            time: new Date((param.time as number) * 1000).toLocaleString(),
            open: data.open,
            high: data.high,
            low: data.low,
            close: data.close,
          });
        }
      });

      // Resize observer
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
  }, [candles]);

  return (
    <Box style={{ position: 'relative', width: '100%', height: '100%', minHeight: 250, ...style }}>
      {/* OHLC tooltip */}
      {crosshair && (
        <Group
          gap="sm"
          style={{
            position: 'absolute',
            top: 4,
            left: 4,
            zIndex: 10,
          }}
        >
          <Text className="mono" fz={10} c="dark.2">{crosshair.time}</Text>
          <Text className="mono" fz={10} c="dark.1">O {crosshair.open.toFixed(2)}</Text>
          <Text className="mono" fz={10} c="dark.1">H {crosshair.high.toFixed(2)}</Text>
          <Text className="mono" fz={10} c="dark.1">L {crosshair.low.toFixed(2)}</Text>
          <Text className="mono" fz={10} c="dark.1">C {crosshair.close.toFixed(2)}</Text>
        </Group>
      )}
      <div ref={containerRef} style={{ width: '100%', height: '100%' }} />
    </Box>
  );
}
