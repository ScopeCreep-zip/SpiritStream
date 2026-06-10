import React from 'react';
import { useTranslation } from 'react-i18next';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { cn } from '@/lib/cn';
import type { ObsIntegrationDirection } from '@spiritstream/types';

const DIRECTION_OPTIONS: ReadonlyArray<{
  value: ObsIntegrationDirection;
  labelKey: string;
  descKey: string;
}> = [
  {
    value: 'disabled',
    labelKey: 'obs.directions.disabled',
    descKey: 'obs.directions.disabledDescription',
  },
  {
    value: 'obs-to-spiritstream',
    labelKey: 'obs.directions.obsToSpiritstream',
    descKey: 'obs.directions.obsToSpiritstreamDescription',
  },
  {
    value: 'spiritstream-to-obs',
    labelKey: 'obs.directions.spiritstreamToObs',
    descKey: 'obs.directions.spiritstreamToObsDescription',
  },
  {
    value: 'bidirectional',
    labelKey: 'obs.directions.bidirectional',
    descKey: 'obs.directions.bidirectionalDescription',
  },
];

interface ObsDirectionSelectorProps {
  direction: ObsIntegrationDirection;
  onSelect: (direction: ObsIntegrationDirection) => void;
}

export function ObsDirectionSelector({
  direction,
  onSelect,
}: ObsDirectionSelectorProps): React.ReactElement {
  const { t } = useTranslation();

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('obs.integration')}</CardTitle>
        <CardDescription>{t('obs.integrationDirectionDescription')}</CardDescription>
      </CardHeader>
      <CardBody>
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
          {DIRECTION_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              onClick={() => onSelect(option.value)}
              className={cn(
                'p-4 rounded-lg border text-left transition-all cursor-pointer',
                direction === option.value
                  ? 'border-primary bg-primary/10'
                  : 'border-border-default bg-bg-base hover:border-border-strong'
              )}
            >
              <div className="text-sm font-medium text-text-primary">
                {t(option.labelKey as 'obs.directions.disabled')}
              </div>
              <div className="text-xs text-text-tertiary mt-1">
                {t(option.descKey as 'obs.directions.disabledDescription')}
              </div>
            </button>
          ))}
        </div>
      </CardBody>
    </Card>
  );
}
