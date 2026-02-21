'use client';

import { supabase } from '@/lib/supabase';
import { useAuthStore } from '@/stores/authStore';
import { Center, Loader, Stack, Text } from '@mantine/core';
import { useRouter } from 'next/navigation';
import { useEffect, useRef } from 'react';

export default function AuthCallbackPage() {
  const router = useRouter();
  const { setAuth } = useAuthStore();
  const handledRef = useRef(false);

  useEffect(() => {
    if (handledRef.current) return;
    handledRef.current = true;

    async function handleCallback() {
      try {
        // Supabase automatically picks up the auth code from the URL hash/query
        const { data, error } = await supabase.auth.getSession();

        if (error) {
          console.error('Auth callback error:', error);
          router.push('/login?error=auth_failed');
          return;
        }

        if (data.session) {
          const user = data.session.user;
          const metadata = user.user_metadata;

          // Fetch the account_id from our accounts table
          const { data: account } = await supabase
            .from('accounts')
            .select('id, fantasy_points, currency_balance')
            .eq('supabase_uid', user.id)
            .single() as { data: { id: number; fantasy_points: number | null; currency_balance: number | null } | null; error: unknown };

          const accountId = account?.id?.toString() ?? user.id;

          setAuth(
            {
              id: accountId,
              name: metadata.full_name ?? metadata.name ?? 'User',
              email: user.email ?? '',
              picture: metadata.avatar_url ?? metadata.picture,
              fantasy_points: account?.fantasy_points ?? undefined,
              currency_balance_dollars: account?.currency_balance
                ? account.currency_balance / 100
                : undefined,
            },
            data.session.access_token,
            accountId
          );

          router.push('/login');
        } else {
          router.push('/login?error=no_session');
        }
      } catch (err) {
        console.error('Auth callback unexpected error:', err);
        router.push('/login?error=unexpected');
      }
    }

    handleCallback();
  }, [router, setAuth]);

  return (
    <Center h="100vh">
      <Stack align="center" gap="md">
        <Loader size="lg" />
        <Text c="dimmed">Completing sign in...</Text>
      </Stack>
    </Center>
  );
}
