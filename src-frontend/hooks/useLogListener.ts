import { useEffect, useRef } from 'react';
import { api, events } from '@/lib/backend';
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
      const unlisten = await events.on<TauriLogRecord>('log://log', (payload) => {
        const { level, message } = payload;
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
