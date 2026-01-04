import { useState, useEffect, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Download, Trash2, ArrowDownToLine } from 'lucide-react';
import { listen } from '@tauri-apps/api/event';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { LogConsole } from '@/components/feedback/LogConsole';
import { LogEntry } from '@/components/feedback/LogEntry';
import type { LogLevel, LogEntry as LogEntryType } from '@/types/stream';

// Interface for the log event payload from tauri_plugin_log
interface TauriLogRecord {
  level: number;
  message: string;
  target?: string;
}

// Map numeric log levels to our LogLevel type
function mapLogLevel(level: number): LogLevel {
  // log::Level values: Error=1, Warn=2, Info=3, Debug=4, Trace=5
  switch (level) {
    case 1:
      return 'error';
    case 2:
      return 'warn';
    case 3:
      return 'info';
    case 4:
    case 5:
    default:
      return 'debug';
  }
}

let logIdCounter = 1;

export function Logs() {
  const { t } = useTranslation();

  // Initial log entry - created with useMemo to include translation
  const initialLog: LogEntryType = useMemo(
    () => ({
      id: '0',
      timestamp: new Date(),
      level: 'info',
      message: t('logs.initialized'),
    }),
    [t]
  );

  const [logs, setLogs] = useState<LogEntryType[]>([initialLog]);
  const [filter, setFilter] = useState<LogLevel | 'all'>('all');
  const [autoScroll, setAutoScroll] = useState(true);
  const consoleRef = useRef<HTMLDivElement>(null);

  // Listen for log events from Tauri backend
  useEffect(() => {
    const setupListener = async () => {
      // tauri_plugin_log emits on 'log://log' event
      const unlisten = await listen<TauriLogRecord>('log://log', (event) => {
        const { level, message } = event.payload;
        const newLog: LogEntryType = {
          id: String(logIdCounter++),
          timestamp: new Date(),
          level: mapLogLevel(level),
          message,
        };
        setLogs((prev) => [...prev.slice(-999), newLog]); // Keep last 1000 logs
      });

      return unlisten;
    };

    const unlistenPromise = setupListener();

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  // Auto-scroll to bottom when new logs arrive
  useEffect(() => {
    if (autoScroll && consoleRef.current) {
      consoleRef.current.scrollTop = consoleRef.current.scrollHeight;
    }
  }, [logs, autoScroll]);

  // Filter logs based on selected level
  const filteredLogs = filter === 'all' ? logs : logs.filter((log) => log.level === filter);

  // Format timestamp for display
  const formatTime = (date: Date): string => {
    return date.toLocaleTimeString('en-US', {
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  };

  const handleExport = () => {
    const content = logs
      .map((log) => `[${formatTime(log.timestamp)}] [${log.level.toUpperCase()}] ${log.message}`)
      .join('\n');

    const blob = new Blob([content], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `spiritstream-logs-${new Date().toISOString().slice(0, 10)}.txt`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleClear = () => {
    setLogs([]);
  };

  const filterOptions = [
    { value: 'all', label: t('logs.allLevels') },
    { value: 'info', label: t('logs.info') },
    { value: 'warn', label: t('logs.warning') },
    { value: 'error', label: t('logs.error') },
    { value: 'debug', label: t('logs.debug') },
  ];

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>{t('logs.title')}</CardTitle>
          <CardDescription>{t('logs.description')}</CardDescription>
        </div>
        <div className="flex items-center" style={{ gap: '12px' }}>
          <Select
            value={filter}
            onChange={(e) => setFilter(e.target.value as LogLevel | 'all')}
            options={filterOptions}
            className="w-32"
          />
          <Button
            variant={autoScroll ? 'primary' : 'ghost'}
            size="sm"
            onClick={() => setAutoScroll(!autoScroll)}
            title={autoScroll ? t('logs.autoScrollEnabled') : t('logs.autoScrollDisabled')}
          >
            <ArrowDownToLine className="w-4 h-4" />
            {t('logs.autoScroll')}
          </Button>
          <Button variant="ghost" size="sm" onClick={handleExport}>
            <Download className="w-4 h-4" />
            {t('common.export')}
          </Button>
          <Button variant="ghost" size="sm" onClick={handleClear}>
            <Trash2 className="w-4 h-4" />
            {t('common.clear')}
          </Button>
        </div>
      </CardHeader>
      <CardBody style={{ padding: 0 }}>
        <LogConsole maxHeight="500px">
          <div ref={consoleRef}>
            {filteredLogs.length === 0 ? (
              <div
                className="text-center text-[var(--text-secondary)]"
                style={{ padding: '32px 16px' }}
              >
                {t('logs.noLogs')}
              </div>
            ) : (
              filteredLogs.map((log) => (
                <LogEntry
                  key={log.id}
                  time={formatTime(log.timestamp)}
                  level={log.level}
                  message={log.message}
                />
              ))
            )}
          </div>
        </LogConsole>
      </CardBody>
    </Card>
  );
}
