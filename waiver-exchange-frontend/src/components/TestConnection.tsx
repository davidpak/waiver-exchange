'use client';

import { apiClient } from '@/lib/api-client';
import { useAppStore } from '@/stores/useAppStore';
import { Alert, Button, Card, Group, Text } from '@mantine/core';
import { useState } from 'react';

export function TestConnection() {
  const [isTesting, setIsTesting] = useState(false);
  const [testResults, setTestResults] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  
  const { setError: setStoreError } = useAppStore();

  const runTests = async () => {
    setIsTesting(true);
    setTestResults([]);
    setError(null);
    setStoreError(null);

    const results: string[] = [];

    try {
      // Test 1: Symbol Info (Josh Allen - symbol ID 764)
      results.push('Testing Symbol Info API...');
      const symbolInfo = await apiClient.rest.getSymbolInfo(764);
      results.push(`✅ Symbol Info: ${symbolInfo.name} (${symbolInfo.position} - ${symbolInfo.team})`);

      // Test 2: Account Summary
      results.push('Testing Account Summary API...');
      const accountSummary = await apiClient.rest.getAccountSummary(1);
      results.push(`✅ Account Summary: Balance $${(accountSummary.balance / 100).toFixed(2)}`);

      // Test 3: Price History
      results.push('Testing Price History API...');
      const priceHistory = await apiClient.rest.getPriceHistory(764, '1d', '5m');
      results.push(`✅ Price History: ${priceHistory.candles.length} candles`);

      // Test 4: Current Snapshot
      results.push('Testing Current Snapshot API...');
      const snapshot = await apiClient.rest.getCurrentSnapshot();
      results.push(`✅ Snapshot: Tick ${snapshot.tick}, ${Object.keys(snapshot.state.order_books).length} order books`);

      // Test 5: WebSocket Connection
      results.push('Testing WebSocket Connection...');
      try {
        await apiClient.connectWebSocket();
        results.push('✅ WebSocket: Connected successfully');
        
        // Test WebSocket Authentication
        results.push('Testing WebSocket Authentication...');
        const authResponse = await apiClient.ws.authenticate('test_key', 'test_secret');
        results.push(`✅ WebSocket Auth: ${authResponse.authenticated ? 'Authenticated' : 'Failed'}`);
        
        apiClient.disconnectWebSocket();
      } catch (wsError) {
        results.push(`⚠️ WebSocket: ${wsError instanceof Error ? wsError.message : 'Connection failed'}`);
      }

      setTestResults(results);
      results.push('🎉 All tests completed!');

    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Unknown error';
      setError(errorMessage);
      setStoreError(errorMessage);
      results.push(`❌ Error: ${errorMessage}`);
    } finally {
      setIsTesting(false);
    }
  };

  return (
    <Card shadow="sm" padding="lg" radius="md" withBorder>
      <Text size="lg" fw={500} mb="md">
        🔧 Backend Connection Test
      </Text>
      
      <Text size="sm" c="dimmed" mb="md">
        Test all API endpoints to verify backend connectivity
      </Text>

      {error && (
        <Alert color="red" mb="md">
          {error}
        </Alert>
      )}

      <Group mb="md">
        <Button 
          onClick={runTests} 
          loading={isTesting}
          disabled={isTesting}
        >
          {isTesting ? 'Testing...' : 'Run Tests'}
        </Button>
      </Group>

      {testResults.length > 0 && (
        <Card withBorder p="md" bg="gray.0">
          <Text size="sm" fw={500} mb="sm">Test Results:</Text>
          {testResults.map((result, index) => (
            <Text key={index} size="xs" mb="xs" style={{ fontFamily: 'monospace' }}>
              {result}
            </Text>
          ))}
        </Card>
      )}
    </Card>
  );
}
