import { useState } from 'react';
import { Plus, Eye, EyeOff, Copy, Pencil, Trash2 } from 'lucide-react';
import { Grid } from '@/components/ui/Grid';
import { Card, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { PlatformIcon } from '@/components/stream/PlatformIcon';
import { useProfileStore } from '@/stores/profileStore';
import type { Platform, StreamTarget } from '@/types/profile';

export function StreamTargets() {
  const { current, loading, error, removeStreamTarget } = useProfileStore();
  const [revealedKeys, setRevealedKeys] = useState<Set<string>>(new Set());

  // Get all stream targets from current profile
  const getAllTargets = (): Array<StreamTarget & { groupId: string }> => {
    if (!current) return [];
    return current.outputGroups.flatMap(group =>
      group.streamTargets.map(target => ({
        ...target,
        groupId: group.id,
      }))
    );
  };

  const targets = getAllTargets();

  const toggleKeyVisibility = (targetId: string) => {
    const newRevealed = new Set(revealedKeys);
    if (newRevealed.has(targetId)) {
      newRevealed.delete(targetId);
    } else {
      newRevealed.add(targetId);
    }
    setRevealedKeys(newRevealed);
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
    // TODO: Show toast notification
  };

  const maskKey = (key: string): string => {
    if (key.length <= 8) return '••••••••';
    return key.substring(0, 4) + '••••••••' + key.substring(key.length - 4);
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--text-secondary)]">Loading targets...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--error-text)]">Error: {error}</div>
      </div>
    );
  }

  if (!current) {
    return (
      <Card>
        <CardBody>
          <div className="text-center py-12">
            <p className="text-[var(--text-secondary)]">
              Please select a profile first to manage stream targets.
            </p>
          </div>
        </CardBody>
      </Card>
    );
  }

  if (targets.length === 0) {
    return (
      <Card>
        <CardBody>
          <div className="text-center" style={{ padding: '48px 0' }}>
            <div className="w-16 h-16 mx-auto rounded-full bg-[var(--primary-subtle)] flex items-center justify-center" style={{ marginBottom: '16px' }}>
              <Plus className="w-8 h-8 text-[var(--primary)]" />
            </div>
            <h3 className="text-lg font-semibold text-[var(--text-primary)]" style={{ marginBottom: '8px' }}>
              No Stream Targets
            </h3>
            <p className="text-[var(--text-secondary)] max-w-md mx-auto" style={{ marginBottom: '24px' }}>
              Add your first streaming destination. Supports YouTube, Twitch, Kick, Facebook, and custom RTMP servers.
            </p>
            <Button>
              <Plus className="w-4 h-4" />
              Add Stream Target
            </Button>
          </div>
        </CardBody>
      </Card>
    );
  }

  return (
    <Grid cols={2}>
      {targets.map((target) => (
        <Card key={target.id}>
          <CardBody>
            <div className="flex items-start justify-between" style={{ marginBottom: '16px' }}>
              <div className="flex items-center" style={{ gap: '12px' }}>
                <PlatformIcon platform={target.platform as Platform} size="lg" />
                <div>
                  <h3 className="font-semibold text-[var(--text-primary)]">{target.name}</h3>
                  <p className="text-sm text-[var(--text-secondary)]">{target.url}</p>
                </div>
              </div>
              <div className="flex" style={{ gap: '4px' }}>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="Edit target"
                  onClick={() => alert('Edit target modal not yet implemented')}
                >
                  <Pencil className="w-4 h-4" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="Delete target"
                  onClick={() => removeStreamTarget(target.groupId, target.id)}
                >
                  <Trash2 className="w-4 h-4" />
                </Button>
              </div>
            </div>

            <div className="flex flex-col" style={{ gap: '6px' }}>
              <label className="block text-sm font-medium text-[var(--text-primary)]">
                Stream Key
              </label>
              <div className="flex" style={{ gap: '8px' }}>
                <Input
                  type={revealedKeys.has(target.id) ? 'text' : 'password'}
                  value={revealedKeys.has(target.id) ? target.streamKey : maskKey(target.streamKey)}
                  readOnly
                  className="flex-1 font-mono text-sm"
                />
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => toggleKeyVisibility(target.id)}
                  aria-label={revealedKeys.has(target.id) ? 'Hide stream key' : 'Show stream key'}
                >
                  {revealedKeys.has(target.id) ? (
                    <EyeOff className="w-4 h-4" />
                  ) : (
                    <Eye className="w-4 h-4" />
                  )}
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => copyToClipboard(target.streamKey)}
                  aria-label="Copy stream key"
                >
                  <Copy className="w-4 h-4" />
                </Button>
              </div>
            </div>
          </CardBody>
        </Card>
      ))}

      {/* Add New Target Card */}
      <Card
        className="border-2 border-dashed border-[var(--border-default)] hover:border-[var(--primary)] transition-colors cursor-pointer"
        onClick={() => alert('Add target modal not yet implemented')}
      >
        <CardBody className="flex flex-col items-center justify-center" style={{ padding: '48px 24px' }}>
          <div className="w-12 h-12 rounded-full bg-[var(--primary-subtle)] flex items-center justify-center" style={{ marginBottom: '12px' }}>
            <Plus className="w-6 h-6 text-[var(--primary)]" />
          </div>
          <span className="text-sm font-medium text-[var(--text-secondary)]">
            Add New Target
          </span>
        </CardBody>
      </Card>
    </Grid>
  );
}
