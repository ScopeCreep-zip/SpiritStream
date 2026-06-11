import React from 'react';
import ReactDOM from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import { isTauri, setBackendBaseUrl } from '@spiritstream/api-client';
import App from './App';
import { ConnectionError } from './components/ui/ConnectionError';
import './lib/i18n'; // Initialize i18n before app renders
// Self-hosted fonts (was Google Fonts at runtime — a per-launch IP
// beacon to a third party, unacceptable for this threat model, and
// broken offline). Weights match the previous <link> exactly.
import '@fontsource/space-grotesk/400.css';
import '@fontsource/space-grotesk/500.css';
import '@fontsource/space-grotesk/600.css';
import '@fontsource/space-grotesk/700.css';
import '@fontsource/jetbrains-mono/400.css';
import '@fontsource/jetbrains-mono/500.css';
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

/**
 * Desktop bootstrap: the backend's port is OS-negotiated per launch, so
 * the shell discovers it from `run/server.port` and hands it over via
 * the `backend_url` command. This MUST complete before anything renders
 * — App's readiness probe and every api-client call read the URL.
 *
 * Returns the failure message when discovery fails. There is no
 * fallback to a guessed port: a guess that happens to hit some OTHER
 * local service would be strictly worse than a loud failure.
 */
async function discoverBackend(): Promise<string | null> {
  if (!isTauri()) return null; // browser builds resolve the URL statically
  try {
    setBackendBaseUrl(await invoke<string>('backend_url'));
    return null;
  } catch (err) {
    return err instanceof Error ? err.message : String(err);
  }
}

function BootFailure({ message }: { message: string }): React.ReactElement {
  return (
    <ConnectionError
      onRetry={() => window.location.reload()}
      details={[message]}
    />
  );
}

const root = ReactDOM.createRoot(document.getElementById('root') as HTMLElement);
discoverBackend().then((bootError) => {
  root.render(
    <React.StrictMode>
      {bootError !== null ? (
        <BootFailure message={bootError} />
      ) : (
        <QueryClientProvider client={queryClient}>
          <App />
        </QueryClientProvider>
      )}
    </React.StrictMode>
  );
});
