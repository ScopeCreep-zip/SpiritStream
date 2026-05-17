import React from 'react';
import ReactDOM from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import App from './App';
import './lib/i18n'; // Initialize i18n before app renders
import './styles/tokens.css';
import './styles/tokens-high-contrast.css';
import './styles/globals.css';
import { applyHighContrastFromStorage } from './hooks/useHighContrast';

// Apply the persisted high-contrast preference before React renders so the
// very first paint already reflects the user's setting (no FOUC for HC users).
applyHighContrastFromStorage();

// Server-tuned client constants (ranges, encoder presets, encoder metadata)
// hydrate inside `AppContent` after the readiness gate passes — see
// `useHydrateServerConstants` in `App.tsx`. Firing them at module load
// raced server startup and produced the "could not connect to the server"
// cascade in the console before the readiness gate could complete.

// TanStack Query client with sensible defaults
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000, // 30 seconds - consider data fresh
      retry: 1, // Only retry once on failure
    },
  },
});

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>
);
