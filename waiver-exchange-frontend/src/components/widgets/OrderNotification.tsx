'use client';

import { Box, Group, Paper, Text } from '@mantine/core';
import { IconCheck, IconX } from '@tabler/icons-react';
import { AnimatePresence, motion } from 'framer-motion';
import { useCallback, useEffect, useState } from 'react';

export interface OrderNotif {
  id: string;
  type: 'success' | 'error';
  message: string;
}

interface OrderNotificationProps {
  notifications: OrderNotif[];
  onDismiss: (id: string) => void;
}

export function OrderNotification({ notifications, onDismiss }: OrderNotificationProps) {
  return (
    <Box
      style={{
        position: 'fixed',
        top: 56,
        right: 16,
        zIndex: 5000,
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
        maxWidth: 360,
      }}
    >
      <AnimatePresence>
        {notifications.map((notif) => (
          <NotifItem key={notif.id} notif={notif} onDismiss={onDismiss} />
        ))}
      </AnimatePresence>
    </Box>
  );
}

function NotifItem({
  notif,
  onDismiss,
}: {
  notif: OrderNotif;
  onDismiss: (id: string) => void;
}) {
  useEffect(() => {
    const timer = setTimeout(() => onDismiss(notif.id), 5000);
    return () => clearTimeout(timer);
  }, [notif.id, onDismiss]);

  const isSuccess = notif.type === 'success';
  const borderColor = isSuccess ? 'var(--color-profit)' : 'var(--color-loss)';
  const Icon = isSuccess ? IconCheck : IconX;

  return (
    <motion.div
      initial={{ opacity: 0, x: 40, scale: 0.95 }}
      animate={{ opacity: 1, x: 0, scale: 1 }}
      exit={{ opacity: 0, x: 40, scale: 0.95 }}
      transition={{ duration: 0.2 }}
    >
      <Paper
        shadow="lg"
        p="sm"
        radius="md"
        style={{
          border: `1px solid ${borderColor}`,
          cursor: 'pointer',
        }}
        onClick={() => onDismiss(notif.id)}
      >
        <Group gap="sm">
          <Icon size={16} color={borderColor} />
          <Text size="sm">
            {notif.message}
          </Text>
        </Group>
      </Paper>
    </motion.div>
  );
}

/** Hook to manage notification state */
export function useOrderNotifications() {
  const [notifications, setNotifications] = useState<OrderNotif[]>([]);

  const addNotification = useCallback((type: 'success' | 'error', message: string) => {
    const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    setNotifications((prev) => [...prev, { id, type, message }]);
  }, []);

  const dismissNotification = useCallback((id: string) => {
    setNotifications((prev) => prev.filter((n) => n.id !== id));
  }, []);

  return { notifications, addNotification, dismissNotification };
}
