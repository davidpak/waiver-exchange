'use client';

import { motion } from 'framer-motion';
import { Loader, Text, Stack, Center } from '@mantine/core';

interface LoadingSpinnerProps {
  size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl';
  message?: string;
  fullScreen?: boolean;
}

/**
 * Professional loading spinner with smooth animations
 * Used throughout the trading platform for loading states
 */
export function LoadingSpinner({ 
  size = 'md', 
  message = 'Loading...', 
  fullScreen = false 
}: LoadingSpinnerProps) {
  const content = (
    <motion.div
      initial={{ opacity: 0, scale: 0.8 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.3, ease: [0.4, 0, 0.2, 1] }}
    >
      <Stack align="center" gap="md">
        <motion.div
          animate={{ rotate: 360 }}
          transition={{
            duration: 1,
            repeat: Infinity,
            ease: 'linear'
          }}
        >
          <Loader size={size} color="blue" />
        </motion.div>
        {message && (
          <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.2, duration: 0.3 }}
          >
            <Text size="sm" c="dimmed">
              {message}
            </Text>
          </motion.div>
        )}
      </Stack>
    </motion.div>
  );

  if (fullScreen) {
    return (
      <Center style={{ height: '100vh', width: '100vw' }}>
        {content}
      </Center>
    );
  }

  return content;
}
