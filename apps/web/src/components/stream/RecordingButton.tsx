/**
 * Recording Button
 * Dropdown button for recording controls in the top bar
 */
import { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Circle, Square, ChevronDown, Folder } from 'lucide-react';
import {
  useRecordingStore,
  formatRecordingDuration,
  RECORDING_FORMATS,
  type RecordingFormat,
} from '@/stores/recordingStore';
import { useFileBrowser } from '@/hooks/useFileBrowser';
import { toast } from '@/hooks/useToast';

export function RecordingButton() {
  const { t } = useTranslation();
  const {
    isRecording,
    duration,
    outputPath,
    format,
    startRecording,
    stopRecording,
    setOutputPath,
    setFormat,
    incrementDuration,
  } = useRecordingStore();

  const [isOpen, setIsOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // File browser for selecting output directory
  const { FileBrowser, openDirectoryPath } = useFileBrowser();

  const handleBrowseDirectory = async () => {
    const path = await openDirectoryPath({
      title: t('recording.selectOutputDirectory', { defaultValue: 'Select Output Directory' }),
      initialPath: outputPath,
    });
    if (path) {
      setOutputPath(path);
    }
  };

  // Increment duration while recording
  useEffect(() => {
    if (isRecording) {
      intervalRef.current = setInterval(() => {
        incrementDuration();
      }, 1000);
    } else {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    }

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
      }
    };
  }, [isRecording, incrementDuration]);

  // Close dropdown when clicking outside
  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node) &&
        buttonRef.current &&
        !buttonRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    }

    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [isOpen]);

  const handleStartRecording = async () => {
    try {
      await startRecording();
      toast.success(t('recording.started', { defaultValue: 'Recording started' }));
      setIsOpen(false);
    } catch (err) {
      toast.error(
        t('recording.startFailed', {
          error: err instanceof Error ? err.message : String(err),
          defaultValue: `Failed to start recording: ${err instanceof Error ? err.message : String(err)}`,
        })
      );
    }
  };

  const handleStopRecording = async () => {
    try {
      await stopRecording();
      toast.success(t('recording.stopped', { defaultValue: 'Recording stopped' }));
    } catch (err) {
      toast.error(
        t('recording.stopFailed', {
          error: err instanceof Error ? err.message : String(err),
          defaultValue: `Failed to stop recording: ${err instanceof Error ? err.message : String(err)}`,
        })
      );
    }
  };

  return (
    <>
    <FileBrowser />
    <div className="relative">
      <button
        ref={buttonRef}
        onClick={() => (isRecording ? handleStopRecording() : setIsOpen(!isOpen))}
        className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm font-medium transition-colors ${
          isRecording
            ? 'bg-red-500 text-white hover:bg-red-600'
            : 'bg-[var(--bg-elevated)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]'
        }`}
        title={
          isRecording
            ? t('recording.stopRecording', { defaultValue: 'Stop Recording' })
            : t('recording.recording', { defaultValue: 'Recording' })
        }
      >
        {isRecording ? (
          <>
            <Square className="w-3 h-3 fill-current" />
            <span className="tabular-nums">{formatRecordingDuration(duration)}</span>
          </>
        ) : (
          <>
            <Circle className="w-3 h-3" />
            <span className="hidden sm:inline">
              {t('recording.rec', { defaultValue: 'REC' })}
            </span>
            <ChevronDown className="w-3 h-3" />
          </>
        )}
      </button>

      {isOpen && !isRecording && (
        <div
          ref={dropdownRef}
          className="absolute top-full mt-1 right-0 z-50 w-64 bg-[var(--bg-elevated)] border border-[var(--border-default)] rounded-lg shadow-lg p-3"
        >
          <h4 className="text-xs font-medium text-[var(--text-primary)] mb-3">
            {t('recording.settings', { defaultValue: 'Recording Settings' })}
          </h4>

          {/* Format selector */}
          <div className="mb-3">
            <label className="block text-xs text-[var(--text-muted)] mb-1">
              {t('recording.format', { defaultValue: 'Format' })}
            </label>
            <select
              value={format}
              onChange={(e) => setFormat(e.target.value as RecordingFormat)}
              className="w-full px-2 py-1.5 text-sm bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded text-[var(--text-secondary)] cursor-pointer hover:border-[var(--border-strong)] focus:outline-none focus:ring-1 focus:ring-primary/50"
            >
              {RECORDING_FORMATS.map((f) => (
                <option key={f.value} value={f.value}>
                  {f.label}
                </option>
              ))}
            </select>
          </div>

          {/* Output path */}
          <div className="mb-4">
            <label className="block text-xs text-[var(--text-muted)] mb-1">
              {t('recording.outputPath', { defaultValue: 'Output Path' })}
            </label>
            <div className="flex gap-1">
              <input
                type="text"
                value={outputPath}
                onChange={(e) => setOutputPath(e.target.value)}
                className="flex-1 px-2 py-1.5 text-sm bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded text-[var(--text-secondary)] focus:outline-none focus:ring-1 focus:ring-primary/50"
                placeholder="~/Videos"
              />
              <button
                type="button"
                onClick={handleBrowseDirectory}
                className="px-2 py-1.5 bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded hover:bg-[var(--bg-hover)] text-[var(--text-muted)] hover:text-[var(--text-secondary)]"
                title={t('recording.browse', { defaultValue: 'Browse' })}
              >
                <Folder className="w-4 h-4" />
              </button>
            </div>
          </div>

          {/* Start button */}
          <button
            type="button"
            onClick={handleStartRecording}
            className="w-full flex items-center justify-center gap-2 px-3 py-2 bg-red-500 hover:bg-red-600 text-white rounded-lg text-sm font-medium transition-colors"
          >
            <Circle className="w-3 h-3 fill-current" />
            {t('recording.startRecording', { defaultValue: 'Start Recording' })}
          </button>

          <p className="mt-2 text-[10px] text-[var(--text-muted)] text-center">
            {t('recording.hint', {
              defaultValue: 'Recording will save to the specified path',
            })}
          </p>
        </div>
      )}
    </div>
    </>
  );
}
