import React from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { useProfileStore } from '@/stores/profileStore';
import type { NestedSceneSource } from '@/types/source';

interface NestedSceneFormProps {
  data: NestedSceneSource;
  onChange: (data: NestedSceneSource) => void;
}

export const NestedSceneForm = React.memo(({ data, onChange }: NestedSceneFormProps) => {
  const { t } = useTranslation();
  const profile = useProfileStore.getState().current;

  const sceneOptions: SelectOption[] = (profile?.scenes || []).map((s) => ({
    value: s.id,
    label: s.name,
  }));

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Nested Scene"
      />
      <Select
        label={t('stream.referencedScene', { defaultValue: 'Scene to Embed' })}
        value={data.referencedSceneId}
        onChange={(e) => {
          const sceneId = e.target.value;
          const scene = profile?.scenes.find((s) => s.id === sceneId);
          onChange({
            ...data,
            referencedSceneId: sceneId,
            name: data.name || scene?.name || 'Nested Scene',
          });
        }}
        options={sceneOptions}
      />
      <p className="text-xs text-muted">
        {t('stream.nestedSceneHelper', { defaultValue: 'Embeds another scene as a source. Circular references are prevented.' })}
      </p>
    </div>
  );
});

NestedSceneForm.displayName = 'NestedSceneForm';
