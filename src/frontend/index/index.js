window.addEventListener("DOMContentLoaded", () => {

    const profileManager = window?.electronAPI?.profileManager;

    if (!profileManager) {
        console.error("profileManager not available — check preload setup.");
    }

    const lastUsed = localStorage.getItem("lastUsedProfile");
    if (lastUsed && window.electronAPI?.profileManager) {
    window.electronAPI.profileManager
        .loadProfile(lastUsed)
        .then(profileDTO => {
        if (profileDTO) {
            console.log(`Restored profile: ${lastUsed}`);
            applyProfileToUI(profileDTO);
        } else {
            console.warn("No profile loaded from localStorage reference.");
        }
        })
        .catch(err => {
        console.error("Failed to load profile from localStorage:", err);
        });
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

    function syncUIToState() {
        const groupDivs = document.querySelectorAll(".output-group");
    
        groupDivs.forEach(groupDiv => {
            const groupId = parseInt(groupDiv.dataset.id);
            const group = outputGroups[groupId];
            if (!group) return;
    
            const selects = groupDiv.querySelectorAll("select");
            const inputs = groupDiv.querySelectorAll("input[type='text']");
            const checkbox = groupDiv.querySelector("input[type='checkbox']");
    
            group.videoEncoder = selects[0].value;
            group.resolution = selects[1].value;
            group.bitrate = inputs[0].value;
            group.fps = inputs[1].value;
            group.audioCodec = selects[2].value;
            group.audioBitrate = inputs[2].value;
            group.generatePTS = checkbox.checked;
    
            const targetDivs = groupDiv.querySelectorAll(".stream-target");
            targetDivs.forEach((targetDiv, tIndex) => {
                const target = group.streamTargets[tIndex];
                if (!target) return;
    
                const inputs = targetDiv.querySelectorAll("input");
                target.url = inputs[0].value.trim().replace(/\/+$/, "");
                target.streamKey = inputs[1].value.trim();
            });
        });
    }

    // ========== Output Group Handling ==========

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
            <div class="stream-targets-container"></div>
            <button class="add-stream-target">Add Stream Target</button>
            <button class="remove-output-group">Remove Output Group</button>
        `;
    
        // Bind dynamic button handlers
        groupDiv.querySelector(".add-stream-target").addEventListener("click", () => {
            addStreamTarget(groupId);
        });
    
        groupDiv.querySelector(".remove-output-group").addEventListener("click", () => {
            removeOutputGroup(groupId);
        });
    
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
            <input type="text" placeholder="Enter stream URL">
            <label>Stream Key:</label>
            <input type="text" placeholder="Enter stream key">
            <button class="remove-stream-target">Remove Stream Target</button>
        `;
    
        // Bind field handlers
        const inputs = targetDiv.querySelectorAll("input");
        inputs[0].addEventListener("change", e => {
            updateStreamTargetURL(groupId, streamTargetId, e.target.value);
        });
        inputs[1].addEventListener("change", e => {
            updateStreamTargetKey(groupId, streamTargetId, e.target.value);
        });
    
        // Bind remove button
        targetDiv.querySelector(".remove-stream-target").addEventListener("click", () => {
            removeStreamTarget(groupId, streamTargetId);
        });
    
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
        syncUIToState(); // Save any edited inputs to memory before deleting
    
        const group = outputGroups[groupId];
        if (!group) return;
    
        group.streamTargets = group.streamTargets.filter(st => st.id !== targetId);
    
        const groupDiv = document.querySelector(`.output-group[data-id="${groupId}"]`);
        const targetDiv = groupDiv.querySelector(`.stream-target[data-id="${targetId}"]`);
        if (targetDiv) targetDiv.remove();
    }    

    function removeOutputGroup(groupId) {
        syncUIToState(); // Capture unsaved changes before anything is removed 
        
        // Remove from array
        outputGroups = outputGroups.filter(group => group.id !== groupId);
    
        // Remove from DOM
        const container = document.querySelector(".output-groups");
        const groupDiv = container.querySelector(`.output-group[data-id="${groupId}"]`);
        if (groupDiv) groupDiv.remove();
    
        // Clear all existing output group DOM nodes
        container.innerHTML = "";
    
        // Re-render all groups from updated array with fresh IDs
        outputGroups.forEach((group, newIndex) => {
            group.id = newIndex; // Reindex the group
            const groupDiv = document.createElement("div");
            groupDiv.className = "output-group";
            groupDiv.dataset.id = newIndex;
    
            groupDiv.innerHTML = `
                <h3>Output Group ${newIndex + 1}</h3>
                <label>Video Encoder:</label>
                <select><option>H.264</option><option>H.265</option></select>
                <label>Resolution:</label>
                <select><option>1920x1080</option><option>1280x720</option><option>640x480</option></select>
                <label>Bitrate:</label>
                <input type="text" value="${group.bitrate}">
                <label>FPS:</label>
                <input type="text" value="${group.fps}">
                <label>Audio Codec:</label>
                <select><option>AAC</option><option>MP3</option></select>
                <label>Audio Bitrate:</label>
                <input type="text" value="${group.audioBitrate}">
                <label><input type="checkbox" ${group.generatePTS ? "checked" : ""}> Generate PTS</label>
                <div class="stream-targets-container"></div>
                <button class="add-stream-target">Add Stream Target</button>
                <button class="remove-output-group">Remove Output Group</button>
            `;
    
            // Re-bind add/remove buttons
            groupDiv.querySelector(".add-stream-target").addEventListener("click", () => {
                addStreamTarget(newIndex);
            });
    
            groupDiv.querySelector(".remove-output-group").addEventListener("click", () => {
                removeOutputGroup(newIndex);
            });
    
            container.appendChild(groupDiv);
    
            // Re-render this group's stream targets
            const targetsContainer = groupDiv.querySelector(".stream-targets-container");
            group.streamTargets.forEach((target, tIndex) => {
                target.id = tIndex;
    
                const targetDiv = document.createElement("div");
                targetDiv.className = "stream-target";
                targetDiv.dataset.id = tIndex;
    
                targetDiv.innerHTML = `
                    <label>Stream Target URL:</label>
                    <input type="text" value="${target.url}">
                    <label>Stream Key:</label>
                    <input type="text" value="${target.streamKey}">
                    <button class="remove-stream-target">Remove Stream Target</button>
                `;
    
                const inputs = targetDiv.querySelectorAll("input");
                inputs[0].addEventListener("change", e => {
                    updateStreamTargetURL(newIndex, tIndex, e.target.value);
                });
                inputs[1].addEventListener("change", e => {
                    updateStreamTargetKey(newIndex, tIndex, e.target.value);
                });
    
                targetDiv.querySelector(".remove-stream-target").addEventListener("click", () => {
                    removeStreamTarget(newIndex, tIndex);
                });
    
                targetsContainer.appendChild(targetDiv);
            });
        });
    }    

    // ========== Profile Dropdown ==========

    async function populateProfileDropdown() {
        const select = document.getElementById("profile-select");
        select.innerHTML = "";
    
        if (!window.electronAPI?.profileManager) {
            console.error("profileManager not available — check preload setup.");
            return;
        }
    
        try {
            const profiles = await window.electronAPI.profileManager.getAllProfileNames();
    
            if (profiles.length === 0) {
                const emptyOption = document.createElement("option");
                emptyOption.textContent = "No profiles found";
                emptyOption.disabled = true;
                emptyOption.selected = true;
                select.appendChild(emptyOption);
                return;
            }
    
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

    // ========== UI Event Listeners ==========

    document.getElementById("add-output-group").addEventListener("click", addOutputGroup);

    document.querySelector(".start-stream").addEventListener("click", () => {
        alert("Starting stream...");
    });
    document.querySelector(".stop-stream").addEventListener("click", () => {
        alert("Stopping stream...");
    });

    // Modals
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

    // Encryption toggle visibility
    document.getElementById("enable-encryption").addEventListener("change", function () {
        const container = document.getElementById("add-profile-password-container");
        container.style.display = this.checked ? "block" : "none";
    });

    // Modal Confirm Buttons 
    document.getElementById("add-profile-confirm").addEventListener("click", async function () {
        const profileName = document.getElementById("profile-name").value;
        const encryptionEnabled = document.getElementById("enable-encryption").checked;
        const profilePassword = document.getElementById("profile-password").value;
    
        const profileDTO = {
            id: crypto.randomUUID(), // You can use a better UUID generator later
            name: profileName,
            incomingURL: "", // default empty
            outputGroups: [],
            generatePTS: false,
            theme: {} // or your default theme object
        };
    
        try {
            await window.electronAPI.profileManager.saveProfile(profileDTO, encryptionEnabled ? profilePassword : undefined);
            await window.electronAPI.profileManager.saveLastUsedProfile(profileName);
            console.log("Profile created:", profileName);
            hideModal("add-profile-modal");
        } catch (err) {
            console.error("Failed to create profile:", err);
        }
    });    

    document.getElementById("load-profile-confirm").addEventListener("click", async function () {
        const selectedProfileName = getSelectedProfileId();
        const password = document.getElementById("load-profile-password").value;
    
        try {
            const profileDTO = await window.electronAPI.profileManager.loadProfile(selectedProfileName, password);
            if (!profileDTO) {
                alert("Failed to load profile. It may be encrypted or corrupted.");
                return;
            }
    
            // TODO: Apply profileDTO to the GUI here
            console.log("Loaded profile:", profileDTO.name);
            hideModal("load-profile-modal");
        } catch (err) {
            console.error("Error loading profile:", err);
        }
    });    

    document.getElementById("save-profile-confirm").addEventListener("click", async function () {
        try {
            // You'll need to gather current profile state from the UI
            const currentProfileName = getSelectedProfileId(); // or however you're tracking it
            const profilePassword = ""; // or prompt user if encrypted
            const profileDTO = buildProfileFromUI(currentProfileName); // implement this
    
            await window.electronAPI.profileManager.saveProfile(profileDTO, profilePassword);
            await window.electronAPI.profileManager.saveLastUsedProfile(profileDTO.name);
            console.log("Saved profile:", profileDTO.name);
            hideModal("save-profile-modal");
        } catch (err) {
            console.error("Failed to save profile:", err);
        }
    });    

    document.getElementById("delete-profile-confirm").addEventListener("click", async function () {
        const selectedProfile = getSelectedProfileId();
    
        try {
            await window.electronAPI.profileManager.deleteProfile(selectedProfile);
            console.log("Deleted profile:", selectedProfile);
            hideModal("delete-profile-modal");
        } catch (err) {
            console.error("Failed to delete profile:", err);
        }
    });

    // === Apply Loaded Profile to UI ===
    function applyProfileToUI(profileDTO) {
        // Set stream URL
        document.getElementById("stream-url").value = profileDTO.incomingURL || "";

        // Set global Generate PTS checkbox
        document.getElementById("generatePTS").checked = !!profileDTO.generatePTS;

        // Clear existing output groups
        outputGroups = [];
        const container = document.querySelector(".output-groups");
        container.innerHTML = "";

        if (profileDTO.outputGroups?.length > 0) {
            profileDTO.outputGroups.forEach((groupDTO, index) => {
                const newGroup = new OutputGroup(index);

                newGroup.name = groupDTO.name;
                newGroup.videoEncoder = groupDTO.videoEncoder;
                newGroup.resolution = groupDTO.resolution;
                newGroup.bitrate = groupDTO.bitrate;
                newGroup.fps = groupDTO.fps;
                newGroup.audioCodec = groupDTO.audioCodec;
                newGroup.audioBitrate = groupDTO.audioBitrate;
                newGroup.generatePTS = groupDTO.generatePTS;
                newGroup.streamTargets = groupDTO.streamTargets.map((t, tIndex) => {
                    return new StreamTarget(tIndex, t.url, t.streamKey, t.rtmpPort || 1935);
                });

                outputGroups.push(newGroup);
            });

            // Re-render output groups
            outputGroups.forEach((_, idx) => {
                addOutputGroup(); // This renders from `outputGroups` array
            });

            // After rendering, restore all the values
            syncGroupsToUI(); // Set values inside rendered fields from state
        }
    }

    // === Sync OutputGroup objects to UI fields ===
    // This assumes that outputGroups already matches the DOM count
    function syncGroupsToUI() {
        const groupDivs = document.querySelectorAll(".output-group");

        groupDivs.forEach(groupDiv => {
            const groupId = parseInt(groupDiv.dataset.id);
            const group = outputGroups[groupId];
            if (!group) return;

            const selects = groupDiv.querySelectorAll("select");
            const inputs = groupDiv.querySelectorAll("input[type='text']");
            const checkbox = groupDiv.querySelector("input[type='checkbox']");

            selects[0].value = group.videoEncoder;
            selects[1].value = group.resolution;
            inputs[0].value = group.bitrate;
            inputs[1].value = group.fps;
            selects[2].value = group.audioCodec;
            inputs[2].value = group.audioBitrate;
            checkbox.checked = group.generatePTS;

            const targetsContainer = groupDiv.querySelector(".stream-targets-container");
            targetsContainer.innerHTML = ""; // Clear any existing

            group.streamTargets.forEach((target, tIndex) => {
                const targetDiv = document.createElement("div");
                targetDiv.className = "stream-target";
                targetDiv.dataset.id = tIndex;

                targetDiv.innerHTML = `
                    <label>Stream Target URL:</label>
                    <input type="text" value="${target.url}">
                    <label>Stream Key:</label>
                    <input type="text" value="${target.streamKey}">
                    <button class="remove-stream-target">Remove Stream Target</button>
                `;

                const inputs = targetDiv.querySelectorAll("input");
                inputs[0].addEventListener("change", e => {
                    updateStreamTargetURL(groupId, tIndex, e.target.value);
                });
                inputs[1].addEventListener("change", e => {
                    updateStreamTargetKey(groupId, tIndex, e.target.value);
                });

                targetDiv.querySelector(".remove-stream-target").addEventListener("click", () => {
                    removeStreamTarget(groupId, tIndex);
                });

                targetsContainer.appendChild(targetDiv);
            });
        });
    }

    // === Build Profile from UI ===
    function buildProfileFromUI(profileName) {
        syncUIToState(); // Make sure latest inputs are synced into outputGroups

        const profile = {
            id: crypto.randomUUID(),
            name: profileName,
            incomingURL: document.getElementById("stream-url").value.trim(),
            generatePTS: document.getElementById("generatePTS").checked,
            outputGroups: outputGroups.map(group => ({
                name: group.name,
                videoEncoder: group.videoEncoder,
                resolution: group.resolution,
                bitrate: group.bitrate,
                fps: group.fps,
                audioCodec: group.audioCodec,
                audioBitrate: group.audioBitrate,
                generatePTS: group.generatePTS,
                streamTargets: group.streamTargets.map(target => ({
                    id: target.id,
                    url: target.url,
                    streamKey: target.streamKey,
                    rtmpPort: target.rtmpPort || 1935,
                }))
            })),
            theme: { mode: "dark" } // Default for now
        };

        return profile;
    }
});