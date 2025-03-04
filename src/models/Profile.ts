// Profile.ts

import { OutputGroup } from "./OutputGroup";
import { Theme } from "./Theme";

export class Profile {
  id: string;
  name: string;
  incomingURL: string;
  generatePTS: boolean;
  outputGroups: OutputGroup[];
  theme?: Theme;

  constructor(id: string, name: string, incomingURL: string, generatePTS: boolean, theme?: Theme) {
    this.id = id;
    this.name = name;
    this.incomingURL = incomingURL;
    this.generatePTS = generatePTS;
    this.outputGroups = [];
    this.theme = theme;
  }

  // Add an output group
  addOutputGroup(group: OutputGroup) {
    this.outputGroups.push(group);
  }

  // Remove an output group by ID
  removeOutputGroup(groupId: string) {
    this.outputGroups = this.outputGroups.filter((g) => g.id !== groupId);
  }
}
