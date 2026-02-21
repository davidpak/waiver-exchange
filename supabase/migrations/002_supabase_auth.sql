-- Phase 2: Supabase Auth Integration
-- Adds supabase_uid column and creates trigger for auto-linking auth.users to accounts

-- Add supabase_uid column to accounts table
ALTER TABLE accounts ADD COLUMN IF NOT EXISTS supabase_uid UUID UNIQUE;
CREATE INDEX IF NOT EXISTS idx_accounts_supabase_uid ON accounts(supabase_uid);

-- Make google_id nullable for accounts created through Supabase Auth
-- (Supabase Auth handles Google OAuth, google_id comes from raw_user_meta_data)
ALTER TABLE accounts ALTER COLUMN google_id DROP NOT NULL;

-- Function to auto-create/link an account when a new user signs up via Supabase Auth
CREATE OR REPLACE FUNCTION public.handle_new_user()
RETURNS TRIGGER AS $$
BEGIN
  INSERT INTO public.accounts (
    google_id,
    display_name,
    supabase_uid,
    created_at,
    last_updated
  )
  VALUES (
    NEW.raw_user_meta_data->>'sub',
    COALESCE(
      NEW.raw_user_meta_data->>'full_name',
      NEW.raw_user_meta_data->>'name',
      NEW.raw_user_meta_data->>'email'
    ),
    NEW.id,
    NOW(),
    NOW()
  )
  ON CONFLICT (google_id) DO UPDATE SET
    supabase_uid = NEW.id,
    last_updated = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Trigger on auth.users insert
DROP TRIGGER IF EXISTS on_auth_user_created ON auth.users;
CREATE TRIGGER on_auth_user_created
  AFTER INSERT ON auth.users
  FOR EACH ROW
  EXECUTE FUNCTION public.handle_new_user();

-- Helper function to get the current user's account ID from their Supabase JWT
CREATE OR REPLACE FUNCTION public.get_my_account_id()
RETURNS BIGINT AS $$
  SELECT id FROM public.accounts WHERE supabase_uid = auth.uid()
$$ LANGUAGE sql STABLE SECURITY DEFINER;
