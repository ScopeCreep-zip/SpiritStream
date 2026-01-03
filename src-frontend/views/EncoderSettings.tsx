import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { RotateCcw, Save } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody, CardFooter } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { Input } from '@/components/ui/Input';
import { useProfileStore } from '@/stores/profileStore';

interface EncoderFormData {
  encoder: string;
  preset: string;
  rateControl: string;
  resolution: string;
  frameRate: string;
  videoBitrate: string;
  keyframeInterval: string;
}

const defaultSettings: EncoderFormData = {
  encoder: 'libx264',
  preset: 'balanced',
  rateControl: 'cbr',
  resolution: '1920x1080',
  frameRate: '60',
  videoBitrate: '6000',
  keyframeInterval: '2',
};

export function EncoderSettings() {
  const { t } = useTranslation();
  const { current, loading, error, updateOutputGroup } = useProfileStore();
  const [formData, setFormData] = useState<EncoderFormData>(defaultSettings);
  const [isDirty, setIsDirty] = useState(false);
  const [selectedGroupId, setSelectedGroupId] = useState<string | null>(null);

  // Load settings from first output group (or selected group)
  useEffect(() => {
    if (current && current.outputGroups.length > 0) {
      const group = selectedGroupId
        ? current.outputGroups.find(g => g.id === selectedGroupId)
        : current.outputGroups[0];

      if (group) {
        setSelectedGroupId(group.id);
        setFormData({
          encoder: group.videoEncoder,
          preset: 'balanced', // Not stored in current model
          rateControl: 'cbr', // Not stored in current model
          resolution: group.resolution,
          frameRate: group.fps.toString(),
          videoBitrate: group.videoBitrate.toString(),
          keyframeInterval: '2', // Not stored in current model
        });
        setIsDirty(false);
      }
    }
  }, [current, selectedGroupId]);

  const handleChange = (field: keyof EncoderFormData, value: string) => {
    setFormData(prev => ({ ...prev, [field]: value }));
    setIsDirty(true);
  };

  const handleReset = () => {
    setFormData(defaultSettings);
    setIsDirty(true);
  };

  const handleSave = () => {
    if (selectedGroupId) {
      updateOutputGroup(selectedGroupId, {
        videoEncoder: formData.encoder,
        resolution: formData.resolution,
        fps: parseInt(formData.frameRate),
        videoBitrate: parseInt(formData.videoBitrate),
      });
      setIsDirty(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--text-secondary)]">{t('common.loading')}</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--error-text)]">{t('common.error')}: {error}</div>
      </div>
    );
  }

  if (!current || current.outputGroups.length === 0) {
    return (
      <Card>
        <CardBody>
          <div className="text-center py-12">
            <p className="text-[var(--text-secondary)]">
              {t('encoder.createOutputGroupFirst')}
            </p>
          </div>
        </CardBody>
      </Card>
    );
  }

  const encoderOptions = [
    { value: 'libx264', label: t('encoder.encoders.libx264') },
    { value: 'h264_nvenc', label: t('encoder.encoders.h264_nvenc') },
    { value: 'h264_qsv', label: t('encoder.encoders.h264_qsv') },
    { value: 'h264_amf', label: t('encoder.encoders.h264_amf') },
  ];

  const presetOptions = [
    { value: 'quality', label: t('encoder.presets.quality') },
    { value: 'balanced', label: t('encoder.presets.balanced') },
    { value: 'performance', label: t('encoder.presets.performance') },
    { value: 'low_latency', label: t('encoder.presets.lowLatency') },
  ];

  const rateControlOptions = [
    { value: 'cbr', label: t('encoder.rateControls.cbr') },
    { value: 'vbr', label: t('encoder.rateControls.vbr') },
    { value: 'cqp', label: t('encoder.rateControls.cqp') },
  ];

  const resolutionOptions = [
    { value: '3840x2160', label: t('encoder.resolutions.3840x2160') },
    { value: '2560x1440', label: t('encoder.resolutions.2560x1440') },
    { value: '1920x1080', label: t('encoder.resolutions.1920x1080') },
    { value: '1280x720', label: t('encoder.resolutions.1280x720') },
    { value: '854x480', label: t('encoder.resolutions.854x480') },
  ];

  const frameRateOptions = [
    { value: '60', label: t('encoder.frameRates.60') },
    { value: '30', label: t('encoder.frameRates.30') },
    { value: '24', label: t('encoder.frameRates.24') },
  ];

  const outputGroupOptions = current.outputGroups.map(g => ({
    value: g.id,
    label: g.name || t('encoder.defaultGroupName'),
  }));

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>{t('encoder.title')}</CardTitle>
          <CardDescription>
            {t('encoder.description')}
          </CardDescription>
        </div>
        {current.outputGroups.length > 1 && (
          <Select
            value={selectedGroupId || ''}
            onChange={(e) => setSelectedGroupId(e.target.value)}
            options={outputGroupOptions}
            className="w-48"
          />
        )}
      </CardHeader>
      <CardBody>
        <div className="grid grid-cols-2" style={{ gap: '24px' }}>
          {/* Left Column - Video Encoder */}
          <div className="flex flex-col" style={{ gap: '16px' }}>
            <h3 className="text-sm font-semibold text-[var(--text-primary)]" style={{ marginBottom: '16px' }}>
              {t('encoder.videoEncoder')}
            </h3>
            <Select
              label={t('encoder.encoder')}
              value={formData.encoder}
              onChange={(e) => handleChange('encoder', e.target.value)}
              options={encoderOptions}
              helper={t('encoder.encoderHelper')}
            />
            <Select
              label={t('encoder.preset')}
              value={formData.preset}
              onChange={(e) => handleChange('preset', e.target.value)}
              options={presetOptions}
              helper={t('encoder.presetHelper')}
            />
            <Select
              label={t('encoder.rateControl')}
              value={formData.rateControl}
              onChange={(e) => handleChange('rateControl', e.target.value)}
              options={rateControlOptions}
              helper={t('encoder.rateControlHelper')}
            />
          </div>

          {/* Right Column - Output Settings */}
          <div className="flex flex-col" style={{ gap: '16px' }}>
            <h3 className="text-sm font-semibold text-[var(--text-primary)]" style={{ marginBottom: '16px' }}>
              {t('encoder.outputSettings')}
            </h3>
            <Select
              label={t('encoder.resolution')}
              value={formData.resolution}
              onChange={(e) => handleChange('resolution', e.target.value)}
              options={resolutionOptions}
            />
            <Select
              label={t('encoder.frameRate')}
              value={formData.frameRate}
              onChange={(e) => handleChange('frameRate', e.target.value)}
              options={frameRateOptions}
            />
            <Input
              label={t('encoder.videoBitrate')}
              type="number"
              value={formData.videoBitrate}
              onChange={(e) => handleChange('videoBitrate', e.target.value)}
              helper={t('encoder.videoBitrateHelper')}
            />
            <Input
              label={t('encoder.keyframeInterval')}
              type="number"
              value={formData.keyframeInterval}
              onChange={(e) => handleChange('keyframeInterval', e.target.value)}
              helper={t('encoder.keyframeIntervalHelper')}
            />
          </div>
        </div>
      </CardBody>
      <CardFooter>
        <Button variant="ghost" onClick={handleReset}>
          <RotateCcw className="w-4 h-4" />
          {t('encoder.resetDefaults')}
        </Button>
        <Button onClick={handleSave} disabled={!isDirty}>
          <Save className="w-4 h-4" />
          {t('encoder.saveSettings')}
        </Button>
      </CardFooter>
    </Card>
  );
}
