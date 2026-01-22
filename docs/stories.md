# User Stories for Profile Management

## 1. Profile Creation (with or without password)
**As a** streamer,  
**I want to** create a new profile,  
**So that** I can save my output settings for future use.  

✅ User can create a profile with **optional encryption**.  
✅ If encryption is **enabled**, they must provide a **password**.  
✅ If encryption is **disabled**, the profile is **base64-encoded** instead of plain text.  
✅ The profile is stored as a **separate encrypted file**.  

---

## 2. List Available Profiles
**As a** streamer,  
**I want to** see all my saved profiles,  
**So that** I can choose which one to load.  

✅ The app **lists profile names** without requiring passwords.  
✅ If a profile is **encrypted**, indicate that it requires a password.  
✅ Profiles appear in a dropdown or list in the UI.  

---

## 3. Load a Profile (Require Password If Encrypted)
**As a** streamer,  
**I want to** load a saved profile,  
**So that** I can continue using my preferred settings.  

✅ Selecting a profile attempts to load it.  
✅ If the profile is **unencrypted**, it loads immediately.  
✅ If the profile is **encrypted**, the user **must enter the correct password**.  
✅ If the **wrong password** is entered, show an error and **do not load the profile**.  

---

## 4. Save Profile (With Encryption Option)
**As a** streamer,  
**I want to** save my profile manually,  
**So that** my changes are retained for future use.  

✅ Clicking **"Save Profile"** updates the stored file.  
✅ If encryption is **enabled**, the profile is saved securely.  
✅ If encryption is **disabled**, the profile is **base64-encoded**.  
✅ **Track unsaved changes** and prompt the user to save when closing the app.  

---

## 5. Track Unsaved Changes & Prompt on Close
**As a** streamer,  
**I want to** be warned if I try to close the app with unsaved changes,  
**So that** I don’t lose important modifications.  

✅ Track when the user makes **unsaved changes**.  
✅ When closing the app, show a **confirmation dialog**:  
   - **Save & Exit**  
   - **Discard Changes & Exit**  
   - **Cancel (Keep App Open)**  
✅ If the user **chooses "Cancel"**, do not close the app.  

---

## 6. Remember Last Used Profile
**As a** streamer,  
**I want the app to remember my last used profile**,  
**So that I don't have to reselect it every time I open the app.**  

✅ The **last used profile ID** is stored in `profileState.json`.  
✅ When the app starts, it **automatically selects the last profile**.  
✅ If the profile is **encrypted**, prompt for a password before loading.  

---

## 7. Delete a Profile (With Confirmation)
**As a** streamer,  
**I want to** delete a profile,  
**So that** I can remove old or unnecessary configurations.  

✅ **User must confirm before deleting a profile.**  
✅ The confirmation warns the user:  
   - **"Are you sure you want to delete this profile? This action cannot be undone."**  
✅ If the user confirms, the **profile file is permanently deleted**.  
✅ If the **deleted profile was the last used one**, reset the app state.  

---

## 8. Remove Encryption From a Profile
**As a** streamer,  
**I want to** remove encryption from my profile,  
**So that** I don’t have to enter a password every time.  

✅ **User must enter the current password** before removing encryption.  
✅ The profile is **converted to base64-encoded JSON**.  
✅ **Warning prompt:**  
   - **"Removing encryption makes your profile less secure. Do you still want to continue?"**  
   - **Option to not show this warning again for this profile.**  

---

## 9. Enable Encryption on an Existing Profile
**As a** streamer,  
**I want to** add encryption to an existing profile,  
**So that** I can protect my settings with a password.  

✅ The user is **prompted to enter a new password**.  
✅ The profile is **encrypted and saved**.  

---

## 10. Change Profile Password
**As a** streamer,  
**I want to** change the password on my encrypted profile,  
**So that** I can update my security settings.  

✅ **User must enter the old password** before changing it.  
✅ A new password is provided, and the profile is **re-encrypted** with it.  

---

## 11. Recover a Lost Password (If Encryption is Optional)
**As a** streamer,  
**I want to** recover my profile if I forget my password,  
**So that** I don’t permanently lose my settings.  

✅ If a profile is **unencrypted**, it can always be accessed.  
✅ If a profile is **encrypted and the user forgets the password**, they have two options:  
   - Restore from **a previously exported unencrypted backup**.  
   - **Reset the profile (deleting encrypted data and starting over).**  

---

## 12. Duplicate an Existing Profile
**As a** streamer,  
**I want to** make a copy of a profile,  
**So that** I can create a new version without overwriting my original.  

✅ The duplicate **retains all settings** but has a new ID.  
✅ User can **choose a new name** for the duplicate.  
✅ If the profile is **encrypted**, the user must enter the password to duplicate it.  
✅ The **duplicate can have its own encryption settings** (optional encryption).  

---

## 13. Set Profile Theme
**As a** streamer,  
**I want to** set a theme for my profile,  
**So that** my UI matches my preferred style.  

✅ Users can choose between **"light" and "dark"** modes for now.  
✅ The selected theme is **saved with the profile**.  
✅ When a profile is **loaded, the theme is applied automatically**.  
✅ Future updates will allow for **custom themes with granular styling**.  
