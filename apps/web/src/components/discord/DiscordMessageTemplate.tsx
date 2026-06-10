import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Smile, Image as ImageIcon, X, Info } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { FormLabel, FormHelper } from '@/components/ui/Form';
import { cn } from '@/lib/cn';

/**
 * Common emojis grouped by streaming-context category. The picker
 * sits inline with the message textarea so users don't lose cursor
 * context when inserting.
 */
const EMOJI_CATEGORIES = [
  {
    nameKey: 'discord.emojiCategories.streaming',
    emojis: ['🎮', '🔴', '📺', '🎬', '🎥', '📡', '🎙️', '🎧', '🕹️', '💻'],
  },
  {
    nameKey: 'discord.emojiCategories.reactions',
    emojis: ['🔥', '💯', '⭐', '✨', '💪', '🎉', '🚀', '👀', '❤️', '💜'],
  },
  {
    nameKey: 'discord.emojiCategories.fun',
    emojis: ['😎', '🤩', '😄', '🥳', '👋', '🙌', '👏', '💬', '📢', '🔔'],
  },
] as const;

interface DiscordMessageTemplateProps {
  webhookEnabled: boolean;
  goLiveMessage: string;
  setGoLiveMessage: (updater: string | ((prev: string) => string)) => void;
  onMessageBlur: () => void;
  imagePath: string;
  onSelectImage: () => void;
  onRemoveImage: () => void;
}

export function DiscordMessageTemplate({
  webhookEnabled,
  goLiveMessage,
  setGoLiveMessage,
  onMessageBlur,
  imagePath,
  onSelectImage,
  onRemoveImage,
}: DiscordMessageTemplateProps): React.ReactElement {
  const { t } = useTranslation();
  const [showEmojiPicker, setShowEmojiPicker] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const emojiPickerRef = useRef<HTMLDivElement>(null);

  // Close emoji picker on outside click.
  useEffect(() => {
    if (!showEmojiPicker) return;
    const handleClickOutside = (event: MouseEvent): void => {
      if (emojiPickerRef.current && !emojiPickerRef.current.contains(event.target as Node)) {
        setShowEmojiPicker(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [showEmojiPicker]);

  const handleEmojiSelect = useCallback(
    (emoji: string) => {
      const textarea = textareaRef.current;
      if (textarea) {
        const start = textarea.selectionStart;
        const end = textarea.selectionEnd;
        setGoLiveMessage((prev) => prev.slice(0, start) + emoji + prev.slice(end));
        // Restore cursor position after the inserted emoji.
        setTimeout(() => {
          textarea.focus();
          textarea.setSelectionRange(start + emoji.length, start + emoji.length);
        }, 0);
      } else {
        setGoLiveMessage((prev) => prev + emoji);
      }
      setShowEmojiPicker(false);
    },
    [setGoLiveMessage]
  );

  const imageFileName = imagePath ? imagePath.split(/[\\/]/).pop() : null;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('discord.goLiveMessage')}</CardTitle>
        <CardDescription>{t('discord.goLiveMessageDescription')}</CardDescription>
      </CardHeader>
      <CardBody className="space-y-4">
        <div className="space-y-2">
          <div className="relative">
            <textarea
              ref={textareaRef}
              value={goLiveMessage}
              onChange={(e) => setGoLiveMessage(e.target.value)}
              onBlur={onMessageBlur}
              disabled={!webhookEnabled}
              rows={4}
              // Discord's webhook `content` field is hard-capped at 2000
              // characters (per Discord API docs). Without `maxLength`
              // the user could type a longer message that silently
              // truncates / fails at send time. The counter below the
              // textarea warns as they approach the limit.
              maxLength={2000}
              className={cn(
                'w-full px-3 py-2 pe-10 rounded-lg',
                'bg-bg-sunken border border-border-default',
                'text-sm text-text-primary',
                'placeholder:text-text-muted',
                'focus:outline-none focus:ring-2 focus:ring-ring-default focus:border-transparent',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'resize-y min-h-[100px]'
              )}
              placeholder={t('discord.messagePlaceholder')}
            />
            <div className="absolute end-2 top-2" ref={emojiPickerRef}>
              <button
                type="button"
                onClick={() => setShowEmojiPicker(!showEmojiPicker)}
                disabled={!webhookEnabled}
                className={cn(
                  'p-1.5 rounded-md transition-colors',
                  'text-text-tertiary hover:text-text-primary',
                  'hover:bg-bg-muted',
                  'disabled:opacity-50 disabled:cursor-not-allowed'
                )}
                title={t('discord.insertEmoji')}
              >
                <Smile className="w-4 h-4" />
              </button>

              {showEmojiPicker && (
                <div className="absolute end-0 top-full mt-1 z-50 p-2 rounded-lg bg-bg-surface border border-border-default shadow-lg w-64">
                  {EMOJI_CATEGORIES.map((category) => (
                    <div key={category.nameKey} className="mb-2 last:mb-0">
                      <div className="text-xs text-text-tertiary mb-1 px-1">
                        {t(category.nameKey)}
                      </div>
                      <div className="flex flex-wrap gap-1">
                        {category.emojis.map((emoji) => (
                          <button
                            key={emoji}
                            type="button"
                            onClick={() => handleEmojiSelect(emoji)}
                            className="p-1.5 rounded hover:bg-bg-muted text-lg transition-colors"
                          >
                            {emoji}
                          </button>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
          <div className="flex justify-end">
            <span
              className={cn(
                'text-xs',
                goLiveMessage.length > 1800 ? 'text-warning-text' : 'text-text-tertiary'
              )}
              aria-live="polite"
            >
              {t('discord.messageCharCount', {
                defaultValue: '{{count}}/2000',
                count: goLiveMessage.length,
              })}
            </span>
          </div>
        </div>

        <div className="flex items-start gap-2 p-3 rounded-lg bg-bg-base border border-border-default">
          <Info className="w-4 h-4 text-text-tertiary flex-shrink-0 mt-0.5" />
          <div className="text-xs text-text-tertiary space-y-1">
            <p>{t('discord.markdownSupport')}</p>
            <p className="font-mono">{t('discord.markdownExample')}</p>
          </div>
        </div>

        <div className="space-y-2">
          <FormLabel>{t('discord.attachImage')}</FormLabel>
          {imagePath ? (
            <div className="flex items-center gap-3 p-3 rounded-lg bg-bg-base border border-border-default">
              <ImageIcon className="w-5 h-5 text-text-tertiary" />
              <span className="flex-1 text-sm text-text-primary truncate">{imageFileName}</span>
              <Button
                variant="ghost"
                size="sm"
                onClick={onRemoveImage}
                disabled={!webhookEnabled}
                title={t('discord.removeImage')}
              >
                <X className="w-4 h-4" />
              </Button>
            </div>
          ) : (
            <Button variant="outline" onClick={onSelectImage} disabled={!webhookEnabled}>
              <ImageIcon className="w-4 h-4" />
              {t('discord.selectImage')}
            </Button>
          )}
          <FormHelper>{t('discord.imageHint')}</FormHelper>
        </div>
      </CardBody>
    </Card>
  );
}
