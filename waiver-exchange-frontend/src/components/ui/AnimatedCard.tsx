'use client';

import { Card, CardProps } from '@mantine/core';
import { ReactNode } from 'react';

interface AnimatedCardProps extends CardProps {
  children: ReactNode;
  hoverable?: boolean;
  interactive?: boolean;
  onClick?: () => void;
}

/**
 * Professional card component with smooth hover effects
 * Used for all card-based layouts in the trading platform
 */
export function AnimatedCard({
  children,
  hoverable = true,
  interactive = false,
  onClick,
  ...props
}: AnimatedCardProps) {
  return (
    <Card
      shadow="sm"
      padding="lg"
      radius="md"
      withBorder
             style={{
               cursor: interactive ? 'pointer' : 'default',
               transition: 'all 0.2s ease',
               backgroundColor: 'var(--mantine-color-body)',
               borderColor: 'var(--mantine-color-default-border)',
               boxShadow: '0 1px 3px rgba(0, 0, 0, 0.1)',
               ...props.style
             }}
      onClick={onClick}
      onMouseEnter={(e) => {
        if (hoverable) {
          e.currentTarget.style.borderColor = 'var(--mantine-color-blue-6)';
          e.currentTarget.style.boxShadow = '0 4px 12px rgba(0, 0, 0, 0.15)';
        }
      }}
             onMouseLeave={(e) => {
               if (hoverable) {
                 e.currentTarget.style.borderColor = 'var(--mantine-color-default-border)';
                 e.currentTarget.style.boxShadow = '0 1px 3px rgba(0, 0, 0, 0.1)';
               }
             }}
      {...props}
    >
      {children}
    </Card>
  );
}
