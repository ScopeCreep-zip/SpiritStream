/**
 * Media Playlist Renderer
 * Renders the current media item from a playlist source with playback controls
 */
import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Play,
  Pause,
  SkipBack,
  SkipForward,
  Shuffle,
  Repeat,
  Repeat1,
  List,
} from 'lucide-react';
import type { MediaPlaylistSource } from '@/types/source';
import { cn } from '@/lib/utils';

interface MediaPlaylistRendererProps {
  source: MediaPlaylistSource;
  onUpdate?: (updates: Partial<MediaPlaylistSource>) => void;
  /** Whether this is shown in a layer (vs. a dedicated preview) */
  isLayerPreview?: boolean;
}

export function MediaPlaylistRenderer({
  source,
  onUpdate,
  isLayerPreview = false,
}: MediaPlaylistRendererProps) {
  const { t } = useTranslation();
  const [isPlaying, setIsPlaying] = useState(true);
  const [showControls, setShowControls] = useState(!isLayerPreview);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);

  const currentItem = source.items[source.currentItemIndex];

  // Handle next/previous
  const handleNext = useCallback(() => {
    if (!onUpdate) return;
    let nextIndex = source.currentItemIndex + 1;

    if (source.shuffleMode === 'repeat-one') {
      nextIndex = source.currentItemIndex;
    } else if (source.shuffleMode === 'all') {
      // Random index excluding current
      const availableIndices = source.items
        .map((_, i) => i)
        .filter((i) => i !== source.currentItemIndex);
      if (availableIndices.length > 0) {
        nextIndex = availableIndices[Math.floor(Math.random() * availableIndices.length)];
      }
    } else if (nextIndex >= source.items.length) {
      // Loop back to start if auto-advance is on
      nextIndex = source.autoAdvance ? 0 : source.items.length - 1;
    }

    onUpdate({ currentItemIndex: nextIndex });
  }, [source, onUpdate]);

  const handlePrevious = useCallback(() => {
    if (!onUpdate) return;
    let prevIndex = source.currentItemIndex - 1;

    if (source.shuffleMode === 'repeat-one') {
      prevIndex = source.currentItemIndex;
    } else if (prevIndex < 0) {
      prevIndex = source.items.length - 1;
    }

    onUpdate({ currentItemIndex: prevIndex });
  }, [source, onUpdate]);

  const handleTogglePlay = useCallback(() => {
    setIsPlaying((prev) => !prev);
  }, []);

  const handleToggleShuffle = useCallback(() => {
    if (!onUpdate) return;
    const modes: MediaPlaylistSource['shuffleMode'][] = ['none', 'all', 'repeat-one'];
    const currentIndex = modes.indexOf(source.shuffleMode);
    const nextIndex = (currentIndex + 1) % modes.length;
    onUpdate({ shuffleMode: modes[nextIndex] });
  }, [source.shuffleMode, onUpdate]);

  const handleSeek = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const video = document.querySelector(`video[data-source-id="${source.id}"]`) as HTMLVideoElement;
    if (video) {
      video.currentTime = parseFloat(e.target.value);
    }
  }, [source.id]);

  // Format time as MM:SS
  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  // Get shuffle mode icon
  const getShuffleModeIcon = () => {
    switch (source.shuffleMode) {
      case 'all':
        return <Shuffle className="w-4 h-4" />;
      case 'repeat-one':
        return <Repeat1 className="w-4 h-4" />;
      default:
        return <Repeat className="w-4 h-4 opacity-50" />;
    }
  };

  if (!currentItem) {
    return (
      <div className="flex items-center justify-center h-full bg-black/50 text-white">
        <div className="text-center">
          <List className="w-8 h-8 mx-auto mb-2 opacity-50" />
          <p className="text-sm opacity-75">
            {t('stream.emptyPlaylist', { defaultValue: 'No items in playlist' })}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div
      className="relative w-full h-full bg-black group"
      onMouseEnter={() => !isLayerPreview && setShowControls(true)}
      onMouseLeave={() => !isLayerPreview && setShowControls(false)}
    >
      {/* Video element */}
      <video
        data-source-id={source.id}
        src={currentItem.filePath}
        className="w-full h-full object-contain"
        autoPlay={isPlaying}
        loop={source.shuffleMode === 'repeat-one'}
        onTimeUpdate={(e) => setCurrentTime(e.currentTarget.currentTime)}
        onDurationChange={(e) => setDuration(e.currentTarget.duration)}
        onEnded={() => {
          if (source.autoAdvance && source.shuffleMode !== 'repeat-one') {
            handleNext();
          }
        }}
        onPlay={() => setIsPlaying(true)}
        onPause={() => setIsPlaying(false)}
      />

      {/* Controls overlay */}
      <div
        className={cn(
          'absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/80 to-transparent p-4 transition-opacity',
          showControls ? 'opacity-100' : 'opacity-0 pointer-events-none'
        )}
      >
        {/* Progress bar */}
        <div className="flex items-center gap-2 mb-2">
          <span className="text-xs text-white/70 tabular-nums w-10">
            {formatTime(currentTime)}
          </span>
          <input
            type="range"
            min={0}
            max={duration || 0}
            value={currentTime}
            onChange={handleSeek}
            className="flex-1 h-1 bg-white/30 rounded-full appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white"
          />
          <span className="text-xs text-white/70 tabular-nums w-10 text-right">
            {formatTime(duration)}
          </span>
        </div>

        {/* Playback controls */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <button
              onClick={handlePrevious}
              className="p-2 rounded-full hover:bg-white/20 text-white transition-colors"
              title={t('stream.previous', { defaultValue: 'Previous' })}
            >
              <SkipBack className="w-4 h-4" />
            </button>
            <button
              onClick={handleTogglePlay}
              className="p-3 rounded-full bg-white/20 hover:bg-white/30 text-white transition-colors"
              title={isPlaying ? t('stream.pause', { defaultValue: 'Pause' }) : t('stream.play', { defaultValue: 'Play' })}
            >
              {isPlaying ? (
                <Pause className="w-5 h-5" />
              ) : (
                <Play className="w-5 h-5" />
              )}
            </button>
            <button
              onClick={handleNext}
              className="p-2 rounded-full hover:bg-white/20 text-white transition-colors"
              title={t('stream.next', { defaultValue: 'Next' })}
            >
              <SkipForward className="w-4 h-4" />
            </button>
          </div>

          {/* Current item info */}
          <div className="flex-1 mx-4 text-center">
            <p className="text-sm text-white truncate">
              {currentItem.name || currentItem.filePath.split('/').pop()}
            </p>
            <p className="text-xs text-white/50">
              {source.currentItemIndex + 1} / {source.items.length}
            </p>
          </div>

          {/* Shuffle/repeat mode */}
          <button
            onClick={handleToggleShuffle}
            className={cn(
              'p-2 rounded-full hover:bg-white/20 transition-colors',
              source.shuffleMode !== 'none' ? 'text-primary' : 'text-white/50'
            )}
            title={
              source.shuffleMode === 'none'
                ? t('stream.shuffleOff', { defaultValue: 'Shuffle: Off' })
                : source.shuffleMode === 'all'
                ? t('stream.shuffleAll', { defaultValue: 'Shuffle: All' })
                : t('stream.repeatOne', { defaultValue: 'Repeat: One' })
            }
          >
            {getShuffleModeIcon()}
          </button>
        </div>
      </div>
    </div>
  );
}
