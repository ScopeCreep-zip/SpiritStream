/**
 * Playlist Editor Modal
 * Modal for editing media playlist source items
 */
import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Plus,
  Trash2,
  GripVertical,
  File,
  FolderOpen,
  ChevronUp,
  ChevronDown,
} from 'lucide-react';
import { Modal, ModalFooter } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';
import type { PlaylistItem, MediaPlaylistSource } from '@/types/source';
import { dialogs } from '@/lib/backend/dialogs';
import { backendMode } from '@/lib/backend/env';

interface PlaylistEditorModalProps {
  open: boolean;
  onClose: () => void;
  source: MediaPlaylistSource;
  onSave: (items: PlaylistItem[]) => void;
}

export function PlaylistEditorModal({
  open,
  onClose,
  source,
  onSave,
}: PlaylistEditorModalProps) {
  const { t } = useTranslation();
  const [items, setItems] = useState<PlaylistItem[]>(source.items);
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);

  // Generate unique ID for new items
  const generateId = () => `playlist-item-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;

  // Add files via dialog
  // In Tauri mode, we can get file paths
  // In HTTP mode, we need to use the browser File API (can only get file names, not full paths)
  const handleAddFiles = useCallback(async () => {
    try {
      if (backendMode === 'tauri') {
        // Tauri mode: can get file paths
        const filePath = await dialogs.openFilePath();
        if (filePath) {
          const newItem: PlaylistItem = {
            id: generateId(),
            filePath,
            name: filePath.split('/').pop()?.split('\\').pop() || 'Untitled',
          };
          setItems((prev) => [...prev, newItem]);
        }
      } else {
        // HTTP mode: use file input with URL.createObjectURL
        const input = document.createElement('input');
        input.type = 'file';
        input.multiple = true;
        input.accept = 'video/*,audio/*,.mp4,.webm,.mkv,.mov,.avi,.mp3,.wav,.ogg,.flac';

        input.onchange = () => {
          if (input.files && input.files.length > 0) {
            const newItems: PlaylistItem[] = Array.from(input.files).map((file) => ({
              id: generateId(),
              filePath: URL.createObjectURL(file),
              name: file.name,
            }));
            setItems((prev) => [...prev, ...newItems]);
          }
        };

        input.click();
      }
    } catch (err) {
      console.error('[PlaylistEditor] Failed to add files:', err);
    }
  }, []);

  // Remove item
  const handleRemove = useCallback((index: number) => {
    setItems((prev) => prev.filter((_, i) => i !== index));
    if (selectedIndex === index) {
      setSelectedIndex(null);
    } else if (selectedIndex !== null && selectedIndex > index) {
      setSelectedIndex(selectedIndex - 1);
    }
  }, [selectedIndex]);

  // Move item up
  const handleMoveUp = useCallback((index: number) => {
    if (index === 0) return;
    setItems((prev) => {
      const newItems = [...prev];
      [newItems[index - 1], newItems[index]] = [newItems[index], newItems[index - 1]];
      return newItems;
    });
    if (selectedIndex === index) {
      setSelectedIndex(index - 1);
    } else if (selectedIndex === index - 1) {
      setSelectedIndex(index);
    }
  }, [selectedIndex]);

  // Move item down
  const handleMoveDown = useCallback((index: number) => {
    if (index === items.length - 1) return;
    setItems((prev) => {
      const newItems = [...prev];
      [newItems[index], newItems[index + 1]] = [newItems[index + 1], newItems[index]];
      return newItems;
    });
    if (selectedIndex === index) {
      setSelectedIndex(index + 1);
    } else if (selectedIndex === index + 1) {
      setSelectedIndex(index);
    }
  }, [items.length, selectedIndex]);

  // Update item name
  const handleUpdateName = useCallback((index: number, name: string) => {
    setItems((prev) =>
      prev.map((item, i) => (i === index ? { ...item, name } : item))
    );
  }, []);

  // Handle save
  const handleSave = useCallback(() => {
    onSave(items);
    onClose();
  }, [items, onSave, onClose]);

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t('stream.editPlaylist', { defaultValue: 'Edit Playlist' })}
      footer={
        <ModalFooter>
          <Button variant="ghost" onClick={onClose}>
            {t('common.cancel', { defaultValue: 'Cancel' })}
          </Button>
          <Button variant="primary" onClick={handleSave}>
            {t('common.save', { defaultValue: 'Save' })}
          </Button>
        </ModalFooter>
      }
    >
      <div className="max-h-[50vh] overflow-y-auto">
        {items.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-12 text-center">
            <FolderOpen className="w-12 h-12 text-muted mb-4" />
            <p className="text-muted mb-4">
              {t('stream.noPlaylistItems', { defaultValue: 'No items in playlist' })}
            </p>
            <Button variant="secondary" onClick={handleAddFiles}>
              <Plus className="w-4 h-4 mr-2" />
              {t('stream.addFiles', { defaultValue: 'Add Files' })}
            </Button>
          </div>
        ) : (
          <div className="space-y-1">
            {items.map((item, index) => (
              <div
                key={item.id}
                className={`flex items-center gap-2 p-2 rounded-lg border transition-colors ${
                  selectedIndex === index
                    ? 'border-primary bg-primary/10'
                    : 'border-transparent hover:bg-muted/30'
                }`}
                onClick={() => setSelectedIndex(index)}
              >
                {/* Drag handle (visual only for now) */}
                <GripVertical className="w-4 h-4 text-muted cursor-grab" />

                {/* File icon */}
                <File className="w-4 h-4 text-muted flex-shrink-0" />

                {/* Name input */}
                <input
                  type="text"
                  value={item.name || ''}
                  onChange={(e) => handleUpdateName(index, e.target.value)}
                  className="flex-1 bg-transparent border-none outline-none text-sm"
                  placeholder={item.filePath.split('/').pop()?.split('\\').pop() || 'Untitled'}
                />

                {/* Duration if available */}
                {item.duration && (
                  <span className="text-xs text-muted tabular-nums">
                    {Math.floor(item.duration / 60)}:{(item.duration % 60).toString().padStart(2, '0')}
                  </span>
                )}

                {/* Move buttons */}
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    handleMoveUp(index);
                  }}
                  disabled={index === 0}
                  className="p-1 rounded hover:bg-muted/50 disabled:opacity-30 disabled:cursor-not-allowed"
                  title={t('common.moveUp', { defaultValue: 'Move Up' })}
                >
                  <ChevronUp className="w-4 h-4" />
                </button>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    handleMoveDown(index);
                  }}
                  disabled={index === items.length - 1}
                  className="p-1 rounded hover:bg-muted/50 disabled:opacity-30 disabled:cursor-not-allowed"
                  title={t('common.moveDown', { defaultValue: 'Move Down' })}
                >
                  <ChevronDown className="w-4 h-4" />
                </button>

                {/* Remove button */}
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    handleRemove(index);
                  }}
                  className="p-1 rounded hover:bg-destructive/20 text-muted hover:text-destructive"
                  title={t('common.remove', { defaultValue: 'Remove' })}
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </div>
            ))}
          </div>
        )}

        {/* Add more files button */}
        {items.length > 0 && (
          <Button
            variant="ghost"
            size="sm"
            className="w-full mt-4"
            onClick={handleAddFiles}
          >
            <Plus className="w-4 h-4 mr-2" />
            {t('stream.addMoreFiles', { defaultValue: 'Add More Files' })}
          </Button>
        )}
      </div>
    </Modal>
  );
}
