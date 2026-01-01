# Frontend Architecture

## Overview

MagillaStream uses a vanilla JavaScript frontend with no framework dependencies. The UI is built with plain HTML, CSS, and JavaScript, communicating with the Electron main process through the preload-exposed API.

## File Structure

```
src/frontend/index/
├── index.html     # Main HTML structure
├── index.js       # Application logic
└── index.css      # Styling
```

## UI Layout

```
┌─────────────────────────────────────────────────────────────────────┐
│                              Header                                  │
│                         MagillaStream                                │
├───────────────────┬─────────────────────────────────────────────────┤
│                   │                                                  │
│     Sidebar       │              Main Content Area                   │
│     (25%)         │                   (75%)                          │
│                   │                                                  │
│ ┌───────────────┐ │  ┌─────────────────────────────────────────────┐│
│ │ Profile       │ │  │ Output Group 1                              ││
│ │ Management    │ │  │ ┌─────────────────────────────────────────┐ ││
│ │               │ │  │ │ Video: libx264 @ 1920x1080 6000kbps    │ ││
│ │ [Add Profile] │ │  │ │ Audio: aac @ 128kbps                    │ ││
│ │ [Load Profile]│ │  │ └─────────────────────────────────────────┘ ││
│ │ [Delete]      │ │  │ Stream Targets:                             ││
│ │ [Save Profile]│ │  │ ┌────────────────────┐ ┌────────────────┐   ││
│ └───────────────┘ │  │ │ YouTube           │ │ Twitch         │   ││
│                   │  │ │ rtmp://...        │ │ rtmp://...     │   ││
│ ┌───────────────┐ │  │ └────────────────────┘ └────────────────┘   ││
│ │ Stream        │ │  └─────────────────────────────────────────────┘│
│ │ Settings      │ │                                                  │
│ │               │ │  ┌─────────────────────────────────────────────┐│
│ │ Incoming URL  │ │  │ Output Group 2                              ││
│ │ [__________]  │ │  │ ...                                         ││
│ │               │ │  └─────────────────────────────────────────────┘│
│ │ ☐ Generate PTS│ │                                                  │
│ └───────────────┘ │  [+ Add Output Group]                            │
│                   │                                                  │
│ ┌───────────────┐ │                                                  │
│ │ Stream Control│ │                                                  │
│ │               │ │                                                  │
│ │ [Start Stream]│ │                                                  │
│ │ [Stop Stream] │ │                                                  │
│ └───────────────┘ │                                                  │
│                   │                                                  │
└───────────────────┴─────────────────────────────────────────────────┘
```

## State Management

### Application State

```javascript
// Global state
let currentProfile = null;      // Currently loaded profile name
let outputGroups = [];          // Array of output group objects
let isStreaming = false;        // Stream active flag
let availableEncoders = {
  video: [],                    // Available video encoders
  audio: []                     // Available audio encoders
};
```

### State Flow

```
User Action → Update State → Sync UI → (Optional) Save to Backend
```

### Example State Update

```javascript
function addOutputGroup() {
  const newGroup = {
    id: generateUUID(),
    videoEncoder: 'libx264',
    resolution: '1920x1080',
    videoBitrate: 6000,
    fps: 30,
    audioCodec: 'aac',
    audioBitrate: 128,
    generatePts: false,
    streamTargets: []
  };

  outputGroups.push(newGroup);
  renderOutputGroups();
}
```

## Component Structure

### Sidebar Components

```javascript
// Profile Management
const profileSection = {
  addButton: '#btn-add-profile',
  loadButton: '#btn-load-profile',
  deleteButton: '#btn-delete-profile',
  saveButton: '#btn-save-profile'
};

// Stream Settings
const settingsSection = {
  incomingUrl: '#incoming-url',
  generatePts: '#generate-pts'
};

// Stream Control
const controlSection = {
  startButton: '#btn-start-stream',
  stopButton: '#btn-stop-stream'
};
```

### Main Content Components

```javascript
// Output Groups Container
const mainContent = {
  container: '#output-groups-container',
  addGroupButton: '#btn-add-group'
};
```

## Modal System

### Modal Types

| Modal | Purpose | Fields |
|-------|---------|--------|
| Add Profile | Create new profile | Name, Password (optional) |
| Load Profile | Select profile to load | Profile list, Password |
| Save Profile | Confirm save | Confirmation |
| Delete Profile | Confirm deletion | Confirmation |

### Modal Implementation

```javascript
function showModal(modalId) {
  const modal = document.getElementById(modalId);
  modal.style.display = 'flex';
  modal.classList.add('active');
}

function hideModal(modalId) {
  const modal = document.getElementById(modalId);
  modal.style.display = 'none';
  modal.classList.remove('active');
}

function showAddProfileModal() {
  document.getElementById('new-profile-name').value = '';
  document.getElementById('new-profile-password').value = '';
  document.getElementById('encrypt-profile').checked = false;
  showModal('add-profile-modal');
}
```

## Event Handling

### Global Event Listeners

```javascript
document.addEventListener('DOMContentLoaded', () => {
  // Initialize
  loadEncoders();
  loadLastUsedProfile();

  // Profile buttons
  document.getElementById('btn-add-profile')
    .addEventListener('click', showAddProfileModal);
  document.getElementById('btn-load-profile')
    .addEventListener('click', showLoadProfileModal);
  document.getElementById('btn-save-profile')
    .addEventListener('click', saveCurrentProfile);
  document.getElementById('btn-delete-profile')
    .addEventListener('click', showDeleteConfirmModal);

  // Stream controls
  document.getElementById('btn-start-stream')
    .addEventListener('click', startStreaming);
  document.getElementById('btn-stop-stream')
    .addEventListener('click', stopStreaming);

  // Add output group
  document.getElementById('btn-add-group')
    .addEventListener('click', addOutputGroup);
});
```

### Dynamic Event Delegation

```javascript
// Handle clicks on dynamically created elements
document.getElementById('output-groups-container')
  .addEventListener('click', (e) => {
    const target = e.target;

    if (target.matches('.btn-remove-group')) {
      const groupId = target.dataset.groupId;
      removeOutputGroup(groupId);
    }

    if (target.matches('.btn-add-target')) {
      const groupId = target.dataset.groupId;
      addStreamTarget(groupId);
    }

    if (target.matches('.btn-remove-target')) {
      const groupId = target.dataset.groupId;
      const targetId = target.dataset.targetId;
      removeStreamTarget(groupId, targetId);
    }
  });
```

## API Integration

### Loading Profiles

```javascript
async function loadProfile(name, password = null) {
  try {
    const profile = await window.electronAPI.profileManager.load(name, password);

    currentProfile = profile.name;
    outputGroups = profile.outputGroups;
    document.getElementById('incoming-url').value = profile.incomingUrl;

    renderOutputGroups();
    updateUIState();

    await window.electronAPI.profileManager.saveLastUsed(name);
  } catch (error) {
    showError(`Failed to load profile: ${error.message}`);
  }
}
```

### Saving Profiles

```javascript
async function saveCurrentProfile() {
  syncUIToState();

  const profile = {
    id: generateUUID(),
    name: currentProfile,
    incomingUrl: document.getElementById('incoming-url').value,
    outputGroups: outputGroups
  };

  const password = document.getElementById('profile-password').value || null;

  try {
    await window.electronAPI.profileManager.save(profile, password);
    showSuccess('Profile saved successfully');
  } catch (error) {
    showError(`Failed to save profile: ${error.message}`);
  }
}
```

### Starting Stream

```javascript
async function startStreaming() {
  const incomingUrl = document.getElementById('incoming-url').value;

  if (!incomingUrl) {
    showError('Please enter an incoming URL');
    return;
  }

  try {
    for (const group of outputGroups) {
      await window.electronAPI.ffmpegHandler.start(group, incomingUrl);
    }
    isStreaming = true;
    updateStreamingUI();
  } catch (error) {
    showError(`Failed to start stream: ${error.message}`);
  }
}
```

## Rendering

### Output Groups Rendering

```javascript
function renderOutputGroups() {
  const container = document.getElementById('output-groups-container');
  container.innerHTML = '';

  outputGroups.forEach((group, index) => {
    const groupElement = createOutputGroupElement(group, index);
    container.appendChild(groupElement);
  });
}

function createOutputGroupElement(group, index) {
  const div = document.createElement('div');
  div.className = 'output-group';
  div.dataset.groupId = group.id;

  div.innerHTML = `
    <div class="output-group-header">
      <h3>Output Group ${index + 1}</h3>
      <button class="btn-remove-group" data-group-id="${group.id}">✕</button>
    </div>
    <div class="output-group-settings">
      <div class="setting-row">
        <label>Video Encoder</label>
        <select class="video-encoder" data-group-id="${group.id}">
          ${renderEncoderOptions(availableEncoders.video, group.videoEncoder)}
        </select>
      </div>
      <!-- More settings... -->
    </div>
    <div class="stream-targets">
      <h4>Stream Targets</h4>
      <div class="targets-container" data-group-id="${group.id}">
        ${renderStreamTargets(group.streamTargets, group.id)}
      </div>
      <button class="btn-add-target" data-group-id="${group.id}">
        + Add Target
      </button>
    </div>
  `;

  return div;
}
```

## UI Sync

### Syncing UI to State

```javascript
function syncUIToState() {
  outputGroups = [];

  document.querySelectorAll('.output-group').forEach((groupEl) => {
    const groupId = groupEl.dataset.groupId;

    const group = {
      id: groupId,
      videoEncoder: groupEl.querySelector('.video-encoder').value,
      resolution: groupEl.querySelector('.resolution').value,
      videoBitrate: parseInt(groupEl.querySelector('.video-bitrate').value),
      fps: parseInt(groupEl.querySelector('.fps').value),
      audioCodec: groupEl.querySelector('.audio-codec').value,
      audioBitrate: parseInt(groupEl.querySelector('.audio-bitrate').value),
      generatePts: groupEl.querySelector('.generate-pts').checked,
      streamTargets: []
    };

    groupEl.querySelectorAll('.stream-target').forEach((targetEl) => {
      group.streamTargets.push({
        id: targetEl.dataset.targetId,
        url: targetEl.querySelector('.target-url').value,
        streamKey: targetEl.querySelector('.stream-key').value,
        port: parseInt(targetEl.querySelector('.port').value) || 1935
      });
    });

    outputGroups.push(group);
  });
}
```

## CSS Architecture

### Layout System

```css
/* Main layout */
.app-container {
  display: flex;
  height: 100vh;
}

.sidebar {
  width: 25%;
  min-width: 250px;
  max-width: 350px;
  background: var(--sidebar-bg);
  padding: 1rem;
}

.main-content {
  flex: 1;
  padding: 1rem;
  overflow-y: auto;
}
```

### Component Styling

```css
/* Output group card */
.output-group {
  background: var(--card-bg);
  border-radius: 8px;
  padding: 1rem;
  margin-bottom: 1rem;
  box-shadow: var(--card-shadow);
}

/* Buttons */
.btn-primary {
  background: var(--primary-color);
  color: white;
  border: none;
  padding: 0.5rem 1rem;
  border-radius: 4px;
  cursor: pointer;
}

.btn-primary:hover {
  background: var(--primary-hover);
}
```

### CSS Variables

```css
:root {
  --primary-color: #007bff;
  --primary-hover: #0056b3;
  --sidebar-bg: #f8f9fa;
  --card-bg: #ffffff;
  --card-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
  --text-color: #212529;
  --border-color: #dee2e6;
}

[data-theme="dark"] {
  --primary-color: #0d6efd;
  --sidebar-bg: #212529;
  --card-bg: #343a40;
  --text-color: #f8f9fa;
  --border-color: #495057;
}
```

## Utilities

### UUID Generation

```javascript
function generateUUID() {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = Math.random() * 16 | 0;
    const v = c === 'x' ? r : (r & 0x3 | 0x8);
    return v.toString(16);
  });
}
```

### Notifications

```javascript
function showSuccess(message) {
  showNotification(message, 'success');
}

function showError(message) {
  showNotification(message, 'error');
}

function showNotification(message, type) {
  const notification = document.createElement('div');
  notification.className = `notification notification-${type}`;
  notification.textContent = message;

  document.body.appendChild(notification);

  setTimeout(() => {
    notification.classList.add('fade-out');
    setTimeout(() => notification.remove(), 300);
  }, 3000);
}
```
