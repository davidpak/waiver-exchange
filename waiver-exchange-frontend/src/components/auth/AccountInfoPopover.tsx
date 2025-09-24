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
    Title,
} from '@mantine/core';
import { IconLogout, IconShield, IconUser } from '@tabler/icons-react';
import { useEffect, useRef, useState } from 'react';

interface AccountInfoPopoverProps {
  children: React.ReactNode;
}

export function AccountInfoPopover({ children }: AccountInfoPopoverProps) {
  const { user, accountId, sleeperSetupComplete, sleeperUsername, sleeperLeagueId, logout } = useAuthStore();
  const [opened, setOpened] = useState(false);
  const targetRef = useRef<HTMLDivElement>(null);

  const handleLogout = () => {
    logout();
    setOpened(false);
  };

  // Close popover when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (targetRef.current && !targetRef.current.contains(event.target as Node)) {
        setOpened(false);
      }
    };

    if (opened) {
      document.addEventListener('mousedown', handleClickOutside);
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [opened]);

  return (
    <Popover
      opened={opened}
      onChange={setOpened}
      position="bottom-end"
      withArrow
      shadow="md"
      radius="md"
      width={320}
      withinPortal
    >
      <Popover.Target>
        <Box 
          ref={targetRef}
          style={{ cursor: 'pointer' }}
          onClick={() => setOpened(!opened)}
        >
          {children}
        </Box>
      </Popover.Target>
      
      <Popover.Dropdown>
        <Stack gap="md">
          {/* Header */}
          <Group gap="sm">
            <Avatar size="lg" color="blue">
              <IconUser size={20} />
            </Avatar>
            <Box style={{ flex: 1 }}>
              <Title order={5} size="h6" mb="xs">
                {user?.name || 'User'}
              </Title>
              <Text size="sm" c="dimmed">
                {user?.email || 'user@example.com'}
              </Text>
              {user?.id === 'temp' && (
                <Text size="xs" c="orange">
                  Account details loading...
                </Text>
              )}
            </Box>
          </Group>

          <Divider />

                 {/* Account Info */}
                 <Stack gap="xs">
                   <Text size="sm" fw={500} c="dimmed">
                     Account Information
                   </Text>
                   
                   <Group justify="space-between">
                     <Text size="sm">Account ID:</Text>
                     <Text size="sm" ff="monospace" c="dimmed">
                       {accountId || 'N/A'}
                     </Text>
                   </Group>

                   <Group justify="space-between">
                     <Text size="sm">User ID:</Text>
                     <Text size="sm" ff="monospace" c="dimmed">
                       {user?.id || 'N/A'}
                     </Text>
                   </Group>

                   <Group justify="space-between">
                     <Text size="sm">Email:</Text>
                     <Text size="sm" c="dimmed">
                       {user?.email || 'N/A'}
                     </Text>
                   </Group>

                   <Group justify="space-between">
                     <Text size="sm">Fantasy Points:</Text>
                     <Text size="sm" c="dimmed">
                       {user?.fantasy_points || 'N/A'}
                     </Text>
                   </Group>

                   <Group justify="space-between">
                     <Text size="sm">Balance:</Text>
                     <Text size="sm" c="dimmed">
                       ${user?.currency_balance_dollars ? user.currency_balance_dollars.toFixed(2) : 'N/A'}
                     </Text>
                   </Group>

                   <Group justify="space-between">
                     <Text size="sm">Status:</Text>
                     <Badge 
                       color={sleeperSetupComplete ? 'green' : 'orange'} 
                       variant="light" 
                       size="sm"
                     >
                       {sleeperSetupComplete ? 'Connected' : 'Beta'}
                     </Badge>
                   </Group>
                 </Stack>

          {/* Sleeper Integration */}
          {sleeperSetupComplete && (
            <>
              <Divider />
              <Stack gap="xs">
                <Group gap="xs">
                  <IconShield size={16} />
                  <Text size="sm" fw={500} c="dimmed">
                    Sleeper Integration
                  </Text>
                </Group>
                
                <Group justify="space-between">
                  <Text size="sm">Username:</Text>
                  <Text size="sm" c="dimmed">
                    {sleeperUsername || 'N/A'}
                  </Text>
                </Group>

                <Group justify="space-between">
                  <Text size="sm">League ID:</Text>
                  <Text size="sm" ff="monospace" c="dimmed">
                    {sleeperLeagueId || 'N/A'}
                  </Text>
                </Group>
              </Stack>
            </>
          )}

          <Divider />

          {/* Actions */}
          <Button
            variant="light"
            color="red"
            leftSection={<IconLogout size={16} />}
            onClick={handleLogout}
            fullWidth
          >
            Sign Out
          </Button>
        </Stack>
      </Popover.Dropdown>
    </Popover>
  );
}
