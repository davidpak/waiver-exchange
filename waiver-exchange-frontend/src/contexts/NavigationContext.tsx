'use client';

import { usePathname, useRouter } from 'next/navigation';
import { createContext, ReactNode, useCallback, useContext, useEffect, useState } from 'react';

interface NavigationContextType {
  currentRoute: string;
  setCurrentRoute: (route: string) => void;
  navigate: (route: string) => void;
  isNavigating: boolean;
  progress: number;
}

const NavigationContext = createContext<NavigationContextType | undefined>(undefined);

interface NavigationProviderProps {
  children: ReactNode;
}

export function NavigationProvider({ children }: NavigationProviderProps) {
  const [currentRoute, setCurrentRoute] = useState('dashboard');
  const [isNavigating, setIsNavigating] = useState(false);
  const [isFirstVisit, setIsFirstVisit] = useState(true);
  const [progress, setProgress] = useState(0);
  const router = useRouter();
  const pathname = usePathname();

  // Handle first visit loading with progress
  useEffect(() => {
    if (isFirstVisit) {
      setIsNavigating(true);
      setProgress(0);
      
      // Simulate progress for first visit
      const progressInterval = setInterval(() => {
        setProgress(prev => {
          if (prev >= 90) {
            clearInterval(progressInterval);
            return 90;
          }
          return prev + Math.random() * 15;
        });
      }, 100);
      
      setTimeout(() => {
        setProgress(100);
        setTimeout(() => {
          setIsNavigating(false);
          setIsFirstVisit(false);
          setProgress(0);
        }, 200);
      }, 1000);
    }
  }, [isFirstVisit]);

  // Update current route based on pathname
  useEffect(() => {
    if (pathname === '/market') {
      setCurrentRoute('market');
    } else if (pathname === '/') {
      setCurrentRoute('dashboard');
    }
  }, [pathname]);

  // Stop navigating when pathname changes (page actually loaded)
  useEffect(() => {
    if (isNavigating) {
      // Complete progress and hide loader
      setProgress(100);
      setTimeout(() => {
        setIsNavigating(false);
        setProgress(0);
      }, 200);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only trigger on pathname change, not isNavigating
  }, [pathname]);

  const navigate = useCallback((route: string) => {
    setIsNavigating(true);
    setCurrentRoute(route);
    setProgress(0);
    
    // Simulate progress for navigation
    const progressInterval = setInterval(() => {
      setProgress(prev => {
        if (prev >= 80) {
          clearInterval(progressInterval);
          return 80;
        }
        return prev + Math.random() * 20;
      });
    }, 50);
    
    // Handle page navigation
    if (route === 'market') {
      router.push('/market');
    } else if (route === 'dashboard') {
      router.push('/');
    } else if (route === 'login' || route === 'signup') {
      router.push('/login');
    }
    
    // Loading state will be cleared when pathname changes (handled by useEffect above)
  }, [router]);

  return (
    <NavigationContext.Provider value={{
      currentRoute,
      setCurrentRoute,
      navigate,
      isNavigating,
      progress
    }}>
      {children}
    </NavigationContext.Provider>
  );
}

export function useNavigation() {
  const context = useContext(NavigationContext);
  if (context === undefined) {
    throw new Error('useNavigation must be used within a NavigationProvider');
  }
  return context;
}
