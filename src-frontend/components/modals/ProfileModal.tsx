import { useState, useEffect } from 'react';
import { Modal } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import { useProfileStore } from '@/stores/profileStore';
import type { Profile } from '@/types/profile';

export interface ProfileModalProps {
  open: boolean;
  onClose: () => void;
  mode: 'create' | 'edit';
  profile?: Profile;
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

interface FormData {
  name: string;
  incomingUrl: string;
  resolution: string;
  fps: string;
  videoBitrate: string;
}

const defaultFormData: FormData = {
  name: '',
  incomingUrl: 'rtmp://localhost/live',
  resolution: '1920x1080',
  fps: '60',
  videoBitrate: '6000',
};

export function ProfileModal({ open, onClose, mode, profile }: ProfileModalProps) {
  const { createProfile, updateProfile, saveProfile, current } = useProfileStore();
  const [formData, setFormData] = useState<FormData>(defaultFormData);
  const [errors, setErrors] = useState<Partial<FormData>>({});
  const [saving, setSaving] = useState(false);

  // Initialize form data when modal opens or profile changes
  useEffect(() => {
    if (open) {
      if (mode === 'edit' && profile) {
        const firstGroup = profile.outputGroups[0];
        setFormData({
          name: profile.name,
          incomingUrl: profile.incomingUrl,
          resolution: firstGroup?.resolution || '1920x1080',
          fps: String(firstGroup?.fps || 60),
          videoBitrate: String(firstGroup?.videoBitrate || 6000),
        });
      } else {
        setFormData(defaultFormData);
      }
      setErrors({});
    }
  }, [open, mode, profile]);

  const validate = (): boolean => {
    const newErrors: Partial<FormData> = {};

    if (!formData.name.trim()) {
      newErrors.name = 'Profile name is required';
    }

    if (!formData.incomingUrl.trim()) {
      newErrors.incomingUrl = 'Incoming URL is required';
    } else if (!formData.incomingUrl.startsWith('rtmp://') && !formData.incomingUrl.startsWith('rtmps://')) {
      newErrors.incomingUrl = 'URL must start with rtmp:// or rtmps://';
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
      if (mode === 'create') {
        // Create new profile
        await createProfile(formData.name);

        // Update with form data (createProfile sets current)
        updateProfile({
          incomingUrl: formData.incomingUrl,
          outputGroups: [{
            id: crypto.randomUUID(),
            name: 'Default Output',
            videoEncoder: 'libx264',
            resolution: formData.resolution,
            videoBitrate: parseInt(formData.videoBitrate),
            fps: parseInt(formData.fps),
            audioCodec: 'aac',
            audioBitrate: 128,
            generatePts: false,
            streamTargets: [],
          }],
        });

        // Save to backend
        await saveProfile();
      } else if (mode === 'edit' && current) {
        // Update existing profile
        updateProfile({
          name: formData.name,
          incomingUrl: formData.incomingUrl,
        });

        // Update first output group if it exists, or create one if none exist
        const firstGroup = current.outputGroups[0];
        if (firstGroup) {
          // Update existing first group with new encoding settings
          updateProfile({
            outputGroups: current.outputGroups.map((g, i) =>
              i === 0
                ? {
                    ...firstGroup,
                    resolution: formData.resolution,
                    fps: parseInt(formData.fps),
                    videoBitrate: parseInt(formData.videoBitrate),
                  }
                : g
            ),
          });
        } else {
          // Create a default output group if none exist
          updateProfile({
            outputGroups: [{
              id: crypto.randomUUID(),
              name: 'Default Output',
              videoEncoder: 'libx264',
              resolution: formData.resolution,
              videoBitrate: parseInt(formData.videoBitrate),
              fps: parseInt(formData.fps),
              audioCodec: 'aac',
              audioBitrate: 128,
              generatePts: false,
              streamTargets: [],
            }],
          });
        }

        // Save to backend
        await saveProfile();
      }

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

  const title = mode === 'create' ? 'Create New Profile' : 'Edit Profile';

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={title}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={saving}>
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={saving}>
            {saving ? 'Saving...' : mode === 'create' ? 'Create Profile' : 'Save Changes'}
          </Button>
        </>
      }
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
        <Input
          label="Profile Name"
          placeholder="e.g., Gaming Stream - High Quality"
          value={formData.name}
          onChange={handleChange('name')}
          error={errors.name}
        />

        <Input
          label="Incoming RTMP URL"
          placeholder="rtmp://localhost/live"
          value={formData.incomingUrl}
          onChange={handleChange('incomingUrl')}
          error={errors.incomingUrl}
          helper="The RTMP source URL from your streaming software (OBS, etc.)"
        />

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

        <Input
          label="Video Bitrate (kbps)"
          type="number"
          placeholder="6000"
          value={formData.videoBitrate}
          onChange={handleChange('videoBitrate')}
          error={errors.videoBitrate}
          helper="Recommended: 4500-6000 for 1080p60, 2500-4000 for 720p60"
        />
      </div>
    </Modal>
  );
}
