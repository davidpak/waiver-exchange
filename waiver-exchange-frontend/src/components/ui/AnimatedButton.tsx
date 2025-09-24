'use client';

import { Button, ButtonProps } from '@mantine/core';
import { ReactNode } from 'react';

interface AnimatedButtonProps extends ButtonProps {
  children: ReactNode;
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl';
  fullWidth?: boolean;
  loading?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}

/**
 * Professional button component with smooth CSS transitions
 * Used throughout the trading platform for all interactive elements
 */
export function AnimatedButton({
  children,
  variant = 'primary',
  size = 'sm',
  fullWidth = false,
  loading = false,
  disabled = false,
  onClick,
  ...props
}: AnimatedButtonProps) {
  const getVariantProps = () => {
    switch (variant) {
      case 'primary':
        return {
          variant: 'filled' as const,
          color: 'blue',
          radius: 'md'
        };
      case 'secondary':
        return {
          variant: 'subtle' as const,
          color: 'gray',
          radius: 'md'
        };
      case 'ghost':
        return {
          variant: 'light' as const,
          color: 'gray',
          radius: 'md'
        };
      case 'danger':
        return {
          variant: 'filled' as const,
          color: 'red',
          radius: 'md'
        };
      default:
        return {
          variant: 'filled' as const,
          color: 'blue',
          radius: 'md'
        };
    }
  };

  return (
    <Button
      {...getVariantProps()}
      size={size}
      fullWidth={fullWidth}
      loading={loading}
      disabled={disabled}
      onClick={onClick}
      style={{
        cursor: disabled ? 'not-allowed' : 'pointer',
        transition: 'all 0.2s ease',
        opacity: 0.9,
        ...props.style
      }}
      onMouseEnter={(e) => {
        if (!disabled) {
          e.currentTarget.style.opacity = '1';
        }
      }}
      onMouseLeave={(e) => {
        if (!disabled) {
          e.currentTarget.style.opacity = '0.9';
        }
      }}
      {...props}
    >
      {children}
    </Button>
  );
}
