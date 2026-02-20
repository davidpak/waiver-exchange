import { createTheme, MantineColorsTuple } from '@mantine/core';

// Gold accent — Binance-inspired
const gold: MantineColorsTuple = [
  '#FFF8E1', // 0: lightest tint
  '#FFECB3', // 1
  '#FFD54F', // 2
  '#F0B90B', // 3: primary accent (Binance gold)
  '#D4A20A', // 4: hover
  '#B88C09', // 5: pressed
  '#9C7608', // 6
  '#7A5C06', // 7
  '#584205', // 8
  '#3D2E03', // 9: darkest shade
];

// Dark palette — Binance-inspired, text deliberately muted
const dark: MantineColorsTuple = [
  '#EAECEF', // 0: primary text (Binance standard — not pure white)
  '#B7BDC6', // 1: secondary text
  '#848E9C', // 2: tertiary / labels
  '#5E6673', // 3: disabled / muted
  '#2B3139', // 4: borders, dividers
  '#1E2329', // 5: elevated surfaces (cards, inputs)
  '#181A20', // 6: panels, sidebars
  '#0B0E11', // 7: body background
  '#090C0F', // 8: header, fixed bars
  '#060708', // 9: deepest
];

export const tradingTheme = createTheme({
  primaryColor: 'gold',
  primaryShade: 3,
  autoContrast: true,

  colors: {
    gold,
    dark,
  },

  fontFamilyMonospace: 'var(--font-mono)',

  defaultRadius: 'md',

  headings: {
    fontFamily: undefined,
    fontWeight: '600',
    sizes: {
      h1: { fontSize: '1.75rem', lineHeight: '1.2' },
      h2: { fontSize: '1.375rem', lineHeight: '1.3' },
      h3: { fontSize: '1.125rem', lineHeight: '1.4' },
      h4: { fontSize: '0.975rem', lineHeight: '1.4' },
    },
  },

  components: {
    Button: {
      defaultProps: {
        radius: 'md',
      },
      styles: {
        root: {
          fontWeight: 600,
          transition: 'all 0.15s ease',
        },
      },
    },

    Paper: {
      defaultProps: {
        radius: 'md',
      },
      styles: {
        root: {
          backgroundColor: 'var(--mantine-color-dark-6)',
        },
      },
    },

    Card: {
      defaultProps: {
        radius: 'md',
      },
      styles: {
        root: {
          backgroundColor: 'var(--mantine-color-dark-6)',
        },
      },
    },

    Modal: {
      defaultProps: {
        radius: 'lg',
      },
      styles: {
        content: {
          backgroundColor: 'var(--mantine-color-dark-6)',
          border: '1px solid var(--border-default)',
          boxShadow: '0 16px 48px rgba(0, 0, 0, 0.5)',
        },
        header: {
          backgroundColor: 'var(--mantine-color-dark-6)',
        },
        overlay: {
          backdropFilter: 'blur(4px)',
        },
      },
    },

    Popover: {
      styles: {
        dropdown: {
          backgroundColor: 'var(--mantine-color-dark-6)',
          borderColor: 'var(--border-default)',
          boxShadow: '0 8px 24px rgba(0, 0, 0, 0.4)',
        },
      },
    },

    Table: {
      styles: {
        table: {
          borderCollapse: 'separate' as const,
          borderSpacing: 0,
        },
        th: {
          fontWeight: 500,
          fontSize: '11px',
          textTransform: 'uppercase' as const,
          letterSpacing: '0.06em',
          padding: '10px 12px',
          color: 'var(--mantine-color-dark-2)',
        },
        td: {
          padding: '10px 12px',
          fontSize: '13px',
        },
        tr: {
          transition: 'background-color 0.1s ease',
        },
      },
    },

    Drawer: {
      styles: {
        content: {
          backgroundColor: 'var(--mantine-color-dark-6)',
        },
        header: {
          backgroundColor: 'var(--mantine-color-dark-6)',
        },
      },
    },

    SegmentedControl: {
      styles: {
        root: {
          backgroundColor: 'var(--mantine-color-dark-5)',
          borderRadius: 'var(--mantine-radius-md)',
        },
      },
    },

    Tabs: {
      styles: {
        tab: {
          fontWeight: 500,
          fontSize: '12px',
          letterSpacing: '0.02em',
          transition: 'color 0.15s ease, border-color 0.15s ease',
        },
      },
    },

    TextInput: {
      defaultProps: {
        radius: 'md',
      },
      styles: {
        input: {
          backgroundColor: 'var(--mantine-color-dark-5)',
          borderColor: 'var(--border-default)',
          transition: 'border-color 0.15s ease, box-shadow 0.15s ease',
          '&:focus': {
            borderColor: 'var(--mantine-color-gold-3)',
            boxShadow: '0 0 0 3px rgba(240, 185, 11, 0.1)',
          },
        },
      },
    },

    NumberInput: {
      defaultProps: {
        radius: 'md',
      },
      styles: {
        input: {
          backgroundColor: 'var(--mantine-color-dark-5)',
          borderColor: 'var(--border-default)',
          transition: 'border-color 0.15s ease, box-shadow 0.15s ease',
        },
      },
    },

    Badge: {
      defaultProps: {
        radius: 'sm',
      },
    },

    Tooltip: {
      defaultProps: {
        color: 'dark.5',
        radius: 'md',
      },
    },

    Skeleton: {
      styles: {
        root: {
          '&::after': {
            background: 'linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.03), transparent)',
          },
        },
      },
    },
  },

  other: {
    headerHeight: '48px',
    tickerHeight: '36px',
  },
});
