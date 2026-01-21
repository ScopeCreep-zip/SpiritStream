import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Download, Trash2, ArrowDownToLine } from 'lucide-react';
import { save } from '@tauri-apps/plugin-dialog';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { LogConsole } from '@/components/feedback/LogConsole';
import { LogEntry } from '@/components/feedback/LogEntry';
import type { LogLevel } from '@/types/stream';
import { useLogStore } from '@/stores/logStore';
import { api } from '@/lib/tauri';
import { toast } from '@/hooks/useToast';

export function Logs() {
  const { t, i18n } = useTranslation();
  const {
    logs,
    filter,
    autoScroll,
    timeFilter,
    setFilter,
    setAutoScroll,
    setTimeFilter,
    clearLogs,
  } = useLogStore();
  const consoleRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when new logs arrive
  useEffect(() => {
    if (autoScroll && consoleRef.current) {
      consoleRef.current.scrollTop = consoleRef.current.scrollHeight;
    }
  }, [logs, autoScroll]);

  const applyTimeFilter = (entries: typeof logs) => {
    if (timeFilter === 'all') {
      return entries;
    }
    const now = Date.now();
    const cutoffMs = (() => {
      switch (timeFilter) {
        case '15m':
          return 15 * 60 * 1000;
        case '1h':
          return 60 * 60 * 1000;
        case '24h':
          return 24 * 60 * 60 * 1000;
        case '7d':
          return 7 * 24 * 60 * 60 * 1000;
        default:
          return 0;
      }
    })();

    return entries.filter((log) => now - log.timestamp.getTime() <= cutoffMs);
  };

  const timeFilteredLogs = applyTimeFilter(logs);
  const filteredLogs =
    filter === 'all' ? timeFilteredLogs : timeFilteredLogs.filter((log) => log.level === filter);

  // Format timestamp for display
  const formatTime = (date: Date): string => {
    const locale = i18n.resolvedLanguage || i18n.language || 'en';
    return date.toLocaleTimeString(locale, {
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  };

  const handleExport = async () => {
    const content = filteredLogs
      .map((log) => `[${formatTime(log.timestamp)}] [${log.level.toUpperCase()}] ${log.message}`)
      .join('\n');

    const defaultName = `spiritstream-logs-${new Date().toISOString().slice(0, 10)}.txt`;
    const selected = await save({
      title: t('logs.exportTitle', { defaultValue: 'Export Logs' }),
      defaultPath: defaultName,
      filters: [{ name: t('logs.logFile'), extensions: ['txt', 'log'] }],
    });
    if (!selected) {
      return;
    }

    try {
      await api.system.exportLogs(selected, content);
      toast.success(t('logs.exportSuccess', { defaultValue: 'Logs exported' }));
    } catch (error) {
      console.error('Failed to export logs:', error);
      toast.error(t('logs.exportFailed', { defaultValue: 'Failed to export logs' }));
    }
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
  const timeOptions = [
    { value: 'all', label: t('logs.allTime', { defaultValue: 'All time' }) },
    { value: '15m', label: t('logs.last15Minutes', { defaultValue: 'Last 15 minutes' }) },
    { value: '1h', label: t('logs.lastHour', { defaultValue: 'Last hour' }) },
    { value: '24h', label: t('logs.last24Hours', { defaultValue: 'Last 24 hours' }) },
    { value: '7d', label: t('logs.last7Days', { defaultValue: 'Last 7 days' }) },
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
            value={timeFilter}
            onChange={(e) =>
              setTimeFilter(e.target.value as 'all' | '15m' | '1h' | '24h' | '7d')
            }
            options={timeOptions}
            className="w-44"
          />
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
