// Store output groups in an array
let outputGroups = [];

// OutputGroup Model (JavaScript adaptation)
class OutputGroup {
    constructor(id) {
        this.id = id;
        this.name = `Output Group ${id + 1}`;
        this.videoEncoder = "H.264";
        this.resolution = "1920x1080";
        this.bitrate = "4000kbps";
        this.fps = "30";
        this.audioCodec = "AAC";
        this.audioBitrate = "128kbps";
        this.generatePTS = false;
        this.streamTargets = [];
    }
}

// StreamTarget Model (JavaScript adaptation)
class StreamTarget {
    constructor(id, url = "", streamKey = "", rtmpPort = 1935) {
        this.id = id;
        this.url = url.trim().replace(/\/+$/, ""); // Strip trailing slashes
        this.streamKey = streamKey.trim();
        this.rtmpPort = rtmpPort;
    }
}

function addOutputGroup() {
    const container = document.querySelector(".output-groups");

    // Remove placeholder on first add
    const placeholder = document.getElementById("output-groups-placeholder");
    if (placeholder) placeholder.remove();

    // Generate unique group index
    const groupId = outputGroups.length;
    const newGroup = new OutputGroup(groupId);

    // Create output group element
    const groupDiv = document.createElement("div");
    groupDiv.className = "output-group";
    groupDiv.dataset.id = groupId;
    groupDiv.innerHTML = `
        <h3>Output Group ${groupId + 1}</h3>
        <label>Video Encoder:</label>
        <select>
            <option>H.264</option>
            <option>H.265</option>
        </select>
        <label>Resolution:</label>
        <select>
            <option>1920x1080</option>
            <option>1280x720</option>
            <option>640x480</option>
        </select>
        <label>Bitrate:</label>
        <input type="text" value="4000kbps">
        <label>FPS:</label>
        <input type="text" value="30">
        <label>Audio Codec:</label>
        <select>
            <option>AAC</option>
            <option>MP3</option>
        </select>
        <label>Audio Bitrate:</label>
        <input type="text" value="128kbps">
        <label><input type="checkbox"> Generate PTS</label>
        <button class="add-stream-target" onclick="addStreamTarget(${groupId})">Add Stream Target</button>
        <button class="remove-output-group" onclick="removeOutputGroup(${groupId})">Remove Output Group</button>
    `;

    // Append new output group directly to the container (horizontal alignment)
    container.appendChild(groupDiv);
    outputGroups.push(newGroup);
}

// Attach event listener for adding output groups
document.getElementById("add-output-group").addEventListener("click", addOutputGroup);

// Handle Start/Stop Stream Buttons
document.querySelector(".start-stream").addEventListener("click", () => {
    alert("Starting stream...");
});

document.querySelector(".stop-stream").addEventListener("click", () => {
    alert("Stopping stream...");
});