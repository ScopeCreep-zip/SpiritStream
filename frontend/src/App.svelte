<script>
  import "./theme.css";
  import { outputGroups, addOutputGroup, removeOutputGroup, addStreamTarget, removeStreamTarget } from './outputGroupStore';
</script>

<main class="app-container">
  <!-- Sidebar -->
  <aside class="sidebar">
    <!-- Profile Management -->
    <section class="profile-management">
      <h2>Profile Management</h2>
      <button class="profile-btn add">Add Profile</button>
      <button class="profile-btn load">Load Profile</button>
      <button class="profile-btn delete">Delete Profile</button>
      <button class="profile-btn save">Save Profile</button>
    </section>

    <!-- Stream Settings -->
    <section class="stream-settings">
      <h2>Stream Settings</h2>
      <label for="stream-url">Incoming Stream URL</label>
      <input type="text" id="stream-url" placeholder="Enter incoming URL here..." />

      <div class="checkbox-group">
        <input type="checkbox" id="generatePTS" />
        <label for="generatePTS">Generate PTS</label>
      </div>

      <button class="start-stream">Start Stream</button>
      <button class="stop-stream">Stop Stream</button>
    </section>
  </aside>

  <!-- Main Content Area (Output Groups) -->
  <section class="content">
    <h2>Output Groups</h2>
    <button class="add-output-group" on:click={addOutputGroup}>Add Output Group</button>

    <!-- Output Groups Container -->
    {#each $outputGroups as group, index}
  <div class="output-group">
    <div>
      <label for="group-name-{index}">Group Name</label>
      <input id="group-name-{index}" type="text" bind:value={group.name} placeholder="Enter group name"/>
    </div>

    <div>
      <label for="video-encoder-{index}">Video Encoder</label>
      <input id="video-encoder-{index}" type="text" bind:value={group.videoEncoder} placeholder="Enter video encoder"/>
    </div>

    <div>
      <label for="resolution-{index}">Resolution</label>
      <input id="resolution-{index}" type="text" bind:value={group.resolution} placeholder="Enter resolution"/>
    </div>

    <div>
      <label for="bitrate-{index}">Bitrate</label>
      <input id="bitrate-{index}" type="text" bind:value={group.bitrate} placeholder="Enter bitrate"/>
    </div>

    <div>
      <label for="fps-{index}">FPS</label>
      <input id="fps-{index}" type="text" bind:value={group.fps} placeholder="Enter FPS"/>
    </div>

    <div>
      <label for="audio-codec-{index}">Audio Codec</label>
      <input id="audio-codec-{index}" type="text" bind:value={group.audioCodec} placeholder="Enter audio codec"/>
    </div>

    <div>
      <label for="audio-bitrate-{index}">Audio Bitrate</label>
      <input id="audio-bitrate-{index}" type="text" bind:value={group.audioBitrate} placeholder="Enter audio bitrate"/>
    </div>

    <div>
      <label for="generatePTS-{index}">Generate PTS</label>
      <input id="generatePTS-{index}" type="checkbox" bind:checked={group.generatePTS}/>
    </div>

    <!-- Stream Targets -->
    <div>
      <label for="stream-targets-{index}">Stream Targets</label>
      <div id="stream-targets-{index}">
        {#each group.streamTargets as target, targetIndex}
          <div class="stream-target">
            <label for="url-{index}-{targetIndex}">URL</label>
            <input id="url-{index}-{targetIndex}" type="text" bind:value={target.url} placeholder="Enter URL"/>

            <label for="stream-key-{index}-{targetIndex}">Stream Key</label>
            <input id="stream-key-{index}-{targetIndex}" type="text" bind:value={target.streamKey} placeholder="Enter Key"/>

            <label for="rtmp-port-{index}-{targetIndex}">RTMP Port</label>
            <input id="rtmp-port-{index}-{targetIndex}" type="number" bind:value={target.rtmpPort} placeholder="Enter RTMP Port"/>

            <button class="remove-stream-target" on:click={() => removeStreamTarget(index, targetIndex)}>Remove Stream Target</button>
          </div>
        {/each}
        <button class="add-stream-target" on:click={() => addStreamTarget(index)}>Add Stream Target</button>
      </div>
    </div>

    <button class="remove-output-group" on:click={() => removeOutputGroup(index)}>Remove Output Group</button>
  </div>
{/each}

  </section>
</main>

<style>
  /* === Global Layout === */
  .app-container {
    display: grid;
    grid-template-columns: 1fr 3fr; /* Sidebar takes 1/4, content 3/4 */
    height: 100vh;
    background-color: var(--main-bg, #111);
  }

  /* === Sidebar === */
  .sidebar {
    display: flex;
    flex-direction: column;
    justify-content: flex-start; /* Align everything from the top */
    padding: 20px;
    background-color: var(--sidebar-bg, #222);
    color: var(--text-color, white);
    width: 100%;
    box-sizing: border-box; /* Ensures padding doesn't overflow */
  }

  /* === Profile Management & Stream Settings === */
  .profile-management {
    margin-bottom: 20px;
  }

  .stream-settings {
    margin-top: 30px;
    padding-top: 20px;
    display: flex;
    flex-direction: column;
    gap: 2px; 
  }

  /* === Labels & Inputs === */
  label {
    font-weight: bold;
    margin-bottom: 5px;
  }

  input {
    width: calc(100% - 10px); /* Adds a little padding from edges */
    max-width: 100%; /* Prevents overflow */
    padding: 8px;
    margin: 5px 0 10px 0; /* Consistent spacing */
    background-color: var(--input-bg, #333);
    border: 1px solid var(--border-color, #555);
    color: var(--text-color, white);
    border-radius: 4px;
  }

  /* === Checkbox Styling === */
  .checkbox-group {
    display: flex;
    align-items: center; /* Align checkbox and label inline */
    justify-content: center; /* Center checkbox + label as a unit */
    gap: 5px; /* Spacing between checkbox and label */
    width: 100%;
    margin-bottom: 15px; /* Adds spacing below the checkbox */
  }

  .checkbox-group input {
    width: auto; /* Prevents checkbox from stretching */
    margin: 0; /* Resets default margins */
  }

  /* === Buttons === */
  button {
    width: calc(100% - 10px); /* Consistent width for all buttons */
    max-width: 100%;
    padding: 10px;
    border: none;
    cursor: pointer;
    border-radius: 4px;
    font-weight: bold;
    margin: 5px 0; /* Keeps button spacing consistent */
  }

  /* Profile Management Buttons */
  .profile-btn.add,
  .profile-btn.load { background-color: var(--blue, #007bff); }
  .profile-btn.delete { background-color: var(--red, #dc3545); }
  .profile-btn.save { background-color: var(--green, #28a745); }

  /* Stream Control Buttons */
  .start-stream { background-color: var(--green, #28a745); margin-bottom: 2px !important;}
  .stop-stream { background-color: var(--red, #dc3545); }

  /* Hover Effects */
  button:hover {
    filter: brightness(1.2);
  }

  /* === Output Groups Section === */
  .output-groups {
    margin-top: 20px;
    padding: 10px;
    background-color: #333;
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  /* Each Output Group */
  .output-group {
    background-color: #444;
    padding: 15px;
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    gap: 15px;
  }

  /* Remove Output Group Button */
  .remove-output-group {
    background-color: #dc3545;
    color: white;
    padding: 10px;
    border: none;
    cursor: pointer;
    border-radius: 4px;
  }

  /* Input Fields */
  .output-group input {
    padding: 8px;
    margin: 5px 0;
    background-color: #555;
    border: 1px solid #666;
    color: white;
    border-radius: 4px;
  }

  /* Add Stream Target Button */
  .add-output-group {
    margin-top: 15px;
    background-color: #007bff;
    color: white;
    border: none;
    cursor: pointer;
    border-radius: 4px;
    padding: 10px;
  }

  .add-stream-target {
    background-color: #28a745;
    color: white;
    padding: 5px 10px;
    border: none;
    cursor: pointer;
    border-radius: 4px;
  }

  .remove-stream-target {
    background-color: #dc3545;
    color: white;
    padding: 5px 10px;
    border: none;
    cursor: pointer;
    border-radius: 4px;
  }
</style>
