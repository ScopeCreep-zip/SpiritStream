import { useState, useEffect } from 'react';
import { Modal } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Toggle } from '@/components/ui/Toggle';
import { Button } from '@/components/ui/Button';
import { useProfileStore } from '@/stores/profileStore';
import { api } from '@/lib/tauri';
import type { OutputGroup } from '@/types/profile';
import type { Encoders } from '@/types/stream';

export interface OutputGroupModalProps {
  open: boolean;
  onClose: () => void;
  mode: 'create' | 'edit';
  group?: OutputGroup;
}

// Resolution options
const RESOLUTION_OPTIONS: SelectOption[] = [
  { value: '1920x1080', label: '1080p (1920x1080)' },
  { value: '1280x720', label: '720p (1280x720)' },
  { value: '2560x1440', label: '1440p (2560x1440)' },
  { value: '3840x2160', label: '4K (3840x2160)' },
  { value: '854x480', label: '480p (854x480)' },
];

// Frame rate options
const FPS_OPTIONS: SelectOption[] = [
  { value: '60', label: '60 fps' },
  { value: '30', label: '30 fps' },
  { value: '24', label: '24 fps' },
  { value: '25', label: '25 fps' },
  { value: '50', label: '50 fps' },
];

// Audio bitrate options
const AUDIO_BITRATE_OPTIONS: SelectOption[] = [
  { value: '320', label: '320 kbps (Best)' },
  { value: '256', label: '256 kbps' },
  { value: '192', label: '192 kbps' },
  { value: '128', label: '128 kbps (Standard)' },
  { value: '96', label: '96 kbps' },
  { value: '64', label: '64 kbps (Low)' },
];

interface FormData {
  name: string;
  videoEncoder: string;
  resolution: string;
  fps: string;
  videoBitrate: string;
  audioCodec: string;
  audioBitrate: string;
  generatePts: boolean;
}

const defaultFormData: FormData = {
  name: '',
  videoEncoder: 'libx264',
  resolution: '1920x1080',
  fps: '60',
  videoBitrate: '6000',
  audioCodec: 'aac',
  audioBitrate: '128',
  generatePts: false,
};

export function OutputGroupModal({ open, onClose, mode, group }: OutputGroupModalProps) {
  const { addOutputGroup, updateOutputGroup, saveProfile } = useProfileStore();
  const [formData, setFormData] = useState<FormData>(defaultFormData);
  const [errors, setErrors] = useState<Partial<FormData>>({});
  const [saving, setSaving] = useState(false);
  const [encoders, setEncoders] = useState<Encoders>({ video: ['libx264'], audio: ['aac'] });
  const [loadingEncoders, setLoadingEncoders] = useState(false);

  // Load available encoders when modal opens
  useEffect(() => {
    if (open) {
      setLoadingEncoders(true);
      api.system.getEncoders()
        .then((enc) => {
          setEncoders(enc);
          // If no encoder set yet, use first available
          if (mode === 'create' && enc.video.length > 0) {
            setFormData((prev) => ({
              ...prev,
              videoEncoder: enc.video[0],
              audioCodec: enc.audio[0] || 'aac',
            }));
          }
        })
        .catch((err) => {
          console.error('Failed to load encoders:', err);
        })
        .finally(() => {
          setLoadingEncoders(false);
        });
    }
  }, [open, mode]);

  // Initialize form data when modal opens or group changes
  useEffect(() => {
    if (open) {
      if (mode === 'edit' && group) {
        setFormData({
          name: group.name || '',
          videoEncoder: group.videoEncoder,
          resolution: group.resolution,
          fps: String(group.fps),
          videoBitrate: String(group.videoBitrate),
          audioCodec: group.audioCodec,
          audioBitrate: String(group.audioBitrate),
          generatePts: group.generatePts,
        });
      } else {
        setFormData(defaultFormData);
      }
      setErrors({});
    }
  }, [open, mode, group]);

  // Create encoder options from loaded encoders
  const videoEncoderOptions: SelectOption[] = encoders.video.map((enc) => {
    // Provide friendly names for common encoders
    const labels: Record<string, string> = {
      libx264: 'x264 (Software)',
      h264_nvenc: 'NVENC (NVIDIA)',
      h264_videotoolbox: 'VideoToolbox (Apple)',
      h264_qsv: 'QuickSync (Intel)',
      h264_amf: 'AMF (AMD)',
    };
    return { value: enc, label: labels[enc] || enc };
  });

  const audioEncoderOptions: SelectOption[] = encoders.audio.map((enc) => {
    const labels: Record<string, string> = {
      aac: 'AAC',
      libmp3lame: 'MP3',
      libopus: 'Opus',
    };
    return { value: enc, label: labels[enc] || enc };
  });

  const validate = (): boolean => {
    const newErrors: Partial<FormData> = {};

    if (!formData.name.trim()) {
      newErrors.name = 'Output group name is required';
    }

    const bitrate = parseInt(formData.videoBitrate);
    if (isNaN(bitrate) || bitrate < 500 || bitrate > 50000) {
      newErrors.videoBitrate = 'Bitrate must be between 500 and 50000 kbps';
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSave = async () => {
    if (!validate()) return;

    setSaving(true);
    try {
      const groupData: OutputGroup = {
        id: mode === 'edit' && group ? group.id : crypto.randomUUID(),
        name: formData.name,
        videoEncoder: formData.videoEncoder,
        resolution: formData.resolution,
        fps: parseInt(formData.fps),
        videoBitrate: parseInt(formData.videoBitrate),
        audioCodec: formData.audioCodec,
        audioBitrate: parseInt(formData.audioBitrate),
        generatePts: formData.generatePts,
        streamTargets: mode === 'edit' && group ? group.streamTargets : [],
      };

      if (mode === 'create') {
        addOutputGroup(groupData);
      } else if (mode === 'edit' && group) {
        updateOutputGroup(group.id, groupData);
      }

      // Save to backend
      await saveProfile();
      onClose();
    } catch (error) {
      setErrors({ name: String(error) });
    } finally {
      setSaving(false);
    }
  };

  const handleChange = (field: keyof FormData) => (
    e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>
  ) => {
    setFormData((prev) => ({ ...prev, [field]: e.target.value }));
    // Clear error when user starts typing
    if (errors[field]) {
      setErrors((prev) => ({ ...prev, [field]: undefined }));
    }
  };

  const title = mode === 'create' ? 'Create Output Group' : 'Edit Output Group';

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={title}
      maxWidth="600px"
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={saving}>
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={saving || loadingEncoders}>
            {saving ? 'Saving...' : mode === 'create' ? 'Create Group' : 'Save Changes'}
          </Button>
        </>
      }
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
        <Input
          label="Output Group Name"
          placeholder="e.g., High Quality Stream"
          value={formData.name}
          onChange={handleChange('name')}
          error={errors.name}
        />

        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
          <Select
            label="Video Encoder"
            value={formData.videoEncoder}
            onChange={handleChange('videoEncoder')}
            options={videoEncoderOptions}
            disabled={loadingEncoders}
          />

          <Select
            label="Audio Codec"
            value={formData.audioCodec}
            onChange={handleChange('audioCodec')}
            options={audioEncoderOptions}
            disabled={loadingEncoders}
          />
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
          <Select
            label="Resolution"
            value={formData.resolution}
            onChange={handleChange('resolution')}
            options={RESOLUTION_OPTIONS}
          />

          <Select
            label="Frame Rate"
            value={formData.fps}
            onChange={handleChange('fps')}
            options={FPS_OPTIONS}
          />
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
          <Input
            label="Video Bitrate (kbps)"
            type="number"
            placeholder="6000"
            value={formData.videoBitrate}
            onChange={handleChange('videoBitrate')}
            error={errors.videoBitrate}
            helper="Recommended: 4500-6000 for 1080p60"
          />

          <Select
            label="Audio Bitrate"
            value={formData.audioBitrate}
            onChange={handleChange('audioBitrate')}
            options={AUDIO_BITRATE_OPTIONS}
          />
        </div>

        <div style={{ paddingTop: '8px' }}>
          <Toggle
            label="Generate PTS Timestamps"
            description="Add timestamp generation for compatibility with some platforms"
            checked={formData.generatePts}
            onChange={(checked) => setFormData((prev) => ({ ...prev, generatePts: checked }))}
          />
        </div>
      </div>
    </Modal>
  );
}
