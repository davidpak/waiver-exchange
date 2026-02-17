'use client';

import { useNavigation } from '@/contexts/NavigationContext';
import { Box, Loader, Progress, Stack, Text } from '@mantine/core';

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
        bottom: 0,
        backgroundColor: 'var(--site-bg)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 9999,
      }}
    >
      <Stack align="center" gap="lg" style={{ width: '300px' }}>
        <Loader type="bars" size="xl" color="var(--text-primary)" />
        <Progress
          value={progress}
          size="lg"
          radius="md"
          color="blue"
          transitionDuration={200}
          style={{ width: '100%' }}
        />
        <Text size="sm" c="dimmed" ta="center">
          {progress < 100 ? `Loading... ${Math.round(progress)}%` : 'Complete!'}
        </Text>
      </Stack>
    </Box>
  );
}
