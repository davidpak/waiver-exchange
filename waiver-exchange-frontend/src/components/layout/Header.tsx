'use client';

import { AccountInfoPopover } from '@/components/auth/AccountInfoPopover';
import { AnimatedButton } from '@/components/ui/AnimatedButton';
import { NavigationButton } from '@/components/ui/NavigationButton';
import { useAuthStore } from '@/stores/authStore';
import { AppShell, Avatar, Badge, Box, Burger, Drawer, Group, NavLink, Text, ThemeIcon, useMantineColorScheme } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { IconBell, IconDashboard, IconList, IconMoon, IconSettings, IconSun, IconUser } from '@tabler/icons-react';
import { useRouter } from 'next/navigation';
import { useCallback, useMemo, useState } from 'react';

interface HeaderProps {
  onNavigate?: (route: string) => void;
  onToggleTheme?: () => void;
}

/**
 * Professional header component with smooth animations and interactions
 * Handles navigation, authentication states, and theme switching
 */
function Header({ 
  onNavigate,
  onToggleTheme 
}: HeaderProps) {
  const { colorScheme, toggleColorScheme } = useMantineColorScheme();
  const [opened, { toggle, close }] = useDisclosure(false);
  const [activeRoute, setActiveRoute] = useState('dashboard');
  const { isAuthenticated } = useAuthStore();
  const router = useRouter();

  const handleThemeToggle = () => {
    toggleColorScheme();
    onToggleTheme?.();
  };

  const handleNavigation = useCallback((route: string) => {
    // Use requestAnimationFrame to defer state update until after current render
    requestAnimationFrame(() => {
      setActiveRoute(route);
    });
    
    // Handle special routes
    if (route === 'login' || route === 'signup') {
      router.push('/login');
      close();
      return;
    }
    
    onNavigate?.(route);
    close(); // Close mobile drawer after navigation
  }, [onNavigate, close, router]);
  
  // Memoize the navigation buttons to prevent re-renders
  const navigationButtons = useMemo(() => (
    <Group gap="xs" ml="xl">
      <NavigationButton
        icon={IconDashboard}
        isActive={activeRoute === 'dashboard'}
        onClick={() => handleNavigation('dashboard')}
      >
        Dashboard
      </NavigationButton>
      <NavigationButton
        icon={IconList}
        isActive={activeRoute === 'market'}
        onClick={() => handleNavigation('market')}
      >
        Market
      </NavigationButton>
    </Group>
  ), [activeRoute, handleNavigation]);

  return (
    <>
      <AppShell.Header
               style={{
                 backgroundColor: 'var(--mantine-color-body)',
                 borderBottom: '1px solid var(--mantine-color-default-border)',
                 backdropFilter: 'blur(10px)',
                 zIndex: 1000,
                 // Prevent layout shifts during re-renders
                 contain: 'layout style',
                 willChange: 'auto',
                 // Force stable layout
                 position: 'relative',
                 overflow: 'hidden'
               }}
      >
        <Group h="100%" px="md" justify="space-between" key="header-main-group">
          {/* Left Section - Mobile Burger + Logo */}
          <Group gap="md">
            {/* Mobile Burger Menu */}
            <Box hiddenFrom="md">
                     <Burger
                       opened={opened}
                       onClick={toggle}
                       size="sm"
                       color="var(--mantine-color-text)"
                     />
            </Box>
            
                   {/* Logo */}
                   <Group gap="sm">
                     <Text size="xl" fw={700} c="var(--mantine-color-text)">
                       The Waiver Exchange
                     </Text>
                     <Badge color="orange" variant="light" size="sm">
                       Beta
                     </Badge>
                   </Group>
            
            {/* Desktop Navigation - Only visible on desktop, right after logo */}
            <Box visibleFrom="md">
              {navigationButtons}
            </Box>
          </Group>
          
          {/* Right Section - Theme Toggle & Auth */}
          <Group gap="sm">
            <ThemeIcon
              variant="subtle"
              color="gray"
              onClick={handleThemeToggle}
              size="lg"
              style={{ 
                cursor: 'pointer',
                transition: 'all 0.2s ease',
                opacity: 0.9
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.opacity = '1';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.opacity = '0.9';
              }}
            >
              {colorScheme === 'dark' ? <IconSun size={20} /> : <IconMoon size={20} />}
            </ThemeIcon>
            
            {isAuthenticated ? (
              // Authenticated User
              <Group gap="sm">
                <Box hiddenFrom="sm">
                  <Avatar 
                    size="md" 
                    color="blue"
                    style={{ 
                      transition: 'all 0.2s ease',
                      opacity: 0.9
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.opacity = '1';
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.opacity = '0.9';
                    }}
                  >
                    <IconUser size={16} />
                  </Avatar>
                </Box>
                
                <Box visibleFrom="sm">
                  <Group gap="sm">
                    <ThemeIcon 
                      radius="md" 
                      color="gray" 
                      size="lg"
                      style={{ 
                        transition: 'all 0.2s ease',
                        opacity: 0.9
                      }}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.opacity = '1';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.opacity = '0.9';
                      }}
                    >
                      <IconBell size={20} />
                    </ThemeIcon>
                    
                    <ThemeIcon 
                      radius="md" 
                      color="gray" 
                      size="lg"
                      style={{ 
                        transition: 'all 0.2s ease',
                        opacity: 0.9
                      }}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.opacity = '1';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.opacity = '0.9';
                      }}
                    >
                      <IconSettings size={20} />
                    </ThemeIcon>
                    
                    <AccountInfoPopover>
                      <Avatar 
                        size="md" 
                        color="blue"
                        style={{ 
                          transition: 'all 0.2s ease',
                          cursor: 'pointer'
                        }}
                      >
                        <IconUser size={16} />
                      </Avatar>
                    </AccountInfoPopover>
                  </Group>
                </Box>
              </Group>
            ) : (
              // Guest User
              <Group gap="sm">
                <Box hiddenFrom="sm">
                  <AnimatedButton
                    variant="primary"
                    size="sm"
                    onClick={() => handleNavigation('login')}
                  >
                    Log In
                  </AnimatedButton>
                </Box>
                
                <Box visibleFrom="sm">
                  <Group gap="sm">
                    <AnimatedButton
                      variant="ghost"
                      size="sm"
                      onClick={() => handleNavigation('login')}
                    >
                      Log In
                    </AnimatedButton>
                    <AnimatedButton
                      variant="primary"
                      size="sm"
                      onClick={() => handleNavigation('signup')}
                    >
                      Sign Up
                    </AnimatedButton>
                  </Group>
                </Box>
              </Group>
            )}
          </Group>
        </Group>
      </AppShell.Header>

      {/* Mobile Drawer */}
      <Drawer
        opened={opened}
        onClose={close}
        title="Navigation"
        size="xs"
               styles={{
                 content: {
                   backgroundColor: 'var(--mantine-color-body)',
                 },
                 header: {
                   backgroundColor: 'var(--mantine-color-body)',
                   borderBottom: '1px solid var(--mantine-color-default-border)',
                 },
                 title: {
                   color: 'var(--mantine-color-text)',
                   fontWeight: 600,
                 },
               }}
      >
        <Box w={240} mt="md">
          {/* Main Navigation */}
              <NavLink
                href="#dashboard"
                active={activeRoute === 'dashboard'}
                label="Dashboard"
                leftSection={<IconDashboard size={16} stroke={1.5} />}
                onClick={() => handleNavigation('dashboard')}
                styles={{
                  root: {
                    borderRadius: '6px',
                    marginBottom: '4px',
                    color: 'var(--mantine-color-text)',
                    '&[data-active]': {
                      backgroundColor: 'var(--mantine-color-blue-1)',
                      color: 'var(--mantine-color-blue-6)',
                    },
                    '&:hover': {
                      backgroundColor: 'var(--mantine-color-default-hover)',
                    },
                  },
                  label: {
                    color: 'inherit',
                    '&[data-active]': {
                      color: 'var(--mantine-color-blue-6)',
                    },
                  },
                }}
              />
          
          <NavLink
            href="#market"
            active={activeRoute === 'market'}
            label="Market"
            leftSection={<IconList size={16} stroke={1.5} />}
            onClick={() => handleNavigation('market')}
            styles={{
              root: {
                borderRadius: '6px',
                marginBottom: '4px',
                color: 'var(--mantine-color-text)',
              },
              label: {
                color: 'var(--mantine-color-text)',
              },
            }}
          />
          
          {/* User Actions */}
          {isAuthenticated ? (
            <>
              <Text size="sm" c="dimmed" mt="lg" mb="sm">Account</Text>
              
              <NavLink
                href="#notifications"
                active={activeRoute === 'notifications'}
                label="Notifications"
                leftSection={<IconBell size={16} stroke={1.5} />}
                onClick={() => handleNavigation('notifications')}
                styles={{
                  root: {
                    borderRadius: '6px',
                    marginBottom: '4px',
                    color: 'var(--mantine-color-text)',
                    '&[data-active]': {
                      backgroundColor: 'var(--mantine-color-blue-1)',
                      color: 'var(--mantine-color-blue-6)',
                    },
                    '&:hover': {
                      backgroundColor: 'var(--mantine-color-default-hover)',
                    },
                  },
                  label: {
                    color: 'inherit',
                    '&[data-active]': {
                      color: 'var(--mantine-color-blue-6)',
                    },
                  },
                }}
              />
              
              <NavLink
                href="#settings"
                active={activeRoute === 'settings'}
                label="Settings"
                leftSection={<IconSettings size={16} stroke={1.5} />}
                onClick={() => handleNavigation('settings')}
                styles={{
                  root: {
                    borderRadius: '6px',
                    marginBottom: '4px',
                    color: 'var(--mantine-color-text)',
                    '&[data-active]': {
                      backgroundColor: 'var(--mantine-color-blue-1)',
                      color: 'var(--mantine-color-blue-6)',
                    },
                    '&:hover': {
                      backgroundColor: 'var(--mantine-color-default-hover)',
                    },
                  },
                  label: {
                    color: 'inherit',
                    '&[data-active]': {
                      color: 'var(--mantine-color-blue-6)',
                    },
                  },
                }}
              />
              
              <NavLink
                href="#profile"
                active={activeRoute === 'profile'}
                label="Profile"
                leftSection={<IconUser size={16} stroke={1.5} />}
                onClick={() => handleNavigation('profile')}
                styles={{
                  root: {
                    borderRadius: '6px',
                    marginBottom: '4px',
                    color: 'var(--mantine-color-text)',
                    '&[data-active]': {
                      backgroundColor: 'var(--mantine-color-blue-1)',
                      color: 'var(--mantine-color-blue-6)',
                    },
                    '&:hover': {
                      backgroundColor: 'var(--mantine-color-default-hover)',
                    },
                  },
                  label: {
                    color: 'inherit',
                    '&[data-active]': {
                      color: 'var(--mantine-color-blue-6)',
                    },
                  },
                }}
              />
            </>
          ) : (
            <>
              <Text size="sm" c="dimmed" mt="lg" mb="sm">Account</Text>
              
              <NavLink
                href="#login"
                active={activeRoute === 'login'}
                label="Log In"
                leftSection={<IconUser size={16} stroke={1.5} />}
                onClick={() => handleNavigation('login')}
                styles={{
                  root: {
                    borderRadius: '6px',
                    marginBottom: '4px',
                    color: 'var(--mantine-color-text)',
                    '&[data-active]': {
                      backgroundColor: 'var(--mantine-color-blue-1)',
                      color: 'var(--mantine-color-blue-6)',
                    },
                    '&:hover': {
                      backgroundColor: 'var(--mantine-color-default-hover)',
                    },
                  },
                  label: {
                    color: 'inherit',
                    '&[data-active]': {
                      color: 'var(--mantine-color-blue-6)',
                    },
                  },
                }}
              />
              
              <NavLink
                href="#signup"
                active={activeRoute === 'signup'}
                label="Sign Up"
                leftSection={<IconUser size={16} stroke={1.5} />}
                onClick={() => handleNavigation('signup')}
                styles={{
                  root: {
                    borderRadius: '6px',
                    marginBottom: '4px',
                    color: 'var(--mantine-color-text)',
                    '&[data-active]': {
                      backgroundColor: 'var(--mantine-color-blue-1)',
                      color: 'var(--mantine-color-blue-6)',
                    },
                    '&:hover': {
                      backgroundColor: 'var(--mantine-color-default-hover)',
                    },
                  },
                  label: {
                    color: 'inherit',
                    '&[data-active]': {
                      color: 'var(--mantine-color-blue-6)',
                    },
                  },
                }}
              />
            </>
          )}
        </Box>
      </Drawer>
    </>
  );
}

export { Header };

