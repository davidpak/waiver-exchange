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

/**
 * Professional error boundary component with smooth animations
 * Provides graceful error handling throughout the trading platform
 */
export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    this.setState({
      error,
      errorInfo
    });

    // Log error to monitoring service in production
    console.error('ErrorBoundary caught an error:', error, errorInfo);
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: undefined, errorInfo: undefined });
  };

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <motion.div
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ duration: 0.3, ease: [0.4, 0, 0.2, 1] }}
        >
          <Card
            shadow="lg"
            padding="xl"
            radius="md"
            withBorder
            style={{
              maxWidth: 500,
              margin: '2rem auto',
              backgroundColor: 'var(--mantine-color-body)',
              borderColor: 'var(--mantine-color-red-6)',
            }}
          >
            <Group mb="md">
              <IconAlertCircle size={24} color="var(--mantine-color-red-6)" />
              <Title order={3} c="red">
                Something went wrong
              </Title>
            </Group>

            <Text c="dimmed" mb="lg">
              We encountered an unexpected error. This has been logged and our team will investigate.
            </Text>

            {process.env.NODE_ENV === 'development' && this.state.error && (
              <Alert
                color="red"
                variant="light"
                title="Error Details (Development)"
                mb="lg"
              >
                <Text size="sm" style={{ fontFamily: 'monospace' }}>
                  {this.state.error.message}
                </Text>
                {this.state.errorInfo && (
                  <details style={{ marginTop: '1rem' }}>
                    <summary>Stack Trace</summary>
                    <pre style={{ fontSize: '0.75rem', overflow: 'auto' }}>
                      {this.state.errorInfo.componentStack}
                    </pre>
                  </details>
                )}
              </Alert>
            )}

            <Group justify="center">
              <motion.div
                whileHover={{ scale: 1.05 }}
                whileTap={{ scale: 0.95 }}
              >
                <Button
                  leftSection={<IconRefresh size={16} />}
                  onClick={this.handleRetry}
                  variant="filled"
                  color="blue"
                >
                  Try Again
                </Button>
              </motion.div>
            </Group>
          </Card>
        </motion.div>
      );
    }

    return this.props.children;
  }
}
