import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Search } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import { cn } from '@/lib/cn';

interface AuthorNameProps {
  displayName: string;
  className?: string;
  style?: React.CSSProperties;
  /**
   * The author's canonical `hash:` pseudonym (`author.login`) when anonymous
   * mode rewrote this message; `null`/absent for a plaintext author. When
   * present, the name becomes a button that opens a re-identify check.
   */
  pseudonym?: string | null;
}

/**
 * Renders a chat author's name. For an anonymised author it offers a
 * re-identify affordance: the user can CONFIRM a name they already suspect
 * (e.g. a known harasser) against the one-way pseudonym. The salt stays in
 * the backend and the hash can't be reversed — this only verifies a guess.
 */
export function AuthorName({
  displayName,
  className,
  style,
  pseudonym,
}: AuthorNameProps): React.ReactElement {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [candidate, setCandidate] = useState('');
  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<'match' | 'noMatch' | null>(null);

  const isAnon = typeof pseudonym === 'string' && pseudonym.startsWith('hash:');
  if (!isAnon) {
    return (
      <span className={className} style={style}>
        {displayName}
      </span>
    );
  }

  const check = async (): Promise<void> => {
    const guess = candidate.trim();
    if (!guess) return;
    setChecking(true);
    setResult(null);
    try {
      const matches = await api.chat.reidentify(guess, pseudonym);
      setResult(matches ? 'match' : 'noMatch');
    } catch (error) {
      logger.error('[AuthorName] reidentify failed:', error);
      toast.error(t('chat.reidentify.failed', { defaultValue: 'Identity check failed' }));
    } finally {
      setChecking(false);
    }
  };

  return (
    <span className="relative inline-flex">
      <button
        type="button"
        onClick={() => {
          setOpen((o) => !o);
          setResult(null);
        }}
        className={cn(className, 'cursor-pointer underline decoration-dotted underline-offset-2')}
        style={style}
        title={t('chat.reidentify.hint', {
          defaultValue: 'Anonymised — click to check who this is',
        })}
      >
        {displayName}
      </button>
      {open && (
        <div className="absolute left-0 top-full z-[var(--z-popover)] mt-1 w-64 rounded-md border border-border-subtle bg-bg-elevated p-2 shadow-lg">
          <p className="mb-1.5 text-xs text-text-secondary">
            {t('chat.reidentify.prompt', {
              defaultValue: 'Confirm a name you already suspect — the alias can’t be reversed.',
            })}
          </p>
          <div className="flex items-center gap-1.5">
            <Input
              value={candidate}
              onChange={(e) => setCandidate(e.target.value)}
              placeholder={t('chat.reidentify.placeholder', { defaultValue: 'Suspected username' })}
              onKeyDown={(e) => {
                if (e.key === 'Enter') void check();
              }}
            />
            <Button
              variant="primary"
              size="sm"
              onClick={() => void check()}
              disabled={checking || candidate.trim().length === 0}
              aria-label={t('chat.reidentify.check', { defaultValue: 'Check' })}
            >
              <Search className="h-3.5 w-3.5" />
            </Button>
          </div>
          {result === 'match' && (
            <p className="mt-1.5 text-xs text-success-text">
              {t('chat.reidentify.match', {
                defaultValue: 'Match — this is {{name}}.',
                name: candidate.trim(),
              })}
            </p>
          )}
          {result === 'noMatch' && (
            <p className="mt-1.5 text-xs text-text-tertiary">
              {t('chat.reidentify.noMatch', { defaultValue: 'No match.' })}
            </p>
          )}
        </div>
      )}
    </span>
  );
}
