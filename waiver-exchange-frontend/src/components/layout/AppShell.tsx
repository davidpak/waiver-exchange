'use client';

import { AppShell, Avatar, Burger, Group, Menu, Text, Title, UnstyledButton, rem } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { IconChevronDown, IconLogout, IconSettings, IconUser } from '@tabler/icons-react';
import { useState } from 'react';

interface AppShellLayoutProps {
  children: React.ReactNode;
}

export function AppShellLayout({ children }: AppShellLayoutProps) {
  const [opened, { toggle }] = useDisclosure();
  const [userMenuOpened, setUserMenuOpened] = useState(false);

  return (
    <AppShell
      header={{ height: 60 }}
      navbar={{
        width: 300,
        breakpoint: 'sm',
        collapsed: { mobile: !opened },
      }}
      padding="md"
    >
      <AppShell.Header>
        <Group h="100%" px="md" justify="space-between">
          <Group>
            <Burger opened={opened} onClick={toggle} hiddenFrom="sm" size="sm" />
            <Title order={3} c="blue">
              🏈 Waiver Exchange
            </Title>
          </Group>

          <Group>
            <Text size="sm" c="dimmed">
              Live Market Data
            </Text>
            <div style={{ width: 8, height: 8, borderRadius: '50%', backgroundColor: '#51cf66' }} />
          </Group>

          <Menu
            width={260}
            position="bottom-end"
            transitionProps={{ transition: 'pop-top-right' }}
            onClose={() => setUserMenuOpened(false)}
            onOpen={() => setUserMenuOpened(true)}
            withinPortal
          >
            <Menu.Target>
              <UnstyledButton
                style={{
                  padding: 'var(--mantine-spacing-xs)',
                  borderRadius: 'var(--mantine-radius-sm)',
                  color: 'var(--mantine-color-text)',
                  '&:hover': {
                  backgroundColor: 'var(--mantine-color-default-hover)',
                  },
                }}
              >
                <Group gap={7}>
                  <Avatar
                    src="https://raw.githubusercontent.com/mantinedev/mantine/master/.demo/avatars/avatar-5.png"
                    alt="User"
                    radius="xl"
                    size={20}
                  />
                  <Text fw={500} size="sm" lh={1} mr={3}>
                    Trader
                  </Text>
                  <IconChevronDown style={{ width: rem(12), height: rem(12) }} stroke={1.5} />
                </Group>
              </UnstyledButton>
            </Menu.Target>
            <Menu.Dropdown>
              <Menu.Item
                leftSection={
                  <IconUser style={{ width: rem(16), height: rem(16) }} stroke={1.5} />
                }
              >
                Your profile
              </Menu.Item>
              <Menu.Item
                leftSection={
                  <IconSettings style={{ width: rem(16), height: rem(16) }} stroke={1.5} />
                }
              >
                Account settings
              </Menu.Item>
              <Menu.Divider />
              <Menu.Item
                leftSection={
                  <IconLogout style={{ width: rem(16), height: rem(16) }} stroke={1.5} />
                }
              >
                Logout
              </Menu.Item>
            </Menu.Dropdown>
          </Menu>
        </Group>
      </AppShell.Header>

      <AppShell.Navbar p="md">
        <Text size="sm" fw={500} mb="md" c="dimmed">
          TRADING
        </Text>
        {/* Navigation items will go here */}
      </AppShell.Navbar>

      <AppShell.Main>
        {children}
      </AppShell.Main>
    </AppShell>
  );
}
