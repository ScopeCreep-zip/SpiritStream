import { OutputGroup } from "../models/OutputGroup";
import { StreamTarget } from "../models/StreamTarget";
import { OutputGroupDTO, StreamTargetDTO } from "../shared/interfaces";
import { Theme } from "../models/Theme";
import { ThemeDTO } from "../shared/interfaces";

// Reconstructs a StreamTarget class from a DTO
function reconstructStreamTarget(dto: StreamTargetDTO): StreamTarget {
  return new StreamTarget(dto.id, dto.url, dto.streamKey, dto.rtmpPort);
}

// Reconstructs an OutputGroup class from a DTO
export function reconstructOutputGroups(dtos: OutputGroupDTO[]): OutputGroup[] {
  return dtos.map(dto => {
    const group = new OutputGroup(
      dto.id,
      dto.name,
      dto.videoEncoder,
      dto.resolution,
      dto.bitrate,
      dto.fps,
      dto.audioCodec,
      dto.audioBitrate,
      dto.generatePTS
    );

    dto.streamTargets.forEach(targetDTO => {
      const target = reconstructStreamTarget(targetDTO);
      group.addStreamTarget(target);
    });

    return group;
  });
}

// Reconstructs a Theme class from a DTO
export function reconstructTheme(themeDTO?: ThemeDTO): Theme | undefined {
  if (!themeDTO) return undefined;

  return new Theme(
    themeDTO.id,
    themeDTO.name,
    themeDTO.primaryColor,
    themeDTO.secondaryColor,
    themeDTO.backgroundColor,
    themeDTO.textColor,
    themeDTO.darkMode
  );
}
