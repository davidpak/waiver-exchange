import { createClient } from '@supabase/supabase-js';
import type { Database } from '@/types/supabase';

const supabaseUrl = process.env.NEXT_PUBLIC_SUPABASE_URL!;
const supabaseServiceRoleKey = process.env.SUPABASE_SERVICE_ROLE_KEY!;

/**
 * Verify that the request comes from an authenticated admin user.
 * Extracts the Supabase JWT from the Authorization header, resolves the user,
 * and checks that is_admin = true on the accounts table.
 *
 * Returns { userId, accountId } on success.
 * Throws a Response with 401/403 on failure.
 */
export async function verifyAdmin(
  request: Request
): Promise<{ userId: string; accountId: number }> {
  const authHeader = request.headers.get('Authorization');
  if (!authHeader?.startsWith('Bearer ')) {
    throw new Response(JSON.stringify({ error: 'Missing or invalid Authorization header' }), {
      status: 401,
      headers: { 'Content-Type': 'application/json' },
    });
  }

  const token = authHeader.slice(7);

  // Create a one-off client with the service role to verify the JWT and look up the user
  const supabase = createClient<Database>(supabaseUrl, supabaseServiceRoleKey, {
    auth: { autoRefreshToken: false, persistSession: false },
  });

  const {
    data: { user },
    error: authError,
  } = await supabase.auth.getUser(token);

  if (authError || !user) {
    throw new Response(JSON.stringify({ error: 'Invalid or expired token' }), {
      status: 401,
      headers: { 'Content-Type': 'application/json' },
    });
  }

  // Look up the account and check is_admin
  const { data: account, error: accountError } = await supabase
    .from('accounts')
    .select('id, is_admin')
    .eq('supabase_uid', user.id)
    .single();

  if (accountError || !account) {
    throw new Response(JSON.stringify({ error: 'Account not found' }), {
      status: 403,
      headers: { 'Content-Type': 'application/json' },
    });
  }

  if (!account.is_admin) {
    throw new Response(JSON.stringify({ error: 'Admin access required' }), {
      status: 403,
      headers: { 'Content-Type': 'application/json' },
    });
  }

  return { userId: user.id, accountId: account.id };
}
