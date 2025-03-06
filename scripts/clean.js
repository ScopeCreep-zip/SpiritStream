const fs = require("fs");
const path = require("path");

const distDir = path.join(__dirname, "..", "dist");

function deleteFolderRecursive(folderPath) {
  if (fs.existsSync(folderPath)) {
    fs.rmSync(folderPath, { recursive: true, force: true });
    console.log(`Deleted: ${folderPath}`);
  } else {
    console.log(`Nothing to delete: ${folderPath}`);
  }
}

// Delete `dist/`
deleteFolderRecursive(distDir);
