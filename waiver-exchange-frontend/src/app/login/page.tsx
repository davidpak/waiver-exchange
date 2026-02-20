'use client';

import placeholderLogo from '@/assets/placeholder-logo.jpg';
import { getWebSocketClient } from '@/lib/websocket-client';
import { useAuthStore } from '@/stores/authStore';
import {
    ActionIcon,
    Alert,
    Badge,
    Button,
    Center,
    Container,
    Loader,
    Paper,
    Stack,
    Stepper,
    Text,
    TextInput,
    Title
} from '@mantine/core';
import { IconBrandGoogle, IconCheck, IconShield, IconUser, IconX } from '@tabler/icons-react';
import Image from 'next/image';
import { useRouter } from 'next/navigation';
import { useEffect, useState } from 'react';

export default function LoginPage() {
  const router = useRouter();
  const {
    setAuth,
    setWebSocketState,
    setSleeperSetup,
    setAvailableLeagues,
    availableLeagues,
  } = useAuthStore();
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string>('Ready to authenticate');
  const [activeStep, setActiveStep] = useState(0);
  const [sleeperUsername, setSleeperUsername] = useState('');
  const [sleeperStep, setSleeperStep] = useState<'username' | 'leagues' | 'complete'>('username');

  // Don't auto-redirect - let user choose when to continue
  // useEffect(() => {
  //   if (isAuthenticated) {
  //     router.push('/');
  //   }
  // }, [isAuthenticated, router]);

  // Check for existing authentication on mount
  useEffect(() => {
    const checkExistingAuth = () => {
      const storedToken = localStorage.getItem('waiver_exchange_token');
      const storedUser = localStorage.getItem('waiver_exchange_user');
      
      if (storedToken && storedUser) {
        try {
          const user = JSON.parse(storedUser);
          setAuth(user, storedToken, user.id);
          setStatus('Found existing authentication');
          // Don't auto-redirect - let user choose when to continue
          // router.push('/dashboard');
        } catch (error) {
          console.error('Error parsing stored user data:', error);
          localStorage.removeItem('waiver_exchange_token');
          localStorage.removeItem('waiver_exchange_user');
        }
      }
    };

    checkExistingAuth();
  }, [setAuth, router]);

  const handleGoogleLogin = async () => {
    setIsLoading(true);
    setError(null);
    setStatus('Opening Google OAuth...');
    setActiveStep(1); // Move to OAuth step

    try {
      // Open OAuth popup window
      const oauthUrl = 'http://localhost:8082/auth/google';
      const oauthWindow = window.open(
        oauthUrl, 
        'oauth', 
        'width=600,height=600,scrollbars=yes,resizable=yes'
      );

      if (!oauthWindow) {
        throw new Error('Popup blocked. Please allow popups for this site.');
      }

               // Listen for OAuth success message from popup
               const messageListener = (event: MessageEvent) => {
                 if (event.data && event.data.type === 'oauth_success') {
                   setStatus('OAuth authentication successful!');
                   setActiveStep(2); // Move to Sleeper integration step
                   
                   // Store the token and user info
                   const { token, user } = event.data;
                   localStorage.setItem('waiver_exchange_token', token);
                   
                   // Use the actual user info from OAuth (includes email!)
                   const oauthUser = {
                     id: 'temp', // Will be replaced with actual user_id from WebSocket
                     name: user?.name || 'User',
                     email: user?.email || 'user@example.com'
                   };
                   localStorage.setItem('waiver_exchange_user', JSON.stringify(oauthUser));
                   
                   // Update auth store with OAuth user info
                   setAuth(oauthUser, token, oauthUser.id);
                   
                   // Remove the message listener
                   window.removeEventListener('message', messageListener);
                   
                   // Clear the window close check since we got the token
                   clearInterval(checkClosed);
                   
                   // Connect to WebSocket and authenticate
                   connectWebSocket(token);
                 }
               };

      // Add message listener
      window.addEventListener('message', messageListener);

      // Listen for OAuth completion (fallback)
      const checkClosed = setInterval(() => {
        if (oauthWindow.closed) {
          clearInterval(checkClosed);
          window.removeEventListener('message', messageListener);
          
          // Only show error if we haven't already received the token
          if (!localStorage.getItem('waiver_exchange_token')) {
            setError('OAuth window was closed. Please try again.');
            setStatus('OAuth cancelled');
            setIsLoading(false);
            setActiveStep(0); // Reset to first step
          }
        }
      }, 1000);

    } catch (error) {
      console.error('OAuth error:', error);
      setError(error instanceof Error ? error.message : 'OAuth failed');
      setStatus('OAuth failed');
      setIsLoading(false);
      setActiveStep(0); // Reset to first step
    }
  };

         const connectWebSocket = async (token: string) => {
           try {
             setStatus('Connecting to trading system...');
             
             const wsClient = getWebSocketClient();
             await wsClient.connect();
             
             setStatus('Authenticating with trading system...');
             
             // Set up message handler to capture the JWT auth response
             const authPromise = new Promise<{ authenticated: boolean; user_id?: string }>((resolve, reject) => {
               // Use the WebSocket client's message handling system
               const messageHandler = (message: any) => {
                 console.log('WebSocket message received:', message);
                 
                   // Handle authentication response - this is where we get the user_id!
                   if (message.id === 'auth_jwt_001' && message.result) {
                     if (message.result.authenticated) {
                       const userId = message.result.user_id;
                       console.log(`Authentication successful! User ID: ${userId}`);
                       
                       // Get existing user info from localStorage (preserves OAuth name/email)
                       const existingUserStr = localStorage.getItem('waiver_exchange_user');
                       let existingUser = { name: 'User', email: 'user@example.com' };
                       if (existingUserStr) {
                         try {
                           existingUser = JSON.parse(existingUserStr);
                         } catch (error) {
                           console.error('Error parsing existing user data:', error);
                         }
                       }
                       
                       // Update auth store with real user ID but preserve OAuth user info
                       const updatedUser = {
                         id: userId,
                         name: existingUser.name || 'User',
                         email: existingUser.email || 'user@example.com'
                       };
                       localStorage.setItem('waiver_exchange_user', JSON.stringify(updatedUser));
                       setAuth(updatedUser, token, userId);
                       
                       setWebSocketState(true, true);
                       resolve({ authenticated: true, user_id: userId });
                     } else {
                       reject(new Error('Authentication failed: ' + (message.result.error || 'Unknown error')));
                     }
                   }
               };
               
               // Register the message handler
               wsClient['messageHandlers'].set('auth_jwt_001', messageHandler);
             });
             
             // Send JWT authentication
             wsClient.send({
               method: 'auth.jwt',
               params: { token },
               id: 'auth_jwt_001'
             });
             
             // Wait for authentication response
             const authResult = await authPromise;
             
             if (authResult.authenticated) {
               setStatus('Authentication complete!');
               setIsLoading(false);
               
               // Now check Sleeper integration status
               checkSleeperIntegrationStatus();
             }
           } catch (error) {
             console.error('WebSocket connection error:', error);
             setError('Failed to connect to trading system');
             setStatus('Connection failed');
             setIsLoading(false);
             setActiveStep(0); // Reset to first step
           }
         };

    const handleContinueAsGuest = () => {
      router.push('/');
    };

    const handleContinueToDashboard = () => {
      // Update the user data with real information from the backend (preserve OAuth info)
      const wsClient = getWebSocketClient();
      wsClient.checkSleeperIntegration().then((response) => {
        if (response.result?.user_id) {
          // Get existing user info from localStorage (preserves OAuth name/email)
          const existingUserStr = localStorage.getItem('waiver_exchange_user');
          let existingUser = { name: 'User', email: 'user@example.com' };
          if (existingUserStr) {
            try {
              existingUser = JSON.parse(existingUserStr);
            } catch (error) {
              console.error('Error parsing existing user data:', error);
            }
          }
          
               const updatedUser = {
                 id: response.result.user_id,
                 name: existingUser.name || 'User',
                 email: existingUser.email || 'user@example.com', // Keep OAuth email, no email from account.info
                 fantasy_points: response.result.fantasy_points,
                 currency_balance_dollars: response.result.currency_balance_dollars
               };
          localStorage.setItem('waiver_exchange_user', JSON.stringify(updatedUser));
          setAuth(updatedUser, localStorage.getItem('waiver_exchange_token') || '', updatedUser.id);
        }
      }).catch(console.error);
      
      router.push('/');
    };

         // Sleeper Integration Functions - matches test gateway exactly
         const checkSleeperIntegrationStatus = async () => {
           try {
             setStatus('Checking Sleeper integration...');
             const wsClient = getWebSocketClient();
             
             // Send account.info request to check sleeper integration status
             const response = await wsClient.sendWithResponse({
               id: 'check_sleeper_001',
               method: 'account.info',
               params: {}
             });
             
             console.log('Sleeper integration check response:', response);
             
             if (response.result?.sleeper_user_id && response.result?.sleeper_league_id) {
               // Sleeper integration is already complete - show option to skip or continue
               setStatus('Sleeper integration found! You can skip setup or reconfigure.');
               setActiveStep(2); // Move to Sleeper step
               setSleeperStep('complete'); // Show completion state
               setIsLoading(false);
               
               // Update user info with data from account.info response (preserve OAuth info)
               if (response.result.user_id) {
                 // Get existing user info from localStorage (preserves OAuth name/email)
                 const existingUserStr = localStorage.getItem('waiver_exchange_user');
                 let existingUser = { name: 'User', email: 'user@example.com' };
                 if (existingUserStr) {
                   try {
                     existingUser = JSON.parse(existingUserStr);
                   } catch (error) {
                     console.error('Error parsing existing user data:', error);
                   }
                 }
                 
                 const updatedUser = {
                   id: response.result.user_id,
                   name: existingUser.name || response.result.name || 'User',
                   email: existingUser.email || 'user@example.com', // Keep OAuth email, no email from account.info
                   fantasy_points: response.result.fantasy_points,
                   currency_balance_dollars: response.result.currency_balance_dollars
                 };
                 localStorage.setItem('waiver_exchange_user', JSON.stringify(updatedUser));
                 setAuth(updatedUser, localStorage.getItem('waiver_exchange_token') || '', updatedUser.id);
               }
               
               // Update Sleeper integration status in auth store
               setSleeperSetup(
                 response.result.sleeper_username || 'Unknown',
                 response.result.sleeper_league_id,
                 response.result.sleeper_roster_id || 'Unknown'
               );
               
               // NO AUTO-REDIRECT - wait for user to click "Continue to Dashboard"
             } else {
               // Need Sleeper setup
               setStatus('Sleeper integration required');
               setActiveStep(2); // Move to Sleeper step
               setSleeperStep('username');
               setIsLoading(false);
             }
           } catch (error) {
             console.error('Error checking Sleeper integration:', error);
             setError('Failed to check Sleeper integration');
             setStatus('Sleeper check failed');
             setIsLoading(false);
             setActiveStep(0);
           }
         };

         const setupSleeperIntegration = async () => {
           if (!sleeperUsername.trim()) {
             setError('Please enter your Sleeper username');
             return;
           }

           try {
             setIsLoading(true);
             setError(null);
             setStatus('Setting up Sleeper integration...');
             
             const wsClient = getWebSocketClient();
             
             // Send setup_sleeper request - matches test gateway exactly
             const response = await wsClient.sendWithResponse({
               id: 'setup_sleeper_001',
               method: 'account.setup_sleeper',
               params: {
                 sleeper_username: sleeperUsername.trim()
               }
             });
             
             console.log('Sleeper setup response:', response);
             
             if (response.result?.success) {
               setStatus('Sleeper integration setup successful!');
               setAvailableLeagues(response.result.leagues);
               setSleeperStep('leagues');
               setIsLoading(false);
             } else {
               setError(response.error?.message || 'Sleeper setup failed');
               setStatus('Sleeper setup failed');
               setIsLoading(false);
             }
           } catch (error) {
             console.error('Error setting up Sleeper integration:', error);
             setError('Failed to setup Sleeper integration');
             setStatus('Sleeper setup failed');
             setIsLoading(false);
           }
         };

    const selectLeague = async (leagueId: string, rosterId: string, leagueName: string) => {
      try {
        setIsLoading(true);
        setError(null);
        setStatus(`Selecting league: ${leagueName}...`);
        
        const wsClient = getWebSocketClient();
        
        // Send select_league request - matches test gateway exactly
        const response = await wsClient.sendWithResponse({
          id: 'select_league_001',
          method: 'account.select_league',
          params: {
            league_id: leagueId,
            roster_id: rosterId
          }
        });
        
        console.log('League selection response:', response);
        
        if (response.result?.success) {
          setStatus('League selected successfully!');
          setSleeperSetup(sleeperUsername, leagueId, rosterId);
          setActiveStep(3); // Complete
          setSleeperStep('complete');
          setIsLoading(false);
          
          // Redirect to main page after a short delay
          setTimeout(() => {
            router.push('/');
          }, 1000);
        } else {
          setError(response.error?.message || 'League selection failed');
          setStatus('League selection failed');
          setIsLoading(false);
        }
      } catch (error) {
        console.error('Error selecting league:', error);
        setError('Failed to select league');
        setStatus('League selection failed');
        setIsLoading(false);
      }
    };

  return (
    <Container size="sm" py="xl">
      <Center>
        <Paper shadow="md" p="xl" radius="md" style={{ width: '100%', maxWidth: 600 }}>
          <Stack gap="xl" style={{ minHeight: '450px' }}>
            {/* Header */}
            <div style={{ textAlign: 'center' }}>
                {/* Placeholder Logo */}
                <ActionIcon
                    size="xl"
                    radius="md"
                    variant="subtle"
                    style={{ 
                    margin: '0 auto 16px auto',
                    display: 'block',
                    width: '64px',
                    height: '64px',
                    borderRadius: '12px',
                    overflow: 'hidden',
                    border: '2px solid var(--mantine-color-default-border)'
                    }}
                >
                    <Image
                        src={placeholderLogo}
                        alt="Waiver Exchange Logo"
                        width={60}
                        height={60}
                        style={{
                            objectFit: 'cover',
                            borderRadius: '8px'
                        }}
                    />
                </ActionIcon>
              
              <Title order={1} size="h2" mb="xs">
                Sign in to The Waiver Exchange
              </Title>
              <Text c="dimmed" size="sm">
                or create an account
              </Text>
              <Badge color="orange" variant="light" mt="xs">
                Beta
              </Badge>
            </div>

            {/* Authentication Stepper */}
            <Stepper 
              active={activeStep} 
              onStepClick={setActiveStep}
              allowNextStepsSelect={false}
              size="sm"
              orientation="horizontal"
              style={{ width: '100%', flex: 1 }}
            >
              <Stepper.Step 
                label="Sign In" 
                description="Choose method"
                icon={<IconUser size={16} />}
              >
                <Stack gap="lg" mt="xs" style={{ minHeight: '200px', justifyContent: 'flex-end', paddingBottom: '45px' }}>
                  {/* Google Sign-In Button - Following Google Branding Guidelines */}
                  <button
                    onClick={handleGoogleLogin}
                    disabled={isLoading}
                    style={{
                      width: '100%',
                      height: '48px',
                      backgroundColor: 'var(--mantine-color-body)',
                      border: '1px solid var(--mantine-color-default-border)',
                      borderRadius: '8px',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      gap: '12px',
                      cursor: isLoading ? 'not-allowed' : 'pointer',
                      opacity: isLoading ? 0.7 : 1,
                      transition: 'all 0.2s ease',
                      fontFamily: 'inherit',
                      fontSize: '14px',
                      fontWeight: 500,
                      color: 'var(--mantine-color-text)',
                      padding: '0 16px',
                    }}
                    onMouseEnter={(e) => {
                      if (!isLoading) {
                        e.currentTarget.style.backgroundColor = 'var(--mantine-color-default-hover)';
                      }
                    }}
                    onMouseLeave={(e) => {
                      if (!isLoading) {
                        e.currentTarget.style.backgroundColor = 'var(--mantine-color-body)';
                      }
                    }}
                  >
                    {/* Google G Logo */}
                    <svg width="20" height="20" viewBox="0 0 24 24">
                      <path
                        fill="#4285F4"
                        d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
                      />
                      <path
                        fill="#34A853"
                        d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
                      />
                      <path
                        fill="#FBBC05"
                        d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
                      />
                      <path
                        fill="#EA4335"
                        d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
                      />
                    </svg>
                    
                    {/* Button Text */}
                    <span>
                      {isLoading ? 'Authenticating...' : 'Sign in with Google'}
                    </span>
                  </button>

                  {/* Continue as Guest Button */}
                  <Button
                    variant="subtle"
                    fullWidth
                    onClick={handleContinueAsGuest}
                    disabled={isLoading}
                  >
                    Continue as Guest
                  </Button>
                </Stack>
              </Stepper.Step>

              <Stepper.Step 
                label="OAuth" 
                description="Google auth"
                icon={<IconBrandGoogle size={16} />}
                loading={activeStep === 1 && isLoading}
              >
                <Stack gap="lg" mt="xl" style={{ minHeight: '200px', justifyContent: 'center' }}>
                  <Alert 
                    icon={isLoading ? <Loader size="sm" /> : <IconCheck size="sm" />}
                    color={error ? 'red' : isLoading ? 'blue' : 'green'}
                    variant="light"
                  >
                    {status}
                  </Alert>
                  
                  {error && (
                    <Alert 
                      icon={<IconX size="sm" />}
                      color="red"
                      variant="light"
                    >
                      {error}
                    </Alert>
                  )}
                </Stack>
              </Stepper.Step>

              <Stepper.Step 
                label="Sleeper" 
                description="League setup"
                icon={<IconShield size={16} />}
                loading={activeStep === 2 && isLoading}
              >
                <Stack gap="lg" mt="xs" style={{ minHeight: '200px', justifyContent: 'flex-end', paddingBottom: '15px' }}>
                  {sleeperStep === 'username' && (
                    <form onSubmit={(e) => {
                      e.preventDefault();
                      if (sleeperUsername.trim() && !isLoading) {
                        setupSleeperIntegration();
                      }
                    }}>
                      <Alert 
                        icon={<IconShield size="sm" />}
                        color="blue"
                        variant="light"
                      >
                        Please enter your Sleeper username to link your fantasy football account
                      </Alert>
                      
                      <TextInput
                        label="Sleeper Username"
                        placeholder="Enter your Sleeper username"
                        value={sleeperUsername}
                        onChange={(event) => setSleeperUsername(event.currentTarget.value)}
                        disabled={isLoading}
                        autoComplete="off"
                      />
                      
                      <Button
                        type="submit"
                        loading={isLoading}
                        disabled={!sleeperUsername.trim()}
                        fullWidth
                      >
                        Setup Sleeper Integration
                      </Button>
                    </form>
                  )}
                  
                  {sleeperStep === 'leagues' && (
                    <>
                      <Alert 
                        icon={<IconCheck size="sm" />}
                        color="green"
                        variant="light"
                      >
                        Select your league to complete the setup
                      </Alert>
                      
                      <Stack gap="md">
                        {availableLeagues?.map((league) => (
                          <Paper
                            key={league.id}
                            p="md"
                            radius="md"
                            style={{ 
                              cursor: 'pointer',
                              border: '1px solid var(--mantine-color-default-border)',
                              transition: 'all 0.2s ease'
                            }}
                            onClick={() => selectLeague(league.id, league.roster_id, league.name)}
                            onMouseEnter={(e) => {
                              e.currentTarget.style.backgroundColor = 'var(--mantine-color-default-hover)';
                            }}
                            onMouseLeave={(e) => {
                              e.currentTarget.style.backgroundColor = 'var(--mantine-color-body)';
                            }}
                          >
                            <Text fw={500}>{league.name}</Text>
                            <Text size="sm" c="dimmed">
                              Season: {league.season} | Roster ID: {league.roster_id}
                            </Text>
                          </Paper>
                        ))}
                      </Stack>
                    </>
                  )}
                  
                  {sleeperStep === 'complete' && (
                    <>
                      <Alert 
                        icon={<IconCheck size="sm" />}
                        color="green"
                        variant="light"
                      >
                        {status}
                      </Alert>
                      
                      <Button
                        onClick={handleContinueToDashboard}
                        fullWidth
                        variant="filled"
                        color="green"
                      >
                        Continue to Dashboard
                      </Button>
                    </>
                  )}
                  
                  {error && (
                    <Alert 
                      icon={<IconX size="sm" />}
                      color="red"
                      variant="light"
                    >
                      {error}
                    </Alert>
                  )}
                </Stack>
              </Stepper.Step>

                     <Stepper.Completed>
                       <Stack gap="lg" mt="xl" style={{ minHeight: '200px', justifyContent: 'center' }}>
                         <Alert 
                           icon={<IconCheck size="sm" />}
                           color="green"
                           variant="light"
                         >
                           {status}
                         </Alert>
                         
                         <Text ta="center" c="dimmed">
                           {status.includes('found') 
                             ? 'Your Sleeper account is already connected. Redirecting to your dashboard...'
                             : 'Welcome to The Waiver Exchange! Redirecting to your dashboard...'
                           }
                         </Text>
                       </Stack>
                     </Stepper.Completed>
            </Stepper>

            {/* Info */}
            <Text size="xs" c="dimmed" ta="center">
              By logging in, you agree to our terms of service and privacy policy.
              <br />
              Your fantasy football data will be used to provide trading services.
            </Text>
          </Stack>
        </Paper>
      </Center>
    </Container>
  );
}
