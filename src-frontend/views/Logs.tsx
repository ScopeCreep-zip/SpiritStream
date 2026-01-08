import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Download, Trash2, ArrowDownToLine } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { LogConsole } from '@/components/feedback/LogConsole';
import { LogEntry } from '@/components/feedback/LogEntry';
import type { LogLevel } from '@/types/stream';
import { useLogStore } from '@/stores/logStore';

export function Logs() {
  const { t } = useTranslation();
  const { logs, filter, autoScroll, setFilter, setAutoScroll, clearLogs } = useLogStore();
  const consoleRef = useRef<HTMLDivElement>(null);

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
    clearLogs();
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
