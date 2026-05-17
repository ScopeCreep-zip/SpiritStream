import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';
import {
  eventToBinding,
  formatBindingTokens,
  isMacPlatform,
  type HotkeyBinding,
} from '@/lib/hotkey';
import { cn } from '@/lib/cn';

interface KeybindCaptureDialogProps {
  open: boolean;
  onClose: () => void;
  onCommit: (binding: HotkeyBinding) => void;
  /** Optional label shown above the capture surface (e.g. "Panic disconnect"). */
  label?: string;
}

export function KeybindCaptureDialog({
  open,
  onClose,
  onCommit,
  label,
}: KeybindCaptureDialogProps): React.ReactElement {
  const { t } = useTranslation();
  const [preview, setPreview] = useState<HotkeyBinding | null>(null);
  const isMac = isMacPlatform();

  // Reset preview each time the dialog opens.
  useEffect(() => {
    if (open) setPreview(null);
  }, [open]);

  // Capture keys while open. The Modal already traps focus; we listen at
  // document level so the user can press any combination without first
  // tabbing into a specific control.
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent): void => {
      // Don't intercept Escape — let the Modal handle close.
      if (e.key === 'Escape') return;
      e.preventDefault();
      e.stopPropagation();
      const captured = eventToBinding(e);
      if (captured) setPreview(captured);
    };
    document.addEventListener('keydown', handler, { capture: true });
    return () => document.removeEventListener('keydown', handler, { capture: true });
  }, [open]);

  const handleConfirm = useCallback((): void => {
    if (preview) {
      onCommit(preview);
      onClose();
    }
  }, [preview, onCommit, onClose]);

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t('hotkey.capture.title', { defaultValue: 'Set hotkey' })}
      maxWidth="420px"
      footer={
        <>
          <Button variant="ghost" size="sm" onClick={onClose}>
            {t('common.cancel', { defaultValue: 'Cancel' })}
          </Button>
          <Button size="sm" onClick={handleConfirm} disabled={!preview}>
            {t('hotkey.capture.confirm', { defaultValue: 'Set' })}
          </Button>
        </>
      }
    >
      <div className="flex flex-col items-center gap-4 py-2">
        {label && (
          <p className="text-sm text-text-secondary text-center">{label}</p>
        )}
        <p className="text-xs text-text-tertiary text-center">
          {t('hotkey.capture.prompt', {
            defaultValue: 'Press the key combination you want to use.',
          })}
        </p>
        <div
          className={cn(
            'flex items-center gap-1 min-h-[3rem] px-4 py-3 rounded-lg',
            'border-2 border-dashed border-border-default bg-bg-sunken',
          )}
        >
          {preview ? (
            formatBindingTokens(preview, isMac).map((token, i) => (
              <kbd
                key={`${token}-${i}`}
                className="px-2 py-1 text-sm font-mono rounded border border-border-default bg-bg-elevated text-text-primary min-w-[2rem] text-center"
              >
                {token}
              </kbd>
            ))
          ) : (
            <span className="text-text-muted text-sm">
              {t('hotkey.capture.waiting', { defaultValue: 'Waiting for keypress…' })}
            </span>
          )}
        </div>
      </div>
    </Modal>
  );
}
