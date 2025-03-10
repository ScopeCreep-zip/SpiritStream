const fs = require('fs');
const path = require('path');

function copyFolderSync(src, dest) {
    if (!fs.existsSync(src)) {
        console.warn(`Warning: Source folder "${src}" does not exist. Skipping.`);
        return;
    }
    if (!fs.existsSync(dest)) {
        fs.mkdirSync(dest, { recursive: true });
    }
    fs.readdirSync(src).forEach(file => {
        const srcFile = path.join(src, file);
        const destFile = path.join(dest, file);
        if (fs.lstatSync(srcFile).isDirectory()) {
            copyFolderSync(srcFile, destFile);
        } else {
            fs.copyFileSync(srcFile, destFile);
        }
    });
}

console.log("Copying resources to dist/resources...");
copyFolderSync("resources", "dist/resources");
console.log("Resources copied successfully.");

console.log("Copying config to dist/config...");
copyFolderSync("config", "dist/config");
console.log("Config copied successfully.");

console.log("Copying frontend to dist/frontend...");
copyFolderSync("src/frontend", "dist/frontend");
console.log("Frontend copied successfully.");

console.log("Copying resources to release/win-unpacked/resources...");
copyFolderSync("resources", "release/win-unpacked/resources");
console.log("Resources copied successfully to release.");

console.log("Copying config to release/win-unpacked/config...");
copyFolderSync("config", "release/win-unpacked/config");
console.log("Config copied successfully to release.");

console.log("Copying frontend to release/win-unpacked/frontend...");
copyFolderSync("src/frontend", "release/win-unpacked/frontend");
console.log("Frontend copied successfully to release.");
