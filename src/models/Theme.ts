// Theme.ts

export class Theme {
    id: string;
    name: string;
    primaryColor: string;
    secondaryColor: string;
    backgroundColor: string;
    textColor: string;
    darkMode: boolean;
  
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
  }
  