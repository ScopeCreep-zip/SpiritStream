import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Search } from 'lucide-react';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { ChatList } from '@/components/chat/ChatList';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import type { ChatMessage } from '@spiritstream/types';

interface ChatSearchProps {
  open: boolean;
  onClose: () => void;
  /** On-screen messages, used by the "On screen" scope filter. */
  messages: ChatMessage[];
}

/**
 * Chat search modal. Two scopes: on-screen filter over the messages in
 * memory, and a full-session scan that hits `api.chat.searchSession`.
 * Owns its own scope + query + results state — independent of the
 * parent's send composer.
 */
export function ChatSearch({
  open,
  onClose,
  messages,
}: ChatSearchProps): React.ReactElement {
  const { t } = useTranslation();
  const [searchQuery, setSearchQuery] = useState('');
  const [searchScope, setSearchScope] = useState<'memory' | 'session'>('memory');
  const [searchResults, setSearchResults] = useState<ChatMessage[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  // Track mount status so a late-returning search response doesn't
  // call setState on an unmounted modal (React warning + dev-mode
  // double-cleanup error). Reset each time the modal re-opens.
  const isMounted = useRef(true);
  useEffect(() => {
    isMounted.current = true;
    return () => {
      isMounted.current = false;
    };
  }, []);

  const filteredMessages = useMemo(() => {
    if (!searchQuery.trim() || searchScope === 'session') {
      return messages;
    }
    const query = searchQuery.trim().toLowerCase();
    return messages.filter(
      (message) =>
        message.username.toLowerCase().includes(query) ||
        message.message.toLowerCase().includes(query)
    );
  }, [messages, searchQuery, searchScope]);

  const modalMessages = useMemo(() => {
    if (searchScope === 'session') {
      return searchQuery.trim().length > 0 ? searchResults : [];
    }
    return filteredMessages;
  }, [filteredMessages, searchResults, searchQuery, searchScope]);

  const searchEmptyLabel = useMemo(() => {
    if (!searchQuery.trim()) {
      return searchScope === 'session'
        ? t('chat.searchSessionHint', {
            defaultValue: 'Enter a search term to scan the current stream session.',
          })
        : t('chat.searchMemoryHint', {
            defaultValue: 'Type to filter the messages currently on screen.',
          });
    }
    return t('chat.searchNoResults', { defaultValue: 'No matching messages.' });
  }, [searchQuery, searchScope, t]);

  const handleSearchSession = useCallback(async (): Promise<void> => {
    const query = searchQuery.trim();
    if (!query) {
      setSearchResults([]);
      return;
    }
    try {
      setIsSearching(true);
      const results = await api.chat.searchSession(query, 500);
      if (isMounted.current) {
        setSearchResults(results);
      }
    } catch (error) {
      logger.error('Failed to search chat session:', error);
      if (isMounted.current) {
        toast.error(t('chat.searchFailed', { defaultValue: 'Failed to search chat logs.' }));
      }
    } finally {
      if (isMounted.current) {
        setIsSearching(false);
      }
    }
  }, [searchQuery, t]);

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t('chat.searchTitle', { defaultValue: 'Search chat' })}
      maxWidth="720px"
    >
      <div className="space-y-4">
        <div className="flex flex-wrap items-center gap-3">
          <div className="flex-1 min-w-[240px]">
            <Input
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder={t('chat.searchPlaceholder', { defaultValue: 'Search chat...' })}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && searchScope === 'session') {
                  event.preventDefault();
                  handleSearchSession();
                }
              }}
            />
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant={searchScope === 'memory' ? 'secondary' : 'ghost'}
              size="sm"
              onClick={() => setSearchScope('memory')}
            >
              {t('chat.searchInMemory', { defaultValue: 'On screen' })}
            </Button>
            <Button
              variant={searchScope === 'session' ? 'secondary' : 'ghost'}
              size="sm"
              onClick={() => setSearchScope('session')}
            >
              {t('chat.searchSession', { defaultValue: 'Full session' })}
            </Button>
            {searchScope === 'session' && (
              <Button
                size="sm"
                onClick={handleSearchSession}
                disabled={!searchQuery.trim() || isSearching}
                className="gap-2"
              >
                <Search className="w-4 h-4" />
                {isSearching
                  ? t('chat.searching', { defaultValue: 'Searching...' })
                  : t('chat.search', { defaultValue: 'Search' })}
              </Button>
            )}
          </div>
        </div>
        <ChatList
          messages={modalMessages}
          className="max-h-[480px]"
          emptyLabel={searchEmptyLabel}
          showTimestamps
        />
        <div className="flex items-center justify-between text-xs text-text-tertiary">
          <span>
            {searchScope === 'session'
              ? t('chat.searchSessionHint', {
                  defaultValue: 'Enter a search term to scan the current stream session.',
                })
              : t('chat.searchMemoryHint', {
                  defaultValue: 'Type to filter the messages currently on screen.',
                })}
          </span>
          <span>{t('chat.searchLimit', { defaultValue: 'Up to 500 matches.' })}</span>
        </div>
      </div>
    </Modal>
  );
}
