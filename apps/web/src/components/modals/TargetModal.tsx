import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import { useProfileStore } from '@/stores/profileStore';
import type { StreamTarget, Platform, OutputGroup } from '@/types/profile';
import { PLATFORMS as platformConfig } from '@/types/profile';

export interface TargetModalProps {
  open: boolean;
  onClose: () => void;
  mode: 'create' | 'edit';
  groupId: string;
  target?: StreamTarget;
}

// Platform values - dynamically loaded from JSON
const PLATFORM_VALUES: Platform[] = Object.keys(platformConfig) as Platform[];
const FIRST_PLATFORM = PLATFORM_VALUES[0];

interface FormData {
  service: Platform;
  name: string;
  url: string;
  streamKey: string;
}

const defaultFormData: FormData = {
  service: FIRST_PLATFORM,
  name: '',
  url: platformConfig[FIRST_PLATFORM].defaultServer,
  streamKey: '',
};

export function TargetModal({ open, onClose, mode, groupId, target }: TargetModalProps) {
  const { t } = useTranslation();
  const { current, addStreamTarget, updateStreamTarget, moveStreamTarget } = useProfileStore();
  const [formData, setFormData] = useState<FormData>(defaultFormData);
  const [errors, setErrors] = useState<Partial<FormData>>({});
  const [saving, setSaving] = useState(false);
  const [showStreamKey, setShowStreamKey] = useState(false);
  const [selectedGroupId, setSelectedGroupId] = useState(groupId);
  const [originalGroupId, setOriginalGroupId] = useState(groupId);

  // Get output groups from current profile
  const outputGroups = current?.outputGroups ?? [];

  // Initialize form data when modal opens or target changes
  useEffect(() => {
    if (open) {
      if (mode === 'edit' && target) {
        setFormData({
          service: target.service,
          name: target.name,
          url: target.url,
          streamKey: target.streamKey,
        });
      } else {
        setFormData(defaultFormData);
      }
      setSelectedGroupId(groupId);
      setOriginalGroupId(groupId);
      setErrors({});
      setShowStreamKey(false);
    }
  }, [open, mode, target, groupId]);

  // Update URL when service changes (only in create mode)
  const handleServiceChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const newService = e.target.value as Platform;
    setFormData((prev) => ({
      ...prev,
      service: newService,
      // Only update URL if in create mode or URL hasn't been modified
      url: mode === 'create' ? platformConfig[newService].defaultServer : prev.url,
      // Update name suggestion if empty
      name: prev.name || platformConfig[newService].displayName,
    }));
  };

  const validate = (): boolean => {
    const newErrors: Partial<FormData> = {};

    if (!formData.name.trim()) {
      newErrors.name = t('validation.targetNameRequired');
    }

    if (!formData.url.trim()) {
      newErrors.url = t('validation.serverUrlRequired');
    } else if (!formData.url.startsWith('rtmp://') && !formData.url.startsWith('rtmps://')) {
      newErrors.url = t('validation.urlMustStartWithRtmp');
    }

    if (!formData.streamKey.trim()) {
      newErrors.streamKey = t('validation.streamKeyRequired');
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSave = async () => {
    if (!validate()) return;

    setSaving(true);
    try {
      const targetData: StreamTarget = {
        id: mode === 'edit' && target ? target.id : crypto.randomUUID(),
        service: formData.service,
        name: formData.name,
        url: formData.url,
        streamKey: formData.streamKey,
      };

      if (mode === 'create') {
        await addStreamTarget(selectedGroupId, targetData);
      } else if (mode === 'edit' && target) {
        // Check if the group changed - if so, move the target first
        if (selectedGroupId !== originalGroupId) {
          await moveStreamTarget(originalGroupId, selectedGroupId, target.id);
        }
        // Now update the target data in its current group
        await updateStreamTarget(selectedGroupId, target.id, targetData);
      }
      // Note: saveProfile() is called internally by the store functions
      onClose();
    } catch (error) {
      setErrors({ name: String(error) });
    } finally {
      setSaving(false);
    }
  };

  const handleChange =
    (field: keyof FormData) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
      setFormData((prev) => ({ ...prev, [field]: e.target.value }));
      // Clear error when user starts typing
      if (errors[field]) {
        setErrors((prev) => ({ ...prev, [field]: undefined }));
      }
    };

  const title = mode === 'create' ? t('modals.addStreamTarget') : t('modals.editStreamTarget');

  // Create platform options using displayName from PLATFORMS
  const platformOptions: SelectOption[] = PLATFORM_VALUES.map((value) => ({
    value,
    label: platformConfig[value].displayName,
  }));

  // Create output group options
  const outputGroupOptions: SelectOption[] = outputGroups.map((group: OutputGroup) => ({
    value: group.id,
    label: group.name,
  }));

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={title}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={saving}>
            {t('common.cancel')}
          </Button>
          <Button onClick={handleSave} disabled={saving || outputGroups.length === 0}>
            {saving
              ? t('common.saving')
              : mode === 'create'
                ? t('modals.addTarget')
                : t('common.saveChanges')}
          </Button>
        </>
      }
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
        {/* Output Group Selector */}
        <Select
          label={t('modals.outputGroupLabel')}
          value={selectedGroupId}
          onChange={(e: React.ChangeEvent<HTMLSelectElement>) => setSelectedGroupId(e.target.value)}
          options={outputGroupOptions}
          disabled={outputGroups.length === 0}
          helper={
            outputGroups.length === 0
              ? t('modals.noOutputGroupsAvailable')
              : undefined
          }
        />

        <Select
          label={t('modals.platform')}
          value={formData.service}
          onChange={handleServiceChange}
          options={platformOptions}
        />

        <Input
          label={t('modals.targetName')}
          placeholder={t('modals.targetNamePlaceholder')}
          value={formData.name}
          onChange={handleChange('name')}
          error={errors.name}
        />

        <Input
          label={t('modals.serverUrl')}
          placeholder="rtmp://a.rtmp.youtube.com/live2"
          value={formData.url}
          onChange={handleChange('url')}
          error={errors.url}
          helper={`${t('modals.default')}: ${platformConfig[formData.service].defaultServer}`}
        />

        <div style={{ position: 'relative' }}>
          <Input
            label={t('targets.streamKey')}
            type={showStreamKey ? 'text' : 'password'}
            placeholder={t('modals.streamKeyPlaceholder')}
            value={formData.streamKey}
            onChange={handleChange('streamKey')}
            error={errors.streamKey}
            helper={t('modals.streamKeyHelper')}
            autoComplete="off"
          />
          <button
            type="button"
            onClick={() => setShowStreamKey(!showStreamKey)}
            aria-label={showStreamKey ? t('common.hideStreamKey') : t('common.showStreamKey')}
            aria-pressed={showStreamKey}
            style={{
              position: 'absolute',
              right: '12px',
              top: '32px',
              background: 'transparent',
              border: 'none',
              cursor: 'pointer',
              color: 'var(--text-tertiary)',
              fontSize: '12px',
              padding: '4px 8px',
            }}
          >
            {showStreamKey ? t('common.hide') : t('common.show')}
          </button>
        </div>
      </div>
    </Modal>
  );
}
