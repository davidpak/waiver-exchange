import { AppProviders } from '@/components/providers/AppProviders';
import '@mantine/core/styles.css';
import type { Metadata } from "next";
import { Inter } from 'next/font/google';
import "./globals.css";

const inter = Inter({
  subsets: ['latin'],
  variable: '--font-mono',
  display: 'swap',
});

export const metadata: Metadata = {
  title: "Waiver Exchange",
  description: "Fantasy football player trading platform",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" data-mantine-color-scheme="dark" className={inter.variable}>
      <head />
      <body>
        <AppProviders>
          {children}
        </AppProviders>
      </body>
    </html>
  );
}
