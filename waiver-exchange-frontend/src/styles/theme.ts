import { createTheme, MantineColorsTuple } from '@mantine/core';

// Custom color palette for trading platform
const tradingBlue: MantineColorsTuple = [
  '#e7f5ff',
  '#d0ebff',
  '#a5d8ff',
  '#74c0fc',
  '#339af0',
  '#228be6',
  '#1c7ed6',
  '#1971c2',
  '#1864ab',
  '#0c5aa6'
];

const tradingGreen: MantineColorsTuple = [
  '#ebfbee',
  '#d3f9d8',
  '#b2f2bb',
  '#8ce99a',
  '#69db7c',
  '#51cf66',
  '#40c057',
  '#37b24d',
  '#2f9e44',
  '#2b8a3e'
];

const tradingRed: MantineColorsTuple = [
  '#ffe3e3',
  '#ffc9c9',
  '#ffa8a8',
  '#ff8787',
  '#ff6b6b',
  '#fa5252',
  '#f03e3e',
  '#e03131',
  '#c92a2a',
  '#a61e1e'
];

/**
 * Professional theme configuration for the trading platform
 * Supports both dark and light modes with proper contrast and accessibility
 */
export const tradingTheme = createTheme({
  /** Primary color scheme */
  primaryColor: 'blue',
  
  /** Custom color palette */
  colors: {
    tradingBlue,
    tradingGreen,
    tradingRed,
  },
  
  
  /** Font configuration */
  fontFamily: 'Inter, -apple-system, BlinkMacSystemFont, Segoe UI, Roboto, sans-serif',
  fontFamilyMonospace: 'JetBrains Mono, Monaco, Consolas, monospace',
  
  /** Default radius for components */
  defaultRadius: 'md',
  
  /** Headings configuration */
  headings: {
    fontFamily: 'Inter, -apple-system, BlinkMacSystemFont, Segoe UI, Roboto, sans-serif',
    fontWeight: '600',
    sizes: {
      h1: { fontSize: '2.5rem', lineHeight: '1.2' },
      h2: { fontSize: '2rem', lineHeight: '1.3' },
      h3: { fontSize: '1.5rem', lineHeight: '1.4' },
      h4: { fontSize: '1.25rem', lineHeight: '1.4' },
      h5: { fontSize: '1.125rem', lineHeight: '1.4' },
      h6: { fontSize: '1rem', lineHeight: '1.4' },
    },
  },
  
  /** Component default props */
  components: {
    Button: {
      defaultProps: {
        radius: 'md',
      },
    },
    
    Card: {
      defaultProps: {
        radius: 'md',
        shadow: 'sm',
      },
    },
    
    TextInput: {
      defaultProps: {
        radius: 'md',
      },
    },
    
    Select: {
      defaultProps: {
        radius: 'md',
      },
    },
    
    Modal: {
      defaultProps: {
        radius: 'md',
        shadow: 'xl',
      },
    }
  },
  
  /** Dark theme overrides */
  other: {
    // Custom CSS variables for animations and effects
    animationDuration: '0.2s',
    animationEasing: 'cubic-bezier(0.4, 0, 0.2, 1)',
    
    // Trading-specific colors
    priceUp: 'var(--mantine-color-trading-green-6)',
    priceDown: 'var(--mantine-color-trading-red-6)',
    priceNeutral: 'var(--mantine-color-gray-6)',
    
    // Layout spacing
    headerHeight: '60px',
    sidebarWidth: '280px',
    
    // Z-index layers
    zIndex: {
      header: 1000,
      modal: 2000,
      tooltip: 3000,
      notification: 4000,
    },
  },
});
