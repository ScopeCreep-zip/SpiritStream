import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import { useProfileStore } from '@/stores/profileStore';
import type { StreamTarget, Platform, OutputGroup } from '@spiritstream/types';
import { PLATFORMS as platformConfig } from '@/lib/profile-helpers';
import { PasswordInput, useFormState, useFormValidation } from '@spiritstream/ui';
import type { ValidationRule } from '@spiritstream/ui';

export interface TargetModalProps {
  open: boolean;
  onClose: () => void;
  mode: 'create' | 'edit';
  groupId: string;
  target?: StreamTarget;
  /**
   * In create mode: pre-fill the service dropdown (and the corresponding
   * default URL + suggested name) with this platform. The AppDrawer hands
   * this in after the user picks a service from the catalog. Ignored in
   * edit mode.
   */
  initialService?: Platform;
}

// Platform values - dynamically loaded from JSON
const PLATFORM_VALUES: Platform[] = Object.keys(platformConfig) as Platform[];
const DEFAULT_PLATFORM: Platform = 'YouTube - RTMPS';

interface FormData {
  service: Platform;
  name: string;
  url: string;
  streamKey: string;
}

const defaultFormData: FormData = {
  service: DEFAULT_PLATFORM,
  name: '',
  url: platformConfig[DEFAULT_PLATFORM].defaultServer,
  streamKey: '',
};

export function TargetModal({
  open,
  onClose,
  mode,
  groupId,
  target,
  initialService,
}: TargetModalProps) {
  const { t } = useTranslation();
  const { current, addStreamTarget, updateStreamTarget, moveStreamTarget } = useProfileStore();
  const form = useFormState<FormData>(defaultFormData);
  const formData = form.values;
  const [saving, setSaving] = useState(false);
  const [selectedGroupId, setSelectedGroupId] = useState(groupId);
  const [originalGroupId, setOriginalGroupId] = useState(groupId);
  const [serverError, setServerError] = useState<string | undefined>();

  const rules: Partial<Record<keyof FormData, ValidationRule<FormData>>> = {
    name: (v) => (!v.name.trim() ? t('validation.targetNameRequired') : null),
    url: (v) => (!v.url.trim() ? t('validation.serverUrlRequired') : null),
    streamKey: (v) => (!v.streamKey.trim() ? t('validation.streamKeyRequired') : null),
    // The `rtmp(s)://` prefix check used to live here. It now runs
    // server-side in `PlatformRegistry::normalize_url` during profile
    // save, which returns a `ValidationIssue` the modal's existing
    // error-display path renders — same UX, single source of truth.
  };
  const { errors, validate, clear: clearErrors } = useFormValidation<FormData>(formData, rules);

  // Get output groups from current profile
  const outputGroups = current?.outputGroups ?? [];

  // Initialize form data when modal opens or target changes
  useEffect(() => {
    if (open) {
      if (mode === 'edit' && target) {
        form.reset({
          service: target.service,
          name: target.name,
          url: target.url,
          streamKey: target.streamKey,
        });
      } else if (mode === 'create' && initialService) {
        // AppDrawer handoff: pre-fill the service + its default URL + a
        // suggested name. User still confirms / overrides everything before save.
        form.reset({
          service: initialService,
          name: platformConfig[initialService].displayName,
          url: platformConfig[initialService].defaultServer,
          streamKey: '',
        });
      } else {
        form.reset(defaultFormData);
      }
      setSelectedGroupId(groupId);
      setOriginalGroupId(groupId);
      clearErrors();
      setServerError(undefined);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, mode, target, groupId, initialService]);

  // Update URL when service changes (only in create mode)
  const handleServiceChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const newService = e.target.value as Platform;
    form.merge({
      service: newService,
      url: mode === 'create' ? platformConfig[newService].defaultServer : formData.url,
      name: formData.name || platformConfig[newService].displayName,
    });
  };

  const handleSave = async () => {
    if (!validate()) return;
    setServerError(undefined);

    setSaving(true);
    try {
      // URL normalization happens server-side inside `ProfileService::save`
      // via `PlatformRegistry::normalize_url`; the frontend used to do its own
      // trim/strip-trailing-slash here, but that was a stale duplicate that
      // could drift from the platform registry's per-host rules.
      const targetData: StreamTarget = {
        id: mode === 'edit' && target ? target.id : crypto.randomUUID(),
        service: formData.service,
        name: formData.name,
        url: formData.url.trim(),
        streamKey: formData.streamKey.trim(),
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
      setServerError(String(error));
    } finally {
      setSaving(false);
    }
  };

  const handleChange =
    (field: keyof FormData) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
      form.set(field, e.target.value as FormData[typeof field]);
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
            {(() => {
              if (saving) return t('common.saving');
              if (mode === 'create') return t('modals.addTarget');
              return t('common.saveChanges');
            })()}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        {serverError && (
          <div className="p-3 rounded-lg bg-error-subtle border border-error-border text-error-text text-sm">
            {serverError}
          </div>
        )}
        {/* Output Group Selector */}
        <Select
          label={t('modals.outputGroupLabel')}
          value={selectedGroupId}
          onChange={(e: React.ChangeEvent<HTMLSelectElement>) => setSelectedGroupId(e.target.value)}
          options={outputGroupOptions}
          disabled={outputGroups.length === 0}
          helper={outputGroups.length === 0 ? t('modals.noOutputGroupsAvailable') : undefined}
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
          placeholder={t('modals.target.serverUrlPlaceholder')}
          value={formData.url}
          onChange={handleChange('url')}
          error={errors.url}
          helper={`${t('modals.default')}: ${platformConfig[formData.service].defaultServer}`}
        />

        <PasswordInput
          label={t('targets.streamKey')}
          placeholder={t('modals.streamKeyPlaceholder')}
          value={formData.streamKey}
          onChange={handleChange('streamKey')}
          error={errors.streamKey}
          helper={t('modals.streamKeyHelper')}
          autoComplete="off"
          showLabel={t('common.showStreamKey')}
          hideLabel={t('common.hideStreamKey')}
          renderToggleIcon={(visible) => (
            <span className="text-xs">{visible ? t('common.hide') : t('common.show')}</span>
          )}
        />
      </div>
    </Modal>
  );
}
