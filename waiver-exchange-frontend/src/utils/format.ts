/**
 * Centralized formatting utilities for the trading platform.
 * All monetary values from the API are in cents.
 */

const currencyFormatter = new Intl.NumberFormat('en-US', {
  style: 'currency',
  currency: 'USD',
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

/** Format cents → "$12.34" */
export function formatCents(cents: number | null | undefined): string {
  if (cents == null) return '—';
  return currencyFormatter.format(cents / 100);
}

/** Format P&L cents → "+$12.34" or "-$5.67" */
export function formatPnL(cents: number | null | undefined): string {
  if (cents == null) return '—';
  const abs = Math.abs(cents);
  const formatted = currencyFormatter.format(abs / 100);
  return cents >= 0 ? `+${formatted}` : `-${formatted}`;
}

/** Format percentage → "+1.23%" or "-4.56%" */
export function formatPercentage(value: number | null | undefined): string {
  if (value == null) return '—';
  const sign = value >= 0 ? '+' : '';
  return `${sign}${value.toFixed(2)}%`;
}

/** Return the CSS variable color for a change value */
export function getChangeColor(value: number | null | undefined): string {
  if (value == null || value === 0) return 'var(--mantine-color-dark-2)';
  return value > 0 ? 'var(--color-profit)' : 'var(--color-loss)';
}

/** Return a Mantine color name for a change value */
export function getChangeMantineColor(value: number | null | undefined): string {
  if (value == null || value === 0) return 'dimmed';
  return value > 0 ? 'green' : 'red';
}
