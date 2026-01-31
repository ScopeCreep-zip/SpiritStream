/**
 * Hotkey Capture Modal
 * Modal for capturing a key combination for layer visibility hotkeys
 */
import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';
import { useHotkeyStore } from '@/stores/hotkeyStore';
import { formatHotkeyBinding, type HotkeyBinding } from '@/types/hotkeys';

interface HotkeyCaptureModalProps {
  open: boolean;
  onClose: () => void;
  layerId: string;
  sceneId: string;
  layerName: string;
}

export function HotkeyCaptureModal({
  open,
  onClose,
  layerId,
  sceneId,
  layerName,
}: HotkeyCaptureModalProps) {
  const { t } = useTranslation();
  const { getLayerBinding, setLayerHotkey, removeLayerHotkey } = useHotkeyStore();

  const [capturedKey, setCapturedKey] = useState<string | null>(null);
  const [capturedDisplayKey, setCapturedDisplayKey] = useState<string | null>(null);
  const [capturedModifiers, setCapturedModifiers] = useState<HotkeyBinding['modifiers']>({});
  const [isCapturing, setIsCapturing] = useState(false);

  // Get existing binding
  const existingBinding = getLayerBinding(layerId, sceneId);

  // Reset state when modal opens
  useEffect(() => {
    if (open) {
      if (existingBinding) {
        setCapturedKey(existingBinding.key);
        setCapturedDisplayKey(existingBinding.displayKey);
        setCapturedModifiers(existingBinding.modifiers);
      } else {
        setCapturedKey(null);
        setCapturedDisplayKey(null);
        setCapturedModifiers({});
      }
      setIsCapturing(false);
    }
  }, [open, existingBinding]);

  // Key capture handler
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!isCapturing) return;

      e.preventDefault();
      e.stopPropagation();

      // Ignore modifier-only keys
      if (['Control', 'Alt', 'Shift', 'Meta'].includes(e.key)) {
        return;
      }

      const key = e.code;
      let displayKey = e.key;

      // Clean up display key
      if (key.startsWith('Digit')) {
        displayKey = key.replace('Digit', '');
      } else if (key.startsWith('Key')) {
        displayKey = key.replace('Key', '');
      } else if (key === 'Space') {
        displayKey = 'Space';
      }

      const modifiers: HotkeyBinding['modifiers'] = {
        ctrl: e.ctrlKey,
        alt: e.altKey,
        shift: e.shiftKey,
        meta: e.metaKey,
      };

      setCapturedKey(key);
      setCapturedDisplayKey(displayKey);
      setCapturedModifiers(modifiers);
      setIsCapturing(false);
    },
    [isCapturing]
  );

  // Attach key listener when capturing
  useEffect(() => {
    if (isCapturing) {
      document.addEventListener('keydown', handleKeyDown);
      return () => document.removeEventListener('keydown', handleKeyDown);
    }
  }, [isCapturing, handleKeyDown]);

  const handleSave = () => {
    if (capturedKey && capturedDisplayKey) {
      setLayerHotkey(layerId, sceneId, capturedKey, capturedDisplayKey, capturedModifiers);
    }
    onClose();
  };

  const handleClear = () => {
    removeLayerHotkey(layerId, sceneId);
    setCapturedKey(null);
    setCapturedDisplayKey(null);
    setCapturedModifiers({});
  };

  const formatCurrentBinding = () => {
    if (!capturedKey || !capturedDisplayKey) {
      return t('hotkeys.none', { defaultValue: 'None' });
    }

    return formatHotkeyBinding({
      id: '',
      action: 'toggleLayerVisibility',
      key: capturedKey,
      displayKey: capturedDisplayKey,
      modifiers: capturedModifiers,
      enabled: true,
    });
  };

  const footer = (
    <div className="flex justify-between w-full">
      <Button variant="ghost" onClick={handleClear} disabled={!capturedKey}>
        {t('common.clear', { defaultValue: 'Clear' })}
      </Button>
      <div className="flex gap-2">
        <Button variant="secondary" onClick={onClose}>
          {t('common.cancel', { defaultValue: 'Cancel' })}
        </Button>
        <Button variant="primary" onClick={handleSave} disabled={!capturedKey}>
          {t('common.save', { defaultValue: 'Save' })}
        </Button>
      </div>
    </div>
  );

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t('hotkeys.setVisibilityHotkey', { defaultValue: 'Set Visibility Hotkey' })}
      footer={footer}
    >
      <div className="space-y-4">
        <p className="text-sm text-[var(--text-secondary)]">
          {t('hotkeys.layerLabel', { layer: layerName, defaultValue: `Layer: ${layerName}` })}
        </p>

        <div className="text-sm text-[var(--text-muted)]">
          {isCapturing
            ? t('hotkeys.pressKey', { defaultValue: 'Press the key combination...' })
            : t('hotkeys.clickToCapture', {
                defaultValue: 'Click the box below and press a key combination',
              })}
        </div>

        <button
          type="button"
          onClick={() => setIsCapturing(true)}
          className={`w-full h-16 flex items-center justify-center text-lg font-mono rounded-lg border-2 transition-colors ${
            isCapturing
              ? 'border-primary bg-primary/10 text-primary animate-pulse'
              : 'border-[var(--border-default)] bg-[var(--bg-sunken)] text-[var(--text-secondary)] hover:border-[var(--border-strong)]'
          }`}
        >
          {isCapturing
            ? t('hotkeys.listening', { defaultValue: 'Listening...' })
            : formatCurrentBinding()}
        </button>

        {existingBinding && (
          <p className="text-xs text-[var(--text-muted)]">
            {t('hotkeys.currentBinding', {
              binding: formatHotkeyBinding(existingBinding),
              defaultValue: `Current: ${formatHotkeyBinding(existingBinding)}`,
            })}
          </p>
        )}
      </div>
    </Modal>
  );
}
