'use client';

import { useNavigation } from '@/contexts/NavigationContext';
import { Box } from '@mantine/core';

export function NavigationLoader() {
  const { isNavigating, progress } = useNavigation();

  if (!isNavigating) return null;

  return (
    <Box
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        height: '2px',
        zIndex: 9999,
        backgroundColor: 'rgba(204, 255, 0, 0.15)',
        overflow: 'hidden',
      }}
    >
      <Box
        style={{
          height: '100%',
          width: `${Math.min(progress, 100)}%`,
          backgroundColor: 'var(--accent-primary)',
          transition: 'width 0.2s ease',
          boxShadow: '0 0 8px var(--accent-primary)',
        }}
      />
    </Box>
  );
}
