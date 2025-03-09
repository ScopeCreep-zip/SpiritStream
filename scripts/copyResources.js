const fs = require("fs");
const path = require("path");

const sourceDirs = [
  "resources",
  "config",
];

const distRoot = path.join(__dirname, "../dist");
const frontendSrc = path.join(__dirname, "..", "src/frontend");
const frontendDest = path.join(distRoot, "frontend");

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

// Loop through each source directory
sourceDirs.forEach((dir) => {
  const srcDir = path.join(__dirname, "..", dir);
  const destDir = path.join(distRoot, dir);

  if (fs.existsSync(srcDir)) {
    console.log(`Copying ${dir} from ${srcDir} to ${destDir}...`);
    copyRecursiveSync(srcDir, destDir);
    console.log(`${dir} copied successfully.`);
  } else {
    console.warn(`Warning: ${dir} directory does not exist, skipping.`);
  }
});

// Copy frontend separately to dist/frontend (not dist/src/frontend)
if (fs.existsSync(frontendSrc)) {
  console.log(`Copying frontend from ${frontendSrc} to ${frontendDest}...`);
  copyRecursiveSync(frontendSrc, frontendDest);
  console.log(`frontend copied successfully.`);
} else {
  console.warn(`Warning: src/frontend directory does not exist, skipping.`);
}
