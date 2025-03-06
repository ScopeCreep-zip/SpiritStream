const fs = require("fs");
const path = require("path");

const srcDir = path.join(__dirname, "../resources");
const destDir = path.join(__dirname, "../dist/resources");

function copyRecursiveSync(src, dest) {
  if (!fs.existsSync(dest)) {
    fs.mkdirSync(dest, { recursive: true });
  }
  const entries = fs.readdirSync(src, { withFileTypes: true });

  for (const entry of entries) {
    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);

    if (entry.isDirectory()) {
      copyRecursiveSync(srcPath, destPath);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

// Ensure resources directory is copied to dist
console.log(`Copying resources from ${srcDir} to ${destDir}...`);
copyRecursiveSync(srcDir, destDir);
console.log("Resources copied successfully.");
