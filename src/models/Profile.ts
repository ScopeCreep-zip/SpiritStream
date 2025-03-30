import { OutputGroup } from "./OutputGroup";
import { Theme } from "./Theme";
import { ProfileDTO } from "../shared/interfaces";

export class Profile {
  private id: string;
  private name: string;
  private incomingURL: string;
  private outputGroups: OutputGroup[];
  private theme?: Theme;

  constructor(id: string, name: string, incomingURL: string, generatePTS: boolean, theme?: Theme) {
    this.id = id;
    this.name = name;
    this.incomingURL = incomingURL;
    this.outputGroups = [];
    this.theme = theme;
  }

  // Getters
  public getId(): string {
    return this.id;
  }

  public getName(): string {
    return this.name;
  }

  public getIncomingURL(): string {
    return this.incomingURL;
  }

  public getOutputGroups(): OutputGroup[] {
    return this.outputGroups;
  }

  public getTheme(): Theme | undefined {
    return this.theme;
  }

  // Setters
  public setName(newName: string): void {
    this.name = newName;
  }

  public setIncomingURL(url: string): void {
    this.incomingURL = url;
  }

  public setTheme(theme: Theme): void {
    this.theme = theme;
  }

  // Output Group Methods
  public addOutputGroup(group: OutputGroup): void {
    this.outputGroups.push(group);
  }

  public removeOutputGroup(groupId: string): void {
    this.outputGroups = this.outputGroups.filter(g => g.getId() !== groupId);
  }

  public getOutputGroupById(groupId: string): OutputGroup | undefined {
    return this.outputGroups.find(g => g.getId() === groupId);
  }

  public toDTO(): ProfileDTO {
    return {
      id: this.id,
      name: this.name,
      incomingURL: this.incomingURL,
      outputGroups: this.outputGroups.map(g => g.toDTO()),
      theme: this.theme?.toDTO(),
    };
  }

  // Export profile as JSON
  public export(): string {
    return JSON.stringify({
      id: this.id,
      name: this.name,
      incomingURL: this.incomingURL,
      outputGroups: this.outputGroups.map(group => group.export()),
      theme: this.theme,
    }, null, 2);
  }
}
