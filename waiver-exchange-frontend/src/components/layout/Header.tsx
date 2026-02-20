'use client';

import { AccountInfoPopover } from '@/components/auth/AccountInfoPopover';
import { useTrading } from '@/contexts/TradingContext';
import { useNavigation } from '@/contexts/NavigationContext';
import { useAuthStore } from '@/stores/authStore';
import { useWebSocket } from '@/hooks/useWebSocket';
import {
  Avatar,
  Box,
  Burger,
  Button,
  Drawer,
  Group,
  Kbd,
  NavLink,
  Text,
  Tooltip,
  UnstyledButton,
} from '@mantine/core';
import { useDisclosure, useHotkeys } from '@mantine/hooks';
import {
  IconDashboard,
  IconLayoutList,
  IconSearch,
  IconUser,
  IconWifi,
  IconWifiOff,
} from '@tabler/icons-react';
import { useCallback, useMemo } from 'react';

export function Header() {
  const [drawerOpen, { toggle: toggleDrawer, close: closeDrawer }] = useDisclosure(false);
  const { currentRoute, navigate } = useNavigation();
  const { isAuthenticated } = useAuthStore();
  const { connected } = useWebSocket();
  const { openSearch } = useTrading();

  useHotkeys([['mod+K', openSearch]]);

  const handleNavigation = useCallback(
    (route: string) => {
      navigate(route);
      closeDrawer();
    },
    [navigate, closeDrawer]
  );

  const navItems = useMemo(
    () => [
      { route: 'dashboard', label: 'Dashboard', icon: IconDashboard },
      { route: 'market', label: 'Markets', icon: IconLayoutList },
    ],
    []
  );

  return (
    <>
      <Box
        component="header"
        style={{
          height: 48,
          background: 'rgba(13, 15, 20, 0.80)',
          backdropFilter: 'blur(12px)',
          WebkitBackdropFilter: 'blur(12px)',
          borderBottom: '1px solid var(--border-subtle)',
          display: 'flex',
          alignItems: 'center',
          paddingInline: 16,
          flexShrink: 0,
          zIndex: 100,
        }}
      >
        <Group justify="space-between" style={{ width: '100%' }}>
          {/* Left: burger + logo + nav */}
          <Group gap="md">
            <Box hiddenFrom="md">
              <Burger opened={drawerOpen} onClick={toggleDrawer} size="sm" />
            </Box>

            <UnstyledButton onClick={() => handleNavigation('dashboard')}>
              <Text fw={700} fz={15} c="dark.0">
                Waiver Exchange
              </Text>
            </UnstyledButton>

            <Box
              visibleFrom="md"
              style={{
                width: 1,
                height: 20,
                backgroundColor: 'var(--border-default)',
              }}
            />

            <Box visibleFrom="md">
              <Group gap={4}>
                {navItems.map((item) => (
                  <Button
                    key={item.route}
                    variant={currentRoute === item.route ? 'filled' : 'subtle'}
                    color={currentRoute === item.route ? 'gold' : 'gray'}
                    size="compact-xs"
                    fz={13}
                    onClick={() => handleNavigation(item.route)}
                  >
                    {item.label}
                  </Button>
                ))}
              </Group>
            </Box>
          </Group>

          {/* Right: search + connection + account */}
          <Group gap="sm">
            <Tooltip label="Search players (⌘K)">
              <UnstyledButton
                onClick={openSearch}
                px={8}
                py={4}
                style={{
                  borderRadius: 'var(--mantine-radius-sm)',
                  border: '1px solid var(--border-default)',
                  display: 'flex',
                  alignItems: 'center',
                  gap: 6,
                }}
              >
                <IconSearch size={13} color="var(--mantine-color-dark-2)" />
                <Text fz={12} c="dark.2" visibleFrom="lg">Search</Text>
                <Kbd size="xs" visibleFrom="lg">⌘K</Kbd>
              </UnstyledButton>
            </Tooltip>

            <Tooltip label={connected ? 'Connected' : 'Disconnected'}>
              <Box style={{ display: 'flex', alignItems: 'center' }}>
                {connected ? (
                  <IconWifi size={14} color="var(--color-profit)" />
                ) : (
                  <IconWifiOff size={14} color="var(--mantine-color-dark-2)" />
                )}
              </Box>
            </Tooltip>

            {isAuthenticated ? (
              <AccountInfoPopover>
                <Avatar
                  size={28}
                  radius="sm"
                  color="dark"
                  variant="filled"
                  style={{ cursor: 'pointer' }}
                >
                  <IconUser size={14} />
                </Avatar>
              </AccountInfoPopover>
            ) : (
              <Button
                size="compact-xs"
                variant="filled"
                color="gold"
                fz={12}
                onClick={() => handleNavigation('login')}
              >
                Sign In
              </Button>
            )}
          </Group>
        </Group>
      </Box>

      <Drawer
        opened={drawerOpen}
        onClose={closeDrawer}
        title={<Text fw={700}>Waiver Exchange</Text>}
        size="xs"
      >
        <Box mt="md">
          {navItems.map((item) => (
            <NavLink
              key={item.route}
              active={currentRoute === item.route}
              label={item.label}
              leftSection={<item.icon size={16} />}
              onClick={() => handleNavigation(item.route)}
              color="gold"
              style={{ borderRadius: 'var(--mantine-radius-md)', marginBottom: 4 }}
            />
          ))}
        </Box>
      </Drawer>
    </>
  );
}
