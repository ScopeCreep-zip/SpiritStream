import { useState, useEffect } from 'react';
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
        <div className="text-[var(--text-secondary)]">Loading...</div>
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

  if (!current || current.outputGroups.length === 0) {
    return (
      <Card>
        <CardBody>
          <div className="text-center py-12">
            <p className="text-[var(--text-secondary)]">
              Please create an output group first to configure encoder settings.
            </p>
          </div>
        </CardBody>
      </Card>
    );
  }

  const encoderOptions = [
    { value: 'libx264', label: 'x264 (Software)' },
    { value: 'h264_nvenc', label: 'NVENC (NVIDIA)' },
    { value: 'h264_qsv', label: 'QuickSync (Intel)' },
    { value: 'h264_amf', label: 'AMF (AMD)' },
  ];

  const presetOptions = [
    { value: 'quality', label: 'Quality' },
    { value: 'balanced', label: 'Balanced' },
    { value: 'performance', label: 'Performance' },
    { value: 'low_latency', label: 'Low Latency' },
  ];

  const rateControlOptions = [
    { value: 'cbr', label: 'CBR (Constant Bitrate)' },
    { value: 'vbr', label: 'VBR (Variable Bitrate)' },
    { value: 'cqp', label: 'CQP (Constant Quality)' },
  ];

  const resolutionOptions = [
    { value: '3840x2160', label: '4K (3840x2160)' },
    { value: '2560x1440', label: '1440p (2560x1440)' },
    { value: '1920x1080', label: '1080p (1920x1080)' },
    { value: '1280x720', label: '720p (1280x720)' },
    { value: '854x480', label: '480p (854x480)' },
  ];

  const frameRateOptions = [
    { value: '60', label: '60 FPS' },
    { value: '30', label: '30 FPS' },
    { value: '24', label: '24 FPS' },
  ];

  const outputGroupOptions = current.outputGroups.map(g => ({
    value: g.id,
    label: g.name || `Output Group`,
  }));

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>Encoder Configuration</CardTitle>
          <CardDescription>
            Configure video and audio encoding settings for your streams
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
              Video Encoder
            </h3>
            <Select
              label="Encoder"
              value={formData.encoder}
              onChange={(e) => handleChange('encoder', e.target.value)}
              options={encoderOptions}
              helper="Select your preferred hardware or software encoder"
            />
            <Select
              label="Preset"
              value={formData.preset}
              onChange={(e) => handleChange('preset', e.target.value)}
              options={presetOptions}
              helper="Balance between quality and encoding speed"
            />
            <Select
              label="Rate Control"
              value={formData.rateControl}
              onChange={(e) => handleChange('rateControl', e.target.value)}
              options={rateControlOptions}
              helper="CBR recommended for streaming"
            />
          </div>

          {/* Right Column - Output Settings */}
          <div className="flex flex-col" style={{ gap: '16px' }}>
            <h3 className="text-sm font-semibold text-[var(--text-primary)]" style={{ marginBottom: '16px' }}>
              Output Settings
            </h3>
            <Select
              label="Resolution"
              value={formData.resolution}
              onChange={(e) => handleChange('resolution', e.target.value)}
              options={resolutionOptions}
            />
            <Select
              label="Frame Rate"
              value={formData.frameRate}
              onChange={(e) => handleChange('frameRate', e.target.value)}
              options={frameRateOptions}
            />
            <Input
              label="Video Bitrate (kbps)"
              type="number"
              value={formData.videoBitrate}
              onChange={(e) => handleChange('videoBitrate', e.target.value)}
              helper="Recommended: 4500-9000 for 1080p60"
            />
            <Input
              label="Keyframe Interval (seconds)"
              type="number"
              value={formData.keyframeInterval}
              onChange={(e) => handleChange('keyframeInterval', e.target.value)}
              helper="Most platforms require 2 seconds"
            />
          </div>
        </div>
      </CardBody>
      <CardFooter>
        <Button variant="ghost" onClick={handleReset}>
          <RotateCcw className="w-4 h-4" />
          Reset to Defaults
        </Button>
        <Button onClick={handleSave} disabled={!isDirty}>
          <Save className="w-4 h-4" />
          Save Settings
        </Button>
      </CardFooter>
    </Card>
  );
}
