import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Select, SelectOption } from '@/components/ui/Select';
import { encoderPresets } from '@/lib/encoderPresets';

export interface ContainerFormValues {
  containerFormat: string;
}

interface ContainerSettingsFormProps {
  values: ContainerFormValues;
  onChange: <K extends keyof ContainerFormValues>(field: K, value: ContainerFormValues[K]) => void;
}

export function ContainerSettingsForm({
  values,
  onChange,
}: ContainerSettingsFormProps): React.ReactElement {
  const { t } = useTranslation();

  const containerFormatOptions: SelectOption[] = useMemo(
    () =>
      encoderPresets.CONTAINER_FORMAT_VALUES.map((value) => ({
        value,
        label: value.toUpperCase(),
      })),
    []
  );

  return (
    <div className="p-3 bg-bg-muted rounded-lg">
      <div className="mb-3 text-sm font-medium text-text-primary">
        {t('modals.containerSettings')}
      </div>
      <Select
        label={t('modals.containerFormat')}
        value={values.containerFormat}
        onChange={(e) => onChange('containerFormat', e.target.value)}
        options={containerFormatOptions}
      />
    </div>
  );
}
