'use client';

import { ColorType, createChart, IChartApi, ISeriesApi, LineData, LineSeries, Time } from 'lightweight-charts';
import React, { useEffect, useRef } from 'react';

// Design system colors
const COLORS = {
  profit: '#00ca40',
  loss: '#f21616',
  baseline: '#666666',
} as const;

interface EquityChartProps {
  accountId: number;
  className?: string;
  style?: React.CSSProperties;
}

export function EquityChart({ accountId, className, style }: EquityChartProps) {
  const chartContainerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const lineSeriesRef = useRef<ISeriesApi<'Line'> | null>(null);
  const baselineSeriesRef = useRef<ISeriesApi<'Line'> | null>(null);

  // Generate market hours data (6:30 AM - 1:00 PM PST, 5-minute intervals)
  const generateMarketHoursData = () => {
    const now = new Date();
    const data = [];
    let baseValue = 50000; // $500.00 starting value
    
    // Market hours: 6:30 AM - 1:00 PM PST (6.5 hours = 78 five-minute intervals)
    const marketOpenHour = 6;
    const marketOpenMinute = 30;
    const marketCloseHour = 13; // 1:00 PM
    const marketCloseMinute = 0;
    
    // Create today's market open time
    const today = new Date(now);
    const marketOpen = new Date(today.getFullYear(), today.getMonth(), today.getDate(), marketOpenHour, marketOpenMinute, 0);
    const marketClose = new Date(today.getFullYear(), today.getMonth(), today.getDate(), marketCloseHour, marketCloseMinute, 0);
    
    // Always show full day's data (78 five-minute intervals)
    const totalIntervals = 78;
    
    for (let i = 0; i <= totalIntervals; i++) {
      // Create timestamp for this 5-minute interval
      const time = new Date(marketOpen.getTime() + (i * 5 * 60 * 1000));
      
      // Add realistic market fluctuation (more volatile during market hours)
      const timeProgress = i / 78; // Progress through the day (0 to 1)
      const volatility = 1000 + (timeProgress * 500); // Increase volatility as day progresses
      const fluctuation = (Math.random() - 0.5) * volatility; // ±$10-15 range
      
      // Add a slight upward trend with some randomness
      const trend = i * 25 + (Math.random() - 0.5) * 100; // Small upward trend with noise
      const value = baseValue + fluctuation + trend;
      
      data.push({
        date: time.toISOString(),
        total_equity: Math.max(value, 10000), // Minimum $100.00
        cash_balance: 30000,
        position_value: Math.max(value - 30000, 0),
        unrealized_pnl: fluctuation,
        realized_pnl: 0,
        day_change: value - baseValue,
        day_change_percent: ((value - baseValue) / baseValue) * 100,
      });
    }
    
    // Sort by date to ensure ascending order (oldest first)
    data.sort((a, b) => new Date(a.date).getTime() - new Date(b.date).getTime());
    
    return { snapshots: data };
  };

  const equityData = generateMarketHoursData();
  const isLoading = false;

  // Initialize chart
  useEffect(() => {
    if (!chartContainerRef.current) return;

    // Create chart with minimal styling
    const chart = createChart(chartContainerRef.current, {
      width: chartContainerRef.current.clientWidth,
      height: 150,
      layout: {
        background: { type: ColorType.Solid, color: 'transparent' },
        textColor: 'var(--text-primary)',
      },
      grid: {
        vertLines: { visible: false },
        horzLines: { visible: false },
      },
      rightPriceScale: {
        visible: false,
      },
      timeScale: {
        visible: false,
      },
      crosshair: {
        mode: 0, // Disable crosshair
      },
      handleScroll: {
        mouseWheel: false,
        pressedMouseMove: false,
      },
      handleScale: {
        axisPressedMouseMove: false,
        mouseWheel: false,
        pinch: false,
      },
    });

    // Create main equity line series
    const lineSeries = chart.addSeries(LineSeries, {
      color: COLORS.profit,
      lineWidth: 2,
    });

    // Create baseline series (dotted line for day start)
    const baselineSeries = chart.addSeries(LineSeries, {
      color: COLORS.baseline,
      lineWidth: 1,
      lineStyle: 2, // Dotted line
    });

    chartRef.current = chart;
    lineSeriesRef.current = lineSeries;
    baselineSeriesRef.current = baselineSeries;

    // Handle resize
    const handleResize = () => {
      if (chartContainerRef.current && chart) {
        chart.applyOptions({
          width: chartContainerRef.current.clientWidth,
        });
      }
    };

    window.addEventListener('resize', handleResize);

    return () => {
      window.removeEventListener('resize', handleResize);
      chart.remove();
    };
  }, []);

  // Update chart data when equity data changes
  useEffect(() => {
    if (!equityData?.snapshots || !lineSeriesRef.current || !baselineSeriesRef.current) return;

    // Transform data for Lightweight Charts
    const chartData: LineData[] = equityData.snapshots.map((snapshot, index) => ({
      time: (new Date(snapshot.date).getTime() / 1000) as Time, // Convert to Unix timestamp
      value: snapshot.total_equity / 100, // Convert cents to dollars
    }));

    // Debug: Log the data to see what's happening
    console.log('Chart data:', chartData.slice(0, 5)); // Log first 5 points

    // Get day start value (first data point)
    const dayStartValue = chartData.length > 0 ? chartData[0].value : 0;

    // Create baseline data (horizontal line at day start)
    const baselineData: LineData[] = chartData.map((point) => ({
      time: point.time,
      value: dayStartValue,
    }));

    // Determine line color based on current vs day start
    const currentValue = chartData.length > 0 ? chartData[chartData.length - 1].value : dayStartValue;
    const lineColor = currentValue >= dayStartValue ? COLORS.profit : COLORS.loss;

    // Update series
    lineSeriesRef.current.setData(chartData);
    lineSeriesRef.current.applyOptions({ color: lineColor });
    baselineSeriesRef.current.setData(baselineData);

    // Fit content to show all data
    chartRef.current?.timeScale().fitContent();
  }, [equityData]);

  if (isLoading) {
    return (
      <div className={className} style={style}>
        <div style={{ height: '150px', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          Loading chart...
        </div>
      </div>
    );
  }

  return (
    <div className={className} style={style}>
      {/* Chart Container */}
      <div 
        ref={chartContainerRef} 
        style={{ 
          width: '100%', 
          height: '150px',
          position: 'relative'
        }} 
      />
    </div>
  );
}
