import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { api } from '@/lib/tauri';
import { useLogStore } from '@/stores/logStore';
import { createLogEntry, mapLogLevelFromNumber, parseLogLine } from '@/lib/logging';

interface TauriLogRecord {
  level: number;
  message: string;
  target?: string;
}

export function useLogListener() {
  const { addLog, addLogs } = useLogStore();
  const loadedRef = useRef(false);

  useEffect(() => {
    let isActive = true;

    const loadRecentLogs = async () => {
      if (loadedRef.current) return;
      loadedRef.current = true;
      try {
        const lines = await api.system.getRecentLogs(500);
        if (!isActive || lines.length === 0) return;
        const entries = lines.map(parseLogLine);
        addLogs(entries);
      } catch (error) {
        console.error('Failed to load recent logs:', error);
      }
    };

    const setupListener = async () => {
      const unlisten = await listen<TauriLogRecord>('log://log', (event) => {
        const { level, message } = event.payload;
        addLog(createLogEntry(mapLogLevelFromNumber(level), message));
      });

      return unlisten;
    };

    loadRecentLogs();
    const unlistenPromise = setupListener();

    return () => {
      isActive = false;
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [addLog, addLogs]);
}
