window.addEventListener("DOMContentLoaded", () => {

    const profileManager = window?.electronAPI?.profileManager;

    if (!profileManager) {
        console.error("profileManager not available — check preload setup.");
    }

    // Store output groups in an array
    let outputGroups = [];

    // OutputGroup model
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

    // StreamTarget model
    class StreamTarget {
        constructor(id, url = "", streamKey = "", rtmpPort = 1935) {
            this.id = id;
            this.url = url.trim().replace(/\/+$/, "");
            this.streamKey = streamKey.trim();
            this.rtmpPort = rtmpPort;
        }
    }

    // === Output Group Handling ===

    function addOutputGroup() {
        const container = document.querySelector(".output-groups");
        const placeholder = document.getElementById("output-groups-placeholder");
        if (placeholder) placeholder.remove();

        const groupId = outputGroups.length;
        const newGroup = new OutputGroup(groupId);

        const groupDiv = document.createElement("div");
        groupDiv.className = "output-group";
        groupDiv.dataset.id = groupId;
        groupDiv.innerHTML = `
            <h3>Output Group ${groupId + 1}</h3>
            <label>Video Encoder:</label>
            <select><option>H.264</option><option>H.265</option></select>
            <label>Resolution:</label>
            <select><option>1920x1080</option><option>1280x720</option><option>640x480</option></select>
            <label>Bitrate:</label>
            <input type="text" value="4000kbps">
            <label>FPS:</label>
            <input type="text" value="30">
            <label>Audio Codec:</label>
            <select><option>AAC</option><option>MP3</option></select>
            <label>Audio Bitrate:</label>
            <input type="text" value="128kbps">
            <label><input type="checkbox"> Generate PTS</label>
            <button class="add-stream-target" onclick="addStreamTarget(${groupId})">Add Stream Target</button>
            <button class="remove-output-group" onclick="removeOutputGroup(${groupId})">Remove Output Group</button>
            <div class="stream-targets-container"></div>
        `;

        container.appendChild(groupDiv);
        outputGroups.push(newGroup);
    }

    function addStreamTarget(groupId) {
        let group = outputGroups[groupId];
        if (!group) return;

        const streamTargetId = group.streamTargets.length;
        let newStreamTarget = new StreamTarget(streamTargetId);
        group.streamTargets.push(newStreamTarget);

        const groupDiv = document.querySelector(`.output-group[data-id="${groupId}"]`);
        const targetsContainer = groupDiv.querySelector(".stream-targets-container");

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

    function updateStreamTargetURL(groupId, targetId, value) {
        let group = outputGroups[groupId];
        if (group && group.streamTargets[targetId]) {
            group.streamTargets[targetId].url = value.trim().replace(/\/+$/, "");
        }
    }

    function updateStreamTargetKey(groupId, targetId, value) {
        let group = outputGroups[groupId];
        if (group && group.streamTargets[targetId]) {
            group.streamTargets[targetId].streamKey = value.trim();
        }
    }

    function removeStreamTarget(groupId, targetId) {
        let group = outputGroups[groupId];
        if (!group) return;

        group.streamTargets = group.streamTargets.filter(st => st.id !== targetId);

        const groupDiv = document.querySelector(`.output-group[data-id="${groupId}"]`);
        const targetDiv = groupDiv.querySelector(`.stream-target[data-id="${targetId}"]`);
        if (targetDiv) targetDiv.remove();
    }

    function removeOutputGroup(groupId) {
        outputGroups = outputGroups.filter(og => og.id !== groupId);
        const groupDiv = document.querySelector(`.output-group[data-id="${groupId}"]`);
        if (groupDiv) groupDiv.remove();
    }

    // === Profile Dropdown ===

    async function populateProfileDropdown() {
        const select = document.getElementById("profile-select");
        select.innerHTML = "";

        if (!window.electronAPI?.profileManager) {
            console.error("profileManager not available — check preload setup.");
            return;
        }

        try {
            const profiles = await window.electronAPI.profileManager.getAllProfileNames();

            profiles.forEach(profile => {
                const option = document.createElement("option");
                option.value = profile.name;
                option.textContent = profile.encrypted
                    ? `${profile.name} (encrypted)`
                    : profile.name;
                select.appendChild(option);
            });
        } catch (err) {
            console.error("Failed to load profiles:", err);
        }
    }

    function getSelectedProfileId() {
        return document.getElementById("profile-select").value;
    }

    // === UI Event Listeners ===

    document.getElementById("add-output-group").addEventListener("click", addOutputGroup);

    document.querySelector(".start-stream").addEventListener("click", () => {
        alert("Starting stream...");
    });
    document.querySelector(".stop-stream").addEventListener("click", () => {
        alert("Stopping stream...");
    });

    function showModal(id) {
        document.getElementById(id).style.display = "block";
    }
    function hideModal(id) {
        document.getElementById(id).style.display = "none";
    }

    document.getElementById("add-profile-btn").addEventListener("click", () => showModal("add-profile-modal"));
    document.getElementById("load-profile-btn").addEventListener("click", async () => {
        await populateProfileDropdown();
        showModal("load-profile-modal");
    });
    document.getElementById("save-profile-btn").addEventListener("click", () => showModal("save-profile-modal"));
    document.getElementById("delete-profile-btn").addEventListener("click", () => showModal("delete-profile-modal"));

    document.getElementById("add-profile-close").addEventListener("click", () => hideModal("add-profile-modal"));
    document.getElementById("load-profile-close").addEventListener("click", () => hideModal("load-profile-modal"));
    document.getElementById("save-profile-close").addEventListener("click", () => hideModal("save-profile-modal"));
    document.getElementById("delete-profile-close").addEventListener("click", () => hideModal("delete-profile-modal"));

    document.getElementById("add-profile-cancel").addEventListener("click", () => hideModal("add-profile-modal"));
    document.getElementById("load-profile-cancel").addEventListener("click", () => hideModal("load-profile-modal"));
    document.getElementById("save-profile-cancel").addEventListener("click", () => hideModal("save-profile-modal"));
    document.getElementById("delete-profile-cancel").addEventListener("click", () => hideModal("delete-profile-modal"));

    document.getElementById("enable-encryption").addEventListener("change", function () {
        const container = document.getElementById("add-profile-password-container");
        container.style.display = this.checked ? "block" : "none";
    });

    // Modal Confirm Buttons (currently placeholder logic)
    document.getElementById("add-profile-confirm").addEventListener("click", function () {
        const name = document.getElementById("profile-name").value;
        const encrypted = document.getElementById("enable-encryption").checked;
        const password = document.getElementById("profile-password").value;
        console.log("Creating profile:", name, encrypted, password);
        hideModal("add-profile-modal");
    });

    document.getElementById("load-profile-confirm").addEventListener("click", function () {
        const profile = getSelectedProfileId();
        const password = document.getElementById("load-profile-password").value;
        console.log("Loading profile:", profile, password);
        hideModal("load-profile-modal");
    });

    document.getElementById("save-profile-confirm").addEventListener("click", function () {
        console.log("Saving profile...");
        hideModal("save-profile-modal");
    });

    document.getElementById("delete-profile-confirm").addEventListener("click", function () {
        const profile = getSelectedProfileId();
        console.log("Deleting profile:", profile);
        hideModal("delete-profile-modal");
    });
});
