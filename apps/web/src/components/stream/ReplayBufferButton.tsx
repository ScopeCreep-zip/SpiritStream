/**
 * Replay Buffer Button
 * Dropdown button for replay buffer controls in the top bar
 */
import { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Video, ChevronDown, Save, Play, Square, CheckCircle, Folder } from 'lucide-react';
import { useReplayBufferStore } from '@/stores/replayBufferStore';
import { useFileBrowser } from '@/hooks/useFileBrowser';
import { toast } from '@/hooks/useToast';

const DURATION_OPTIONS = [
  { value: 5, label: '5 sec' },
  { value: 10, label: '10 sec' },
  { value: 15, label: '15 sec' },
  { value: 30, label: '30 sec' },
  { value: 60, label: '1 min' },
  { value: 120, label: '2 min' },
];

export function ReplayBufferButton() {
  const { t } = useTranslation();
  const [isOpen, setIsOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const {
    isActive,
    duration,
    bufferedSeconds,
    outputPath,
    isSaving,
    isLoading,
    lastSavedPath,
    error,
    toggleBuffer,
    saveReplay,
    setDuration,
    setOutputPath,
    clearError,
  } = useReplayBufferStore();

  // File browser for selecting output directory
  const { FileBrowser, openDirectoryPath } = useFileBrowser();

  const handleBrowseDirectory = async () => {
    const path = await openDirectoryPath({
      title: t('stream.selectReplayDirectory', { defaultValue: 'Select Replay Output Directory' }),
      initialPath: outputPath,
    });
    if (path) {
      setOutputPath(path);
    }
  };

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

  const handleToggleBuffer = async () => {
    try {
      await toggleBuffer();
      if (!isActive) {
        toast.success(t('stream.bufferStarted', { defaultValue: 'Replay buffer started' }));
      } else {
        toast.success(t('stream.bufferStopped', { defaultValue: 'Replay buffer stopped' }));
      }
    } catch (err) {
      toast.error(
        t('stream.bufferToggleFailed', {
          error: err instanceof Error ? err.message : String(err),
          defaultValue: `Failed to toggle buffer: ${err instanceof Error ? err.message : String(err)}`,
        })
      );
    }
  };

  const handleSaveReplay = async () => {
    try {
      await saveReplay();
      toast.success(t('stream.replaySaved', { defaultValue: 'Replay saved!' }));
    } catch (err) {
      toast.error(
        t('stream.replaySaveFailed', {
          error: err instanceof Error ? err.message : String(err),
          defaultValue: `Failed to save replay: ${err instanceof Error ? err.message : String(err)}`,
        })
      );
    }
  };

  // Clear error when dropdown closes
  useEffect(() => {
    if (!isOpen && error) {
      clearError();
    }
  }, [isOpen, error, clearError]);

  // Format buffered time
  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    if (mins > 0) {
      return `${mins}:${secs.toString().padStart(2, '0')}`;
    }
    return `${secs}s`;
  };

  return (
    <>
    <FileBrowser />
    <div className="relative">
      <button
        ref={buttonRef}
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm font-medium transition-all ${
          isActive
            ? 'bg-primary/20 text-primary border border-primary/50'
            : 'bg-[var(--bg-sunken)] text-[var(--text-secondary)] hover:bg-[var(--bg-elevated)] border border-transparent'
        }`}
      >
        <Video className={`w-4 h-4 ${isActive ? 'animate-pulse' : ''}`} />
        <span className="hidden sm:inline">
          {isActive
            ? t('stream.buffer', { defaultValue: 'Buffer' })
            : t('stream.replay', { defaultValue: 'Replay' })}
        </span>
        {isActive && (
          <span className="text-xs tabular-nums bg-primary/20 px-1.5 py-0.5 rounded">
            {formatTime(bufferedSeconds)}
          </span>
        )}
        <ChevronDown className="w-3.5 h-3.5" />
      </button>

      {isOpen && (
        <div
          ref={dropdownRef}
          className="absolute top-full mt-1 right-0 w-64 bg-[var(--bg-elevated)] border border-[var(--border-default)] rounded-lg shadow-lg z-50"
        >
          <div className="p-3 space-y-3">
            <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
              {t('stream.replayBuffer', { defaultValue: 'Replay Buffer' })}
            </h4>

            {/* Duration selector */}
            <div className="space-y-1.5">
              <label className="text-xs text-[var(--text-muted)]">
                {t('stream.bufferDuration', { defaultValue: 'Duration' })}
              </label>
              <div className="flex flex-wrap gap-1">
                {DURATION_OPTIONS.map((opt) => (
                  <button
                    key={opt.value}
                    type="button"
                    onClick={() => setDuration(opt.value)}
                    disabled={isActive}
                    className={`px-2 py-1 text-xs rounded transition-colors ${
                      duration === opt.value
                        ? 'bg-primary text-primary-foreground'
                        : isActive
                          ? 'bg-[var(--bg-sunken)] text-[var(--text-muted)] cursor-not-allowed'
                          : 'bg-[var(--bg-sunken)] text-[var(--text-secondary)] hover:bg-[var(--bg-hover)]'
                    }`}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
            </div>

            {/* Output path */}
            <div className="space-y-1.5">
              <label className="text-xs text-[var(--text-muted)]">
                {t('stream.outputPath', { defaultValue: 'Output Path' })}
              </label>
              <div className="flex gap-1">
                <input
                  type="text"
                  value={outputPath}
                  onChange={(e) => setOutputPath(e.target.value)}
                  disabled={isActive}
                  className="flex-1 px-2 py-1.5 text-xs bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded text-[var(--text-secondary)] focus:outline-none focus:ring-1 focus:ring-primary/50 disabled:opacity-50 disabled:cursor-not-allowed"
                  placeholder="~/Videos/Replays"
                />
                <button
                  type="button"
                  onClick={handleBrowseDirectory}
                  disabled={isActive}
                  className="px-2 py-1.5 bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded hover:bg-[var(--bg-hover)] text-[var(--text-muted)] hover:text-[var(--text-secondary)] disabled:opacity-50 disabled:cursor-not-allowed"
                  title={t('stream.browse', { defaultValue: 'Browse' })}
                >
                  <Folder className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>

            {/* Status indicator */}
            <div className="flex items-center gap-2 px-3 py-2 bg-[var(--bg-sunken)] rounded-lg">
              {isActive ? (
                <>
                  <div className="w-2 h-2 rounded-full bg-primary animate-pulse" />
                  <span className="text-sm text-[var(--text-secondary)]">
                    {t('stream.bufferActive', { defaultValue: 'Active' })}
                  </span>
                  <span className="ml-auto text-sm tabular-nums text-primary">
                    {formatTime(bufferedSeconds)} / {formatTime(duration)}
                  </span>
                </>
              ) : (
                <>
                  <div className="w-2 h-2 rounded-full bg-[var(--text-muted)]" />
                  <span className="text-sm text-[var(--text-muted)]">
                    {t('stream.bufferInactive', { defaultValue: 'Inactive' })}
                  </span>
                </>
              )}
            </div>

            {/* Error display */}
            {error && (
              <div className="flex items-start gap-2 px-2 py-1.5 bg-[var(--error)]/10 border border-[var(--error)]/20 rounded text-xs">
                <span className="text-[var(--error)] flex-1">{error}</span>
                <button
                  type="button"
                  onClick={clearError}
                  className="text-[var(--error)] hover:text-[var(--error)]/80"
                >
                  ×
                </button>
              </div>
            )}

            {/* Last saved path */}
            {lastSavedPath && !error && (
              <div className="flex items-center gap-2 px-2 py-1.5 bg-[var(--success)]/10 border border-[var(--success)]/20 rounded text-xs">
                <CheckCircle className="w-3.5 h-3.5 text-[var(--success)]" />
                <span className="text-[var(--text-secondary)] truncate flex-1">
                  {lastSavedPath.split('/').pop()}
                </span>
              </div>
            )}
          </div>

          {/* Actions */}
          <div className="border-t border-[var(--border-default)] p-2 flex gap-2">
            <button
              type="button"
              onClick={handleToggleBuffer}
              disabled={isLoading}
              className={`flex-1 flex items-center justify-center gap-1.5 px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                isLoading
                  ? 'bg-[var(--bg-sunken)] text-[var(--text-muted)] cursor-wait'
                  : isActive
                    ? 'bg-[var(--error)]/10 hover:bg-[var(--error)]/20 text-[var(--error)]'
                    : 'bg-primary/10 hover:bg-primary/20 text-primary'
              }`}
            >
              {isLoading ? (
                <>
                  <div className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
                  {t('common.loading', { defaultValue: 'Loading...' })}
                </>
              ) : isActive ? (
                <>
                  <Square className="w-4 h-4" />
                  {t('stream.stopBuffer', { defaultValue: 'Stop Buffer' })}
                </>
              ) : (
                <>
                  <Play className="w-4 h-4" />
                  {t('stream.startBuffer', { defaultValue: 'Start Buffer' })}
                </>
              )}
            </button>

            <button
              type="button"
              onClick={handleSaveReplay}
              disabled={!isActive || bufferedSeconds === 0 || isSaving}
              className={`flex items-center justify-center gap-1.5 px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                isActive && bufferedSeconds > 0 && !isSaving
                  ? 'bg-[var(--success)]/10 hover:bg-[var(--success)]/20 text-[var(--success)]'
                  : 'bg-[var(--bg-sunken)] text-[var(--text-muted)] cursor-not-allowed'
              }`}
              title={t('stream.saveReplayHotkey', { defaultValue: 'Save Replay (F9)' })}
            >
              <Save className={`w-4 h-4 ${isSaving ? 'animate-pulse' : ''}`} />
              {isSaving
                ? t('stream.saving', { defaultValue: 'Saving...' })
                : t('stream.save', { defaultValue: 'Save' })}
            </button>
          </div>
        </div>
      )}
    </div>
    </>
  );
}
