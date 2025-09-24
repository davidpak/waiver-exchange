'use client';

import { Button, ButtonProps } from '@mantine/core';
import { Icon } from '@tabler/icons-react';
import { ReactNode } from 'react';

interface NavigationButtonProps extends Omit<ButtonProps, 'leftSection'> {
  children: ReactNode;
  icon: Icon;
  isActive?: boolean;
  onClick?: () => void;
}

/**
 * Professional navigation button with smooth CSS transitions
 * Used in header navigation and sidebar menus
 */
export function NavigationButton({
  children,
  icon: Icon,
  isActive = false,
  onClick,
  ...props
}: NavigationButtonProps) {
  return (
    <Button
      variant="subtle"
      color={isActive ? 'blue' : 'gray'}
      leftSection={<Icon size={16} />}
      size="sm"
      fw={500}
      onClick={onClick}
      styles={{
        root: {
          opacity: isActive ? 1 : 0.7,
          transition: 'all 0.2s ease',
          color: 'var(--mantine-color-text)',
          '&:hover': {
            opacity: '1',
            backgroundColor: 'var(--mantine-color-default-hover)',
          },
        },
        label: {
          color: 'inherit',
        },
      }}
      {...props}
    >
      {children}
    </Button>
  );
}
