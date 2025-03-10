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

// Function to create a new output group
function addOutputGroup() {
    const container = document.getElementById("output-groups-container");

    // Generate unique group index
    const groupId = outputGroups.length;
    const newGroup = new OutputGroup(groupId);

    // Create output group element
    const groupDiv = document.createElement("div");
    groupDiv.className = "output-group";
    groupDiv.dataset.id = groupId;
    groupDiv.innerHTML = `
        <h3>${newGroup.name}</h3>

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
        <input type="text" value="${newGroup.bitrate}">

        <label>FPS:</label>
        <input type="text" value="${newGroup.fps}">

        <label>Audio Codec:</label>
        <select>
            <option>AAC</option>
            <option>MP3</option>
        </select>

        <label>Audio Bitrate:</label>
        <input type="text" value="${newGroup.audioBitrate}">

        <label><input type="checkbox"> Generate PTS</label>

        <div id="stream-targets-${groupId}" class="stream-targets">
            <!-- Stream Targets will be added here -->
        </div>

        <button class="add-stream-target" onclick="addStreamTarget(${groupId})">Add Stream Target</button>
        <button class="remove-output-group" onclick="removeOutputGroup(${groupId})">Remove Output Group</button>
    `;

    // Add to DOM & Store in array
    container.appendChild(groupDiv);
    outputGroups.push(newGroup);
}

// Function to add a new Stream Target to an Output Group
function addStreamTarget(groupId) {
    const targetContainer = document.getElementById(`stream-targets-${groupId}`);

    // Generate unique stream target index
    const targetIndex = outputGroups[groupId].streamTargets.length;
    const newTarget = new StreamTarget(targetIndex);

    // Create Stream Target element
    const targetDiv = document.createElement("div");
    targetDiv.className = "stream-target";
    targetDiv.dataset.index = targetIndex;
    targetDiv.innerHTML = `
        <input type="text" placeholder="Stream URL" value="${newTarget.url}">
        <input type="text" placeholder="Stream Key" value="${newTarget.streamKey}">
        <input type="number" placeholder="RTMP Port" value="${newTarget.rtmpPort}">

        <button class="remove-stream-target" onclick="removeStreamTarget(${groupId}, ${targetIndex})">Remove Stream Target</button>
    `;

    // Append to Output Group
    targetContainer.appendChild(targetDiv);
    outputGroups[groupId].streamTargets.push(newTarget);
}

// Function to remove an Output Group
function removeOutputGroup(groupId) {
    outputGroups = outputGroups.filter(group => group.id !== groupId);
    document.querySelector(`.output-group[data-id="${groupId}"]`).remove();
}

// Function to remove a specific Stream Target
function removeStreamTarget(groupId, targetIndex) {
    // Find the correct Output Group
    const group = outputGroups.find(g => g.id === groupId);
    if (group) {
        group.streamTargets = group.streamTargets.filter(target => target.id !== targetIndex);
    }

    // Remove from DOM
    document.querySelector(`#stream-targets-${groupId} .stream-target[data-index="${targetIndex}"]`).remove();
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
