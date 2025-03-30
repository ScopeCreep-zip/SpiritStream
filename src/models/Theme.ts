import { ThemeDTO } from "../shared/interfaces";

export class Theme {
  private id: string;
  private name: string;
  private primaryColor: string;
  private secondaryColor: string;
  private backgroundColor: string;
  private textColor: string;
  private darkMode: boolean;

  constructor(
    id: string,
    name: string,
    primaryColor: string,
    secondaryColor: string,
    backgroundColor: string,
    textColor: string,
    darkMode: boolean
  ) {
    this.id = id;
    this.name = name;
    this.primaryColor = primaryColor;
    this.secondaryColor = secondaryColor;
    this.backgroundColor = backgroundColor;
    this.textColor = textColor;
    this.darkMode = darkMode;
  }

  // Getters
  public getId(): string {
    return this.id;
  }

  public getName(): string {
    return this.name;
  }

  public getPrimaryColor(): string {
    return this.primaryColor;
  }

  public getSecondaryColor(): string {
    return this.secondaryColor;
  }

  public getBackgroundColor(): string {
    return this.backgroundColor;
  }

  public getTextColor(): string {
    return this.textColor;
  }

  public isDarkMode(): boolean {
    return this.darkMode;
  }

  // Setters
  public setName(newName: string): void {
    this.name = newName;
  }

  public setPrimaryColor(color: string): void {
    this.primaryColor = color;
  }

  public setSecondaryColor(color: string): void {
    this.secondaryColor = color;
  }

  public setBackgroundColor(color: string): void {
    this.backgroundColor = color;
  }

  public setTextColor(color: string): void {
    this.textColor = color;
  }

  public setDarkMode(enabled: boolean): void {
    this.darkMode = enabled;
  }

  public toDTO(): ThemeDTO {
    return {
      id: this.id,
      name: this.name,
      primaryColor: this.primaryColor,
      secondaryColor: this.secondaryColor,
      backgroundColor: this.backgroundColor,
      textColor: this.textColor,
      darkMode: this.darkMode,
    };
  }

  // Export Theme as JSON
  public export(): string {
    return JSON.stringify({
      id: this.id,
      name: this.name,
      primaryColor: this.primaryColor,
      secondaryColor: this.secondaryColor,
      backgroundColor: this.backgroundColor,
      textColor: this.textColor,
      darkMode: this.darkMode,
    }, null, 2);
  }
}