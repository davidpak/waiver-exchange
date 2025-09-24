import { useAutoAnimate } from '@formkit/auto-animate/react';

/**
 * Simple hook wrapper for Auto-Animate
 * Provides consistent animations throughout the trading platform
 */
export { useAutoAnimate };

// Pre-configured animation presets for different use cases
export const animationPresets = {
  // For lists and grids
  list: {
    duration: 200,
    easing: 'ease-out'
  },
  
  // For cards and containers
  card: {
    duration: 150,
    easing: 'ease-out'
  },
  
  // For buttons and interactive elements
  button: {
    duration: 100,
    easing: 'ease-out'
  },
  
  // For trading data updates
  trading: {
    duration: 300,
    easing: 'ease-in-out'
  }
};
