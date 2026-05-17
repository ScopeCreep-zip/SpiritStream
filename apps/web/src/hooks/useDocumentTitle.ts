import { useEffect } from 'react';
import { useStreamStore } from '@/stores/streamStore';

const APP_NAME = 'SpiritStream';
const LIVE_PREFIX = '● LIVE · ';

export function useDocumentTitle(): void {
  const isStreaming = useStreamStore((s) => s.isStreaming);

  useEffect(() => {
    document.title = isStreaming ? `${LIVE_PREFIX}${APP_NAME}` : APP_NAME;
    return () => {
      document.title = APP_NAME;
    };
  }, [isStreaming]);
}
