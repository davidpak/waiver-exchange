import { NavigationLoader } from '@/components/common/NavigationLoader';
import { QueryProvider } from '@/components/providers/QueryProvider';
import { NavigationProvider } from '@/contexts/NavigationContext';
import { tradingTheme } from '@/styles/theme';
import { ColorSchemeScript, MantineProvider } from '@mantine/core';
import '@mantine/core/styles.css';
import type { Metadata } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";
import "./globals.css";

const inter = Inter({
  variable: "--font-inter",
  subsets: ["latin"],
  display: "swap",
});

const jetbrainsMono = JetBrains_Mono({
  variable: "--font-jetbrains-mono",
  subsets: ["latin"],
  display: "swap",
});

export const metadata: Metadata = {
  title: "Waiver Exchange - Fantasy Football Trading Platform",
  description: "Professional fantasy football player trading platform",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" data-mantine-color-scheme="dark">
      <head>
        <ColorSchemeScript defaultColorScheme="dark" />
      </head>
      <body
        className={`${inter.variable} ${jetbrainsMono.variable} antialiased`}
      >
        <MantineProvider 
          theme={tradingTheme} 
          defaultColorScheme="dark"
        >
          <QueryProvider>
            <NavigationProvider>
              {children}
              <NavigationLoader />
            </NavigationProvider>
          </QueryProvider>
        </MantineProvider>
      </body>
    </html>
  );
}
