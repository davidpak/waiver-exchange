'use client';

import { Box, Group, Paper, Text } from '@mantine/core';
import { motion } from 'framer-motion';
import { ReactNode } from 'react';

interface WidgetPanelProps {
  title?: string;
  rightSection?: ReactNode;
  children: ReactNode;
  noPadding?: boolean;
  className?: string;
  style?: React.CSSProperties;
}

export function WidgetPanel({
  title,
  rightSection,
  children,
  noPadding,
  className,
  style,
}: WidgetPanelProps) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3, ease: 'easeOut' }}
      style={{ display: 'flex', flexDirection: 'column', ...style }}
      className={className}
    >
      <Paper
        withBorder
        radius="md"
        style={{
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
          flex: 1,
        }}
      >
        {title && (
          <Group
            justify="space-between"
            px="md"
            py="sm"
            style={{
              borderBottom: '1px solid var(--border-subtle)',
              flexShrink: 0,
            }}
          >
            <Text size="xs" fw={600} tt="uppercase" c="dimmed" lts="0.05em">
              {title}
            </Text>
            {rightSection}
          </Group>
        )}
        <Box
          px={noPadding ? 0 : 'md'}
          py={noPadding ? 0 : 'sm'}
          style={{ flex: 1, minHeight: 0, overflow: 'auto' }}
          className="hide-scrollbar"
        >
          {children}
        </Box>
      </Paper>
    </motion.div>
  );
}
