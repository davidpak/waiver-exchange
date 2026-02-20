'use client';

import { useEffect, useRef, useState } from 'react';

interface PriceFlashProps {
  value: number | null | undefined;
  children: React.ReactNode;
  className?: string;
}

/**
 * Wraps children with a brief green/red background flash
 * when the value changes direction (up/down).
 */
export function PriceFlash({ value, children, className }: PriceFlashProps) {
  const prevValue = useRef(value);
  const [flash, setFlash] = useState<'up' | 'down' | null>(null);

  useEffect(() => {
    if (value == null || prevValue.current == null) {
      prevValue.current = value;
      return;
    }

    if (value > prevValue.current) {
      setFlash('up');
    } else if (value < prevValue.current) {
      setFlash('down');
    }

    prevValue.current = value;

    const timer = setTimeout(() => setFlash(null), 300);
    return () => clearTimeout(timer);
  }, [value]);

  const flashClass = flash === 'up' ? 'price-flash-up' : flash === 'down' ? 'price-flash-down' : '';

  return (
    <span className={`${flashClass} ${className || ''}`} style={{ borderRadius: 2 }}>
      {children}
    </span>
  );
}
