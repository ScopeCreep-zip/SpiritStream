// Store output groups in an array
let outputGroups = [];

// OutputGroup Model (JavaScript adaptation)
// This model stores the settings for each output group including an array for stream targets.
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
        this.streamTargets = []; // Array to hold stream target objects
    }
}

// StreamTarget Model (JavaScript adaptation)
// This model represents each stream target with URL, stream key, and RTMP port.
class StreamTarget {
    constructor(id, url = "", streamKey = "", rtmpPort = 1935) {
        this.id = id;
        this.url = url.trim().replace(/\/+$/, ""); // Strip trailing slashes
        this.streamKey = streamKey.trim();
        this.rtmpPort = rtmpPort;
    }
}

// Adds a new output group to the UI and model
function addOutputGroup() {
    const container = document.querySelector(".output-groups");

    // Remove placeholder on first add
    const placeholder = document.getElementById("output-groups-placeholder");
    if (placeholder) placeholder.remove();

    // Generate unique group index based on current count
    const groupId = outputGroups.length;
    const newGroup = new OutputGroup(groupId);

    // Create output group element with necessary input fields and buttons
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
        <div class="stream-targets-container"></div>
    `;

    // Append the new group to the output groups container
    container.appendChild(groupDiv);
    outputGroups.push(newGroup);
}

// Adds a new stream target to a specific output group
function addStreamTarget(groupId) {
    // Get the corresponding output group model
    let group = outputGroups[groupId];
    if (!group) return;

    // Generate a unique stream target id for the group
    const streamTargetId = group.streamTargets.length;
    let newStreamTarget = new StreamTarget(streamTargetId);
    group.streamTargets.push(newStreamTarget);

    // Find the output group DOM element and its stream targets container
    const groupDiv = document.querySelector(`.output-group[data-id="${groupId}"]`);
    const targetsContainer = groupDiv.querySelector('.stream-targets-container');

    // Create a new element for the stream target inputs
    const targetDiv = document.createElement("div");
    targetDiv.className = "stream-target";
    targetDiv.dataset.id = streamTargetId;
    targetDiv.innerHTML = `
        <label>Stream Target URL:</label>
        <input type="text" placeholder="Enter stream URL" onchange="updateStreamTargetURL(${groupId}, ${streamTargetId}, this.value)">
        <label>Stream Key:</label>
        <input type="text" placeholder="Enter stream key" onchange="updateStreamTargetKey(${groupId}, ${streamTargetId}, this.value)">
        <button class="remove-stream-target" onclick="removeStreamTarget(${groupId}, ${streamTargetId})">Remove Stream Target</button>
    `;
    targetsContainer.appendChild(targetDiv);
}

// Update the URL of a stream target when the input changes
function updateStreamTargetURL(groupId, targetId, value) {
    let group = outputGroups[groupId];
    if (group && group.streamTargets[targetId]) {
         group.streamTargets[targetId].url = value.trim().replace(/\/+$/, "");
    }
}

// Update the stream key of a stream target when the input changes
function updateStreamTargetKey(groupId, targetId, value) {
    let group = outputGroups[groupId];
    if (group && group.streamTargets[targetId]) {
         group.streamTargets[targetId].streamKey = value.trim();
    }
}

// Removes a stream target from the specified output group
function removeStreamTarget(groupId, targetId) {
    let group = outputGroups[groupId];
    if (!group) return;

    // Update the stream targets array by filtering out the target with the matching id
    group.streamTargets = group.streamTargets.filter(st => st.id !== targetId);

    // Remove the corresponding DOM element
    const groupDiv = document.querySelector(`.output-group[data-id="${groupId}"]`);
    const targetDiv = groupDiv.querySelector(`.stream-target[data-id="${targetId}"]`);
    if (targetDiv) {
         targetDiv.remove();
    }
}

// Removes an entire output group
function removeOutputGroup(groupId) {
    // Remove the output group from the model
    outputGroups = outputGroups.filter(og => og.id !== groupId);
    // Remove the output group element from the DOM
    const groupDiv = document.querySelector(`.output-group[data-id="${groupId}"]`);
    if (groupDiv) groupDiv.remove();
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

// ================= Profile Management Modals Handling =================

// Utility functions to show and hide modals
function showModal(modalId) {
    document.getElementById(modalId).style.display = "block";
}

function hideModal(modalId) {
    document.getElementById(modalId).style.display = "none";
}

// Event listeners for Profile Management Buttons
document.getElementById("add-profile-btn").addEventListener("click", function() {
    showModal("add-profile-modal");
});

document.getElementById("load-profile-btn").addEventListener("click", function() {
    // TODO: Populate profile list dynamically before showing modal
    showModal("load-profile-modal");
});

document.getElementById("save-profile-btn").addEventListener("click", function() {
    showModal("save-profile-modal");
});

document.getElementById("delete-profile-btn").addEventListener("click", function() {
    // TODO: Ensure a profile is selected before showing delete modal
    showModal("delete-profile-modal");
});

// Event listeners for closing modals via close buttons
document.getElementById("add-profile-close").addEventListener("click", function() {
    hideModal("add-profile-modal");
});
document.getElementById("load-profile-close").addEventListener("click", function() {
    hideModal("load-profile-modal");
});
document.getElementById("save-profile-close").addEventListener("click", function() {
    hideModal("save-profile-modal");
});
document.getElementById("delete-profile-close").addEventListener("click", function() {
    hideModal("delete-profile-modal");
});

// Event listeners for cancel buttons in modals
document.getElementById("add-profile-cancel").addEventListener("click", function() {
    hideModal("add-profile-modal");
});
document.getElementById("load-profile-cancel").addEventListener("click", function() {
    hideModal("load-profile-modal");
});
document.getElementById("save-profile-cancel").addEventListener("click", function() {
    hideModal("save-profile-modal");
});
document.getElementById("delete-profile-cancel").addEventListener("click", function() {
    hideModal("delete-profile-modal");
});

// Toggle password input visibility for Add Profile Modal based on encryption checkbox
document.getElementById("enable-encryption").addEventListener("change", function() {
    let passwordContainer = document.getElementById("add-profile-password-container");
    if (this.checked) {
        passwordContainer.style.display = "block";
    } else {
        passwordContainer.style.display = "none";
    }
});

// Confirm action for Add Profile Modal
document.getElementById("add-profile-confirm").addEventListener("click", function() {
    const profileName = document.getElementById("profile-name").value;
    const encryptionEnabled = document.getElementById("enable-encryption").checked;
    const profilePassword = document.getElementById("profile-password").value;
    // TODO: Call profileManager.createProfile with profileName, encryptionEnabled, and profilePassword
    console.log("Creating profile:", profileName, encryptionEnabled, profilePassword);
    hideModal("add-profile-modal");
});

// Confirm action for Load Profile Modal
document.getElementById("load-profile-confirm").addEventListener("click", function() {
    // Retrieve selected profile and password if required
    const selectedProfileId = getSelectedProfileId(); // Placeholder function
    const profilePassword = document.getElementById("load-profile-password").value;
    // TODO: Call profileManager.loadProfile with selectedProfileId and profilePassword if necessary
    console.log("Loading profile:", selectedProfileId, profilePassword);
    hideModal("load-profile-modal");
});

// Confirm action for Save Profile Modal
document.getElementById("save-profile-confirm").addEventListener("click", function() {
    // TODO: Call profileManager.saveProfile for current profile
    console.log("Saving current profile...");
    hideModal("save-profile-modal");
});

// Confirm action for Delete Profile Modal
document.getElementById("delete-profile-confirm").addEventListener("click", function() {
    const selectedProfileId = getSelectedProfileId(); // Placeholder function
    // TODO: Call profileManager.deleteProfile with selectedProfileId
    console.log("Deleting profile:", selectedProfileId);
    hideModal("delete-profile-modal");
});

// Placeholder utility function to get selected profile id
function getSelectedProfileId() {
    // This function should return the id of the currently selected profile from the Load Profile modal
    // For now, we return a dummy value
    return 0;
}