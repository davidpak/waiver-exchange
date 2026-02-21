'use client';

import { useAuthStore } from '@/stores/authStore';
import {
  Avatar,
  Badge,
  Box,
  Button,
  Divider,
  Group,
  Popover,
  Stack,
  Text,
} from '@mantine/core';
import { IconLogout, IconShield, IconUser, IconWifi, IconWifiOff } from '@tabler/icons-react';
import { useEffect, useRef, useState } from 'react';

interface AccountInfoPopoverProps {
  children: React.ReactNode;
}

export function AccountInfoPopover({ children }: AccountInfoPopoverProps) {
  const {
    user,
    accountId,
    sleeperSetupComplete,
    sleeperUsername,
    sleeperLeagueId,
    wsConnected,
    wsAuthenticated,
    logout,
  } = useAuthStore();
  const [opened, setOpened] = useState(false);
  const targetRef = useRef<HTMLDivElement>(null);

  const handleLogout = () => {
    logout();
    setOpened(false);
  };

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (targetRef.current && !targetRef.current.contains(event.target as Node)) {
        setOpened(false);
      }
    };
    if (opened) document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [opened]);

  return (
    <Popover
      opened={opened}
      onChange={setOpened}
      position="bottom-end"
      withArrow
      shadow="lg"
      radius="md"
      width={320}
      withinPortal
    >
      <Popover.Target>
        <Box ref={targetRef} style={{ cursor: 'pointer' }} onClick={() => setOpened(!opened)}>
          {children}
        </Box>
      </Popover.Target>

      <Popover.Dropdown>
        <Stack gap="md">
          {/* Header */}
          <Group gap="sm">
            <Avatar size="lg" radius="xl" src={user?.picture} alt={user?.name} color="dark" variant="filled">
              <IconUser size={20} />
            </Avatar>
            <Box style={{ flex: 1 }}>
              <Text size="sm" fw={600}>
                {user?.name || 'User'}
              </Text>
              <Text size="xs" c="dimmed">{user?.email || 'user@example.com'}</Text>
            </Box>
          </Group>

          <Divider />

          {/* Connection status */}
          <Group gap="xs">
            {wsConnected ? (
              <IconWifi size={14} color="var(--color-profit)" />
            ) : (
              <IconWifiOff size={14} color="var(--mantine-color-dimmed)" />
            )}
            <Text size="xs" c="dimmed">
              {wsConnected
                ? wsAuthenticated
                  ? 'Connected & Authenticated'
                  : 'Connected'
                : 'Disconnected'}
            </Text>
          </Group>

          {/* Account info */}
          <Stack gap="xs">
            <Text size="xs" c="dimmed" tt="uppercase" fw={600} lts="0.05em">
              Account
            </Text>
            <InfoRow label="Account ID" value={accountId || '\u2014'} mono />
            <InfoRow label="User ID" value={user?.id || '\u2014'} mono />
            <InfoRow
              label="Balance"
              value={
                user?.currency_balance_dollars != null
                  ? `$${user.currency_balance_dollars.toFixed(2)}`
                  : '\u2014'
              }
              mono
            />
            <InfoRow
              label="Status"
              value={
                <Badge
                  color={sleeperSetupComplete ? 'green' : 'orange'}
                  variant="light"
                  size="xs"
                >
                  {sleeperSetupComplete ? 'Connected' : 'Beta'}
                </Badge>
              }
            />
          </Stack>

          {/* Sleeper */}
          {sleeperSetupComplete && (
            <>
              <Divider />
              <Stack gap="xs">
                <Group gap={4}>
                  <IconShield size={12} />
                  <Text size="xs" c="dimmed" tt="uppercase" fw={600} lts="0.05em">
                    Sleeper
                  </Text>
                </Group>
                <InfoRow label="Username" value={sleeperUsername || '\u2014'} />
                <InfoRow label="League" value={sleeperLeagueId || '\u2014'} mono />
              </Stack>
            </>
          )}

          <Divider />

          <Button
            variant="light"
            color="red"
            leftSection={<IconLogout size={14} />}
            onClick={handleLogout}
            fullWidth
            size="sm"
          >
            Sign Out
          </Button>
        </Stack>
      </Popover.Dropdown>
    </Popover>
  );
}

function InfoRow({
  label,
  value,
  mono,
}: {
  label: string;
  value: React.ReactNode;
  mono?: boolean;
}) {
  return (
    <Group justify="space-between">
      <Text size="xs" c="dimmed">{label}</Text>
      {typeof value === 'string' ? (
        <Text size="xs" className={mono ? 'mono' : undefined}>
          {value}
        </Text>
      ) : (
        value
      )}
    </Group>
  );
}
