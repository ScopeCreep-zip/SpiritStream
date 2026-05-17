import React from 'react';
import { SourceCard } from './SourceCard';
import { EncoderCard } from './EncoderCard';
import { ObsCard } from './ObsCard';
import type { Profile, OutputGroup } from '@spiritstream/types';

interface InputColumnProps {
  profile: Profile | null;
  /** Active group whose encoder details we surface in the EncoderCard. */
  activeGroup: OutputGroup | null;
  /** Source → opens profileEdit modal (RTMP input lives in the profile editor). */
  onConfigureSource: () => void;
  /** Encoder → opens OutputGroupModal in edit mode on the active group. */
  onEditEncoder: (group: OutputGroup) => void;
  /** OBS → opens the OBS configuration modal. */
  onConfigureObs: () => void;
}

export function InputColumn({
  profile,
  activeGroup,
  onConfigureSource,
  onEditEncoder,
  onConfigureObs,
}: InputColumnProps): React.ReactElement {
  return (
    <div className="flex flex-col gap-3">
      <SourceCard profile={profile} onConfigure={onConfigureSource} />
      <EncoderCard
        group={activeGroup}
        onEdit={activeGroup ? () => onEditEncoder(activeGroup) : undefined}
      />
      <ObsCard onConfigure={onConfigureObs} />
    </div>
  );
}
