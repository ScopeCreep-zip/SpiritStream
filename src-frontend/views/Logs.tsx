import { useState, useEffect, useRef } from 'react';
import { Download, Trash2, ArrowDownToLine } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { LogConsole } from '@/components/feedback/LogConsole';
import { LogEntry } from '@/components/feedback/LogEntry';
import type { LogLevel, LogEntry as LogEntryType } from '@/types/stream';

// Demo log entries for development
const demoLogs: LogEntryType[] = [
  { id: '1', timestamp: new Date(), level: 'info', message: 'MagillaStream initialized successfully' },
  { id: '2', timestamp: new Date(), level: 'info', message: 'FFmpeg version 6.1 detected' },
  { id: '3', timestamp: new Date(), level: 'debug', message: 'Loading profile: Gaming Stream' },
  { id: '4', timestamp: new Date(), level: 'info', message: 'Profile loaded with 3 stream targets' },
  { id: '5', timestamp: new Date(), level: 'warn', message: 'NVENC encoder not available, falling back to x264' },
  { id: '6', timestamp: new Date(), level: 'info', message: 'Encoder configured: libx264, preset=balanced' },
  { id: '7', timestamp: new Date(), level: 'debug', message: 'Audio codec: AAC, bitrate: 160kbps' },
  { id: '8', timestamp: new Date(), level: 'info', message: 'Ready to stream' },
];

export function Logs() {
  const [logs, setLogs] = useState<LogEntryType[]>(demoLogs);
  const [filter, setFilter] = useState<LogLevel | 'all'>('all');
  const [autoScroll, setAutoScroll] = useState(true);
  const consoleRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when new logs arrive
  useEffect(() => {
    if (autoScroll && consoleRef.current) {
      consoleRef.current.scrollTop = consoleRef.current.scrollHeight;
    }
  }, [logs, autoScroll]);

  // Filter logs based on selected level
  const filteredLogs = filter === 'all'
    ? logs
    : logs.filter(log => log.level === filter);

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
      .map(log => `[${formatTime(log.timestamp)}] [${log.level.toUpperCase()}] ${log.message}`)
      .join('\n');

    const blob = new Blob([content], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `magillastream-logs-${new Date().toISOString().slice(0, 10)}.txt`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleClear = () => {
    setLogs([]);
  };

  const filterOptions = [
    { value: 'all', label: 'All Levels' },
    { value: 'info', label: 'Info' },
    { value: 'warn', label: 'Warning' },
    { value: 'error', label: 'Error' },
    { value: 'debug', label: 'Debug' },
  ];

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>Application Logs</CardTitle>
          <CardDescription>
            Real-time application events and debug information
          </CardDescription>
        </div>
        <div className="flex items-center" style={{ gap: '12px' }}>
          <Select
            value={filter}
            onChange={(e) => setFilter(e.target.value as LogLevel | 'all')}
            options={filterOptions}
            className="w-32"
          />
          <Button
            variant={autoScroll ? "primary" : "ghost"}
            size="sm"
            onClick={() => setAutoScroll(!autoScroll)}
            title={autoScroll ? "Auto-scroll enabled" : "Auto-scroll disabled"}
          >
            <ArrowDownToLine className="w-4 h-4" />
            Auto-scroll
          </Button>
          <Button variant="ghost" size="sm" onClick={handleExport}>
            <Download className="w-4 h-4" />
            Export
          </Button>
          <Button variant="ghost" size="sm" onClick={handleClear}>
            <Trash2 className="w-4 h-4" />
            Clear
          </Button>
        </div>
      </CardHeader>
      <CardBody style={{ padding: 0 }}>
        <LogConsole maxHeight="500px">
          <div ref={consoleRef}>
            {filteredLogs.length === 0 ? (
              <div className="text-center text-[var(--text-secondary)]" style={{ padding: '32px 16px' }}>
                No logs to display
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
