import { app } from "electron";
import path from "path";

// Ensure the correct app name
app.setName("MagillaStream");

// Set userData path to ensure logs and profiles are stored in the correct location
app.setPath("userData", path.join(app.getPath("appData"), "MagillaStream"));
