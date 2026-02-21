'use client';

import { Header } from '@/components/layout/Header';
import { supabase } from '@/lib/supabase';
import { useAuthStore } from '@/stores/authStore';
import {
  Alert,
  Badge,
  Box,
  Button,
  Container,
  Group,
  Loader,
  NumberInput,
  Paper,
  ScrollArea,
  Stack,
  Table,
  Text,
  Title,
} from '@mantine/core';
import {
  IconAlertCircle,
  IconCheck,
  IconClock,
  IconLoader,
  IconLock,
  IconPlayerPlay,
  IconRefresh,
  IconTrash,
} from '@tabler/icons-react';
import { useRouter } from 'next/navigation';
import { useCallback, useEffect, useState } from 'react';

type StepStatus = 'pending' | 'running' | 'done' | 'error';

interface PipelineStep {
  label: string;
  description: string;
  status: StepStatus;
  detail?: string;
}

interface PriceResult {
  player: string;
  priceCents: number;
  percentile: number;
  confidence: number;
}

const INITIAL_STEPS: PipelineStep[] = [
  { label: 'Fetch FantasyCalc', description: 'Trade values from FantasyCalc', status: 'pending' },
  { label: 'Fetch KTC', description: 'Trade values from KeepTradeCut', status: 'pending' },
  { label: 'Fetch Sleeper', description: 'Projections & stats from Sleeper', status: 'pending' },
  { label: 'Sync player metadata', description: 'Build player universe from source data', status: 'pending' },
  { label: 'Auto-match players', description: 'Fuzzy-match source data to players', status: 'pending' },
  { label: 'Calculate prices', description: 'Run pricing engine', status: 'pending' },
];

export default function AdminPage() {
  const router = useRouter();
  const { isAuthenticated, token } = useAuthStore();

  const [authChecking, setAuthChecking] = useState(true);
  const [isAdmin, setIsAdmin] = useState(false);

  const [season, setSeason] = useState<number>(new Date().getFullYear());
  const [week, setWeek] = useState<number>(0);

  const [steps, setSteps] = useState<PipelineStep[]>(INITIAL_STEPS);
  const [isRunning, setIsRunning] = useState(false);
  const [results, setResults] = useState<PriceResult[]>([]);
  const [pipelineError, setPipelineError] = useState<string | null>(null);

  // Data summary
  interface WeekSummary { season: number; week: number; count: number; lastCalculated: string }
  const [dataSummary, setDataSummary] = useState<{
    rpeFairPrices: number;
    sourceValues: number;
    calculatedWeeks: WeekSummary[];
  } | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);

  // Auth guard: redirect if not authenticated
  useEffect(() => {
    if (!isAuthenticated) {
      router.push('/login');
    }
  }, [isAuthenticated, router]);

  // Admin check via Supabase
  useEffect(() => {
    async function checkAdmin() {
      if (!isAuthenticated) return;

      try {
        const { data: { user } } = await supabase.auth.getUser();
        if (!user) {
          setIsAdmin(false);
          setAuthChecking(false);
          return;
        }

        const { data: account } = await supabase
          .from('accounts')
          .select('is_admin')
          .eq('supabase_uid', user.id)
          .single();

        setIsAdmin(account?.is_admin === true);
      } catch {
        setIsAdmin(false);
      } finally {
        setAuthChecking(false);
      }
    }

    checkAdmin();
  }, [isAuthenticated]);

  // Fetch data summary on mount and after pipeline runs
  const fetchSummary = useCallback(async () => {
    try {
      const res = await fetch('/api/admin/reset', {
        headers: { Authorization: `Bearer ${token}` },
      });
      if (res.ok) {
        setDataSummary(await res.json());
      }
    } catch { /* ignore */ }
  }, [token]);

  useEffect(() => {
    if (isAdmin && token) fetchSummary();
  }, [isAdmin, token, fetchSummary]);

  const handleDelete = useCallback(async () => {
    if (!confirm('Delete ALL prices from the system? This cannot be undone.')) return;
    setIsDeleting(true);
    try {
      const res = await fetch('/api/admin/reset', {
        method: 'DELETE',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ full: false }),
      });
      if (res.ok) {
        setResults([]);
        await fetchSummary();
      } else {
        const err = await res.json().catch(() => ({ error: 'Unknown error' }));
        setPipelineError(`Delete failed: ${err.error}`);
      }
    } catch (err: any) {
      setPipelineError(`Delete failed: ${err.message}`);
    } finally {
      setIsDeleting(false);
    }
  }, [token, fetchSummary]);

  const updateStep = useCallback((index: number, update: Partial<PipelineStep>) => {
    setSteps(prev => prev.map((s, i) => (i === index ? { ...s, ...update } : s)));
  }, []);

  const runStep = useCallback(async (
    index: number,
    url: string,
    body?: object
  ): Promise<any> => {
    updateStep(index, { status: 'running', detail: undefined });

    const res = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
      body: body ? JSON.stringify(body) : undefined,
    });

    if (!res.ok) {
      let errMsg: string;
      try {
        const errJson = await res.json();
        // Include debug info if present
        if (errJson.debug) {
          errMsg = `${errJson.error}\n\nDebug: ${JSON.stringify(errJson.debug, null, 2)}`;
        } else {
          errMsg = errJson.error || JSON.stringify(errJson);
        }
      } catch {
        errMsg = await res.text().catch(() => `HTTP ${res.status}`);
      }
      throw new Error(errMsg);
    }

    const data = await res.json();
    const count = data.count ?? data.totalInserted;
    let detail = count != null ? `${count} rows` : 'done';
    // Show extra context for mapping/sync steps
    if (data.debug?.playersInMetadata != null) {
      detail += ` (${data.debug.playersInMetadata} players)`;
    }
    if (data.debug?.sourceValuesLoaded != null) {
      detail += ` | ${data.debug.sourceValuesLoaded} source vals, ${data.debug.mappingsLoaded} mappings`;
    }
    if (data.resolution) {
      detail += ` | ${data.resolution.rate} resolved`;
    }
    if (data.totalInUniverse != null) {
      detail += ` (${data.totalInUniverse} total)`;
    }
    updateStep(index, { status: 'done', detail });
    return data;
  }, [token, updateStep]);

  const runPipeline = useCallback(async () => {
    setIsRunning(true);
    setPipelineError(null);
    setResults([]);
    setSteps(INITIAL_STEPS);

    const endpoints = [
      { url: '/api/admin/fetch/fantasycalc', body: { season, week } },
      { url: '/api/admin/fetch/ktc', body: { season, week } },
      { url: '/api/admin/fetch/sleeper', body: { season, week } },
      { url: '/api/admin/sync-players', body: { season, week } },
      { url: '/api/admin/mapping', body: {} },
      { url: '/api/admin/calculate', body: { season, week } },
    ];

    let currentStep = 0;
    try {
      for (let i = 0; i < endpoints.length; i++) {
        currentStep = i;
        const data = await runStep(i, endpoints[i].url, endpoints[i].body);

        // Capture results from the calculate step (last step)
        if (i === endpoints.length - 1 && data.topPrices) {
          setResults(data.topPrices);
          // Log all debug info for visibility
          if (data.debug) {
            console.log('Calculate debug:', data.debug);
          }
          if (data.resolution) {
            console.log('Resolution:', data.resolution);
          }
          if (data.sources) {
            console.log('Source counts:', data.sources);
          }
        }
      }
    } catch (err: any) {
      updateStep(currentStep, { status: 'error', detail: err.message });
      setPipelineError(err.message);
    } finally {
      setIsRunning(false);
      fetchSummary();
    }
  }, [season, week, runStep, updateStep, fetchSummary]);

  // Loading state
  if (authChecking) {
    return (
      <Box style={{ height: '100vh', display: 'flex', flexDirection: 'column' }}>
        <Header />
        <Container size="sm" py="xl" style={{ flex: 1 }}>
          <Stack align="center" gap="md" mt={80}>
            <Loader color="gold" />
            <Text c="dimmed">Checking permissions...</Text>
          </Stack>
        </Container>
      </Box>
    );
  }

  // Access denied
  if (!isAdmin) {
    return (
      <Box style={{ height: '100vh', display: 'flex', flexDirection: 'column' }}>
        <Header />
        <Container size="sm" py="xl" style={{ flex: 1 }}>
          <Stack align="center" gap="md" mt={80}>
            <IconLock size={48} color="var(--mantine-color-dark-2)" />
            <Title order={2} c="dark.1">Access Denied</Title>
            <Text c="dimmed">You do not have admin privileges.</Text>
            <Button variant="subtle" color="gray" onClick={() => router.push('/')}>
              Back to Dashboard
            </Button>
          </Stack>
        </Container>
      </Box>
    );
  }

  const statusIcon = (status: StepStatus) => {
    switch (status) {
      case 'pending':
        return <IconClock size={16} color="var(--mantine-color-dark-3)" />;
      case 'running':
        return <IconLoader size={16} color="var(--mantine-color-gold-3)" className="spin" />;
      case 'done':
        return <IconCheck size={16} color="var(--color-profit)" />;
      case 'error':
        return <IconAlertCircle size={16} color="var(--mantine-color-red-6)" />;
    }
  };

  const statusColor = (status: StepStatus) => {
    switch (status) {
      case 'pending': return 'gray';
      case 'running': return 'gold';
      case 'done': return 'green';
      case 'error': return 'red';
    }
  };

  return (
    <Box style={{ height: '100vh', display: 'flex', flexDirection: 'column' }}>
      <Header />
      <Box
        style={{
          flex: 1,
          overflow: 'auto',
          minHeight: 0,
          padding: '24px 16px',
        }}
      >
        <Container size="sm">
          <Stack gap="lg">
            {/* Title */}
            <Group justify="space-between" align="center">
              <div>
                <Title order={2} c="dark.0">Pricing Admin</Title>
                <Text size="sm" c="dimmed" mt={4}>
                  Fetch source data, match players, and calculate fair prices.
                </Text>
              </div>
              <Badge color="gold" variant="light" size="lg">Admin</Badge>
            </Group>

            {/* Data summary */}
            {dataSummary && (
              <Paper p="md" withBorder style={{ borderColor: 'var(--border-default)' }}>
                <Group justify="space-between" mb="sm">
                  <Text size="xs" tt="uppercase" fw={500} c="dimmed" style={{ letterSpacing: '0.06em' }}>
                    Current Data
                  </Text>
                  <Button
                    size="compact-xs"
                    variant="subtle"
                    color="red"
                    leftSection={<IconTrash size={12} />}
                    onClick={handleDelete}
                    loading={isDeleting}
                    disabled={isRunning || dataSummary.rpeFairPrices === 0}
                  >
                    Delete All Prices
                  </Button>
                </Group>
                <Group gap="xl" mb={dataSummary.calculatedWeeks.length > 0 ? 'sm' : 0}>
                  <div>
                    <Text size="xl" fw={700} c="dark.0" ff="monospace">{dataSummary.rpeFairPrices}</Text>
                    <Text size="xs" c="dimmed">Active prices</Text>
                  </div>
                  <div>
                    <Text size="xl" fw={700} c="dark.0" ff="monospace">{dataSummary.sourceValues}</Text>
                    <Text size="xs" c="dimmed">Source values</Text>
                  </div>
                  <div>
                    <Text size="xl" fw={700} c="dark.0" ff="monospace">{dataSummary.calculatedWeeks.length}</Text>
                    <Text size="xs" c="dimmed">Weeks calculated</Text>
                  </div>
                </Group>
                {dataSummary.calculatedWeeks.length > 0 && (
                  <Stack gap={4}>
                    {dataSummary.calculatedWeeks.map((w) => (
                      <Group key={`${w.season}-${w.week}`} gap="xs">
                        <Badge size="sm" variant="light" color="gold">
                          {w.season} W{w.week}
                        </Badge>
                        <Text size="xs" c="dimmed">
                          {w.count} prices
                        </Text>
                        <Text size="xs" c="dark.3">
                          {new Date(w.lastCalculated).toLocaleDateString()}
                        </Text>
                      </Group>
                    ))}
                  </Stack>
                )}
              </Paper>
            )}

            {/* Season / Week inputs */}
            <Paper p="md" withBorder style={{ borderColor: 'var(--border-default)' }}>
              <Group grow>
                <NumberInput
                  label="Season"
                  value={season}
                  onChange={(v) => setSeason(typeof v === 'number' ? v : season)}
                  min={2020}
                  max={2030}
                  disabled={isRunning}
                />
                <NumberInput
                  label="Week"
                  value={week}
                  onChange={(v) => setWeek(typeof v === 'number' ? v : week)}
                  min={0}
                  max={18}
                  disabled={isRunning}
                />
              </Group>
            </Paper>

            {/* Run button */}
            <Button
              size="lg"
              color="gold"
              fullWidth
              onClick={runPipeline}
              loading={isRunning}
              leftSection={isRunning ? undefined : <IconPlayerPlay size={18} />}
            >
              {isRunning ? 'Running pipeline...' : 'Update All Prices'}
            </Button>

            {/* Pipeline steps */}
            <Paper p="md" withBorder style={{ borderColor: 'var(--border-default)' }}>
              <Text size="xs" tt="uppercase" fw={500} c="dimmed" mb="sm" style={{ letterSpacing: '0.06em' }}>
                Pipeline Steps
              </Text>
              <Stack gap={0}>
                {steps.map((step, i) => (
                  <Group
                    key={i}
                    gap="sm"
                    py={10}
                    px={4}
                    style={{
                      borderBottom: i < steps.length - 1 ? '1px solid var(--border-subtle)' : undefined,
                    }}
                  >
                    {statusIcon(step.status)}
                    <div style={{ flex: 1 }}>
                      <Text size="sm" fw={500} c="dark.0">{step.label}</Text>
                      <Text size="xs" c="dimmed">{step.description}</Text>
                    </div>
                    <Badge
                      size="sm"
                      variant="light"
                      color={statusColor(step.status)}
                    >
                      {step.status}
                    </Badge>
                    {step.detail && (
                      <Text size="xs" c="dimmed" style={{ maxWidth: 320 }} truncate>
                        {step.detail}
                      </Text>
                    )}
                  </Group>
                ))}
              </Stack>
            </Paper>

            {/* Error alert */}
            {pipelineError && (
              <Alert
                icon={<IconAlertCircle size={16} />}
                color="red"
                variant="light"
                title="Pipeline Error"
                withCloseButton
                onClose={() => setPipelineError(null)}
              >
                <Text size="sm" style={{ whiteSpace: 'pre-wrap', fontFamily: 'monospace' }}>
                  {pipelineError}
                </Text>
              </Alert>
            )}

            {/* Results table */}
            {results.length > 0 && (
              <Paper p="md" withBorder style={{ borderColor: 'var(--border-default)' }}>
                <Group justify="space-between" mb="sm">
                  <Text size="xs" tt="uppercase" fw={500} c="dimmed" style={{ letterSpacing: '0.06em' }}>
                    Top Prices
                  </Text>
                  <Badge size="sm" variant="light" color="gold">
                    {results.length} players
                  </Badge>
                </Group>
                <ScrollArea>
                  <Table highlightOnHover>
                    <Table.Thead>
                      <Table.Tr>
                        <Table.Th>#</Table.Th>
                        <Table.Th>Player</Table.Th>
                        <Table.Th style={{ textAlign: 'right' }}>Price</Table.Th>
                        <Table.Th style={{ textAlign: 'right' }}>Percentile</Table.Th>
                        <Table.Th style={{ textAlign: 'right' }}>Confidence</Table.Th>
                      </Table.Tr>
                    </Table.Thead>
                    <Table.Tbody>
                      {results.map((row, i) => (
                        <Table.Tr key={i}>
                          <Table.Td>
                            <Text size="xs" c="dimmed">{i + 1}</Text>
                          </Table.Td>
                          <Table.Td>
                            <Text size="sm" fw={500} c="dark.0">{row.player}</Text>
                          </Table.Td>
                          <Table.Td style={{ textAlign: 'right' }}>
                            <Text size="sm" fw={600} c="gold.3" ff="monospace">
                              ${(row.priceCents / 100).toFixed(2)}
                            </Text>
                          </Table.Td>
                          <Table.Td style={{ textAlign: 'right' }}>
                            <Text size="sm" ff="monospace">
                              {(row.percentile * 100).toFixed(1)}%
                            </Text>
                          </Table.Td>
                          <Table.Td style={{ textAlign: 'right' }}>
                            <Badge
                              size="sm"
                              variant="light"
                              color={row.confidence >= 0.7 ? 'green' : row.confidence >= 0.4 ? 'yellow' : 'red'}
                            >
                              {(row.confidence * 100).toFixed(0)}%
                            </Badge>
                          </Table.Td>
                        </Table.Tr>
                      ))}
                    </Table.Tbody>
                  </Table>
                </ScrollArea>
              </Paper>
            )}

            {/* Re-run hint */}
            {results.length > 0 && !isRunning && (
              <Button
                variant="subtle"
                color="gray"
                leftSection={<IconRefresh size={14} />}
                onClick={runPipeline}
                size="sm"
              >
                Run again
              </Button>
            )}
          </Stack>
        </Container>
      </Box>

      {/* Spinner animation for running icon */}
      <style>{`
        @keyframes spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
        .spin {
          animation: spin 1s linear infinite;
        }
      `}</style>
    </Box>
  );
}
