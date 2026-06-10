// Restore persisted theme + contrast BEFORE any CSS loads, so users with a
// custom theme or high-contrast preference see zero flash of unstyled content.
// Served as a same-origin file (not inline) so the production CSP can use
// `script-src 'self'` with NO `'unsafe-inline'`. Loaded render-blocking from
// <head> (no async/defer), so it still completes before the first stylesheet
// parses. Theme and contrast are orthogonal axes restored from separate keys.
//
// Cached custom-theme tokens are applied via CSSOM `setProperty` on <html>
// rather than a script-created <style> element. The CSSOM path is exempt from
// CSP style-src (unlike an injected <style>, which would need `'unsafe-inline'`
// or a nonce), which is why the production CSP can drop `'unsafe-inline'`.
(function () {
  var themeId = null;
  var mode = 'dark';
  var tokens = null;
  var highContrast = false;

  try {
    var storedTheme = localStorage.getItem('spiritstream-theme');
    if (storedTheme) {
      var parsed = JSON.parse(storedTheme);
      themeId = parsed.state && parsed.state.currentThemeId;
      tokens = parsed.state && parsed.state.currentTokens;
      if (themeId) {
        // Theme IDs containing '-light' are light mode, all others are dark
        mode = themeId.includes('-light') ? 'light' : 'dark';
      }
    }
    highContrast = localStorage.getItem('ss-high-contrast') === '1';
  } catch (e) {
    // localStorage genuinely unavailable (sandbox, private mode, etc.).
    // Surface to console so bugs aren't silenced; the boot continues
    // with safe defaults (dark + no contrast override).
    console.error('[theme-init] failed to read localStorage', e);
  }

  if (!themeId) {
    themeId = 'spirit-dark';
    mode = 'dark';
  }

  document.documentElement.setAttribute('data-theme', mode);
  document.documentElement.setAttribute('data-theme-id', themeId);
  if (highContrast) {
    document.documentElement.setAttribute('data-contrast', 'high');
  }

  // Apply cached tokens immediately if available (prevents flash for custom
  // themes). Set as inline custom properties on <html> via CSSOM, which is
  // exempt from CSP style-src — no script-created <style>, no `'unsafe-inline'`.
  // themeStore re-applies the same keys on React rehydration and owns clearing
  // them on theme switch.
  if (tokens && typeof tokens === 'object' && Object.keys(tokens).length > 0) {
    for (var key in tokens) {
      if (tokens.hasOwnProperty(key)) {
        document.documentElement.style.setProperty(key, tokens[key]);
      }
    }
  }
})();
