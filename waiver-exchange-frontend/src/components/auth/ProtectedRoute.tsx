'use client';

import { useAuthStore } from '@/stores/authStore';
import { Center, Container, Loader } from '@mantine/core';
import { useRouter } from 'next/navigation';
import { useEffect, useState } from 'react';

interface ProtectedRouteProps {
  children: React.ReactNode;
  requireSleeperSetup?: boolean;
}

export default function ProtectedRoute({ 
  children, 
  requireSleeperSetup = false 
}: ProtectedRouteProps) {
  const router = useRouter();
  const { isAuthenticated, sleeperSetupComplete } = useAuthStore();
  const [isChecking, setIsChecking] = useState(true);

  useEffect(() => {
    const checkAuth = () => {
      // Check localStorage for existing auth
      const storedToken = localStorage.getItem('waiver_exchange_token');
      const storedUser = localStorage.getItem('waiver_exchange_user');
      
      if (!storedToken || !storedUser) {
        router.push('/login');
        return;
      }

      // If we have stored auth but store is not authenticated, restore it
      if (!isAuthenticated) {
        try {
          const user = JSON.parse(storedUser);
          const { setAuth } = useAuthStore.getState();
          setAuth(user, storedToken, user.id);
        } catch (error) {
          console.error('Error restoring auth state:', error);
          localStorage.removeItem('waiver_exchange_token');
          localStorage.removeItem('waiver_exchange_user');
          router.push('/login');
          return;
        }
      }

      // Check if Sleeper setup is required
      if (requireSleeperSetup && !sleeperSetupComplete) {
        // For now, we'll allow access and handle Sleeper setup in the component
        // In the future, we might redirect to a Sleeper setup page
      }

      setIsChecking(false);
    };

    checkAuth();
  }, [isAuthenticated, sleeperSetupComplete, requireSleeperSetup, router]);

  if (isChecking) {
    return (
      <Container size="sm" py="xl">
        <Center>
          <Loader size="lg" />
        </Center>
      </Container>
    );
  }

  return <>{children}</>;
}
