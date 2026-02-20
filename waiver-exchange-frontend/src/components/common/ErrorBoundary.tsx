'use client';

import { Alert, Button, Card, Group, Text, Title } from '@mantine/core';
import { IconAlertCircle, IconRefresh } from '@tabler/icons-react';
import { motion } from 'framer-motion';
import { Component, ErrorInfo, ReactNode } from 'react';

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error?: Error;
  errorInfo?: ErrorInfo;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    this.setState({ error, errorInfo });
    console.error('ErrorBoundary caught an error:', error, errorInfo);
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: undefined, errorInfo: undefined });
  };

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) return this.props.fallback;

      return (
        <motion.div
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ duration: 0.3 }}
        >
          <Card
            shadow="lg"
            padding="xl"
            radius="md"
            withBorder
            style={{
              maxWidth: 500,
              margin: '2rem auto',
              borderColor: 'var(--color-loss)',
            }}
          >
            <Group mb="md">
              <IconAlertCircle size={24} color="var(--color-loss)" />
              <Title order={3} style={{ color: 'var(--color-loss)' }}>
                Something went wrong
              </Title>
            </Group>

            <Text c="dimmed" mb="lg">
              We encountered an unexpected error. This has been logged and our team will investigate.
            </Text>

            {process.env.NODE_ENV === 'development' && this.state.error && (
              <Alert color="red" variant="light" title="Error Details" mb="lg">
                <Text size="sm" className="mono">
                  {this.state.error.message}
                </Text>
                {this.state.errorInfo && (
                  <details style={{ marginTop: '1rem' }}>
                    <summary style={{ cursor: 'pointer' }}>
                      Stack Trace
                    </summary>
                    <pre style={{ fontSize: '0.7rem', overflow: 'auto' }}>
                      {this.state.errorInfo.componentStack}
                    </pre>
                  </details>
                )}
              </Alert>
            )}

            <Group justify="center">
              <Button
                leftSection={<IconRefresh size={16} />}
                onClick={this.handleRetry}
                color="lime"
              >
                Try Again
              </Button>
            </Group>
          </Card>
        </motion.div>
      );
    }

    return this.props.children;
  }
}
