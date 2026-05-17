import js from '@eslint/js';
import tsParser from '@typescript-eslint/parser';
import tsPlugin from '@typescript-eslint/eslint-plugin';
import reactPlugin from 'eslint-plugin-react';
import reactHooksPlugin from 'eslint-plugin-react-hooks';
import securityPlugin from 'eslint-plugin-security';
import sonarjsPlugin from 'eslint-plugin-sonarjs';
import globals from 'globals';

// SpiritStream frontend ESLint configuration.
//
// Rule families included:
//   - js / typescript-eslint / react / react-hooks (functional baseline)
//   - eslint-plugin-security: OWASP-anchored static checks
//       (eval, child_process, regex-DOS, non-literal-fs, etc).
//       Reference: https://github.com/eslint-community/eslint-plugin-security
//   - eslint-plugin-sonarjs: cognitive-complexity + AI-slop tells
//       (excessive nesting, duplicate string/branch, redundant conditional).
//       Reference: https://github.com/SonarSource/eslint-plugin-sonarjs
//
// `no-explicit-any` promoted from warn -> error per CLAUDE.md strict-mode
// rule: "TypeScript: strict mode, explicit return types". Any new `any`
// in the frontend is treated as a missing type, not a stylistic note.

export default [
  {
    ignores: ['dist/', 'node_modules/'],
  },
  {
    files: ['src/**/*.{ts,tsx}'],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: 'latest',
        sourceType: 'module',
        ecmaFeatures: { jsx: true },
      },
      globals: {
        ...globals.browser,
        ...globals.es2021,
        React: 'readonly',
      },
    },
    plugins: {
      '@typescript-eslint': tsPlugin,
      'react': reactPlugin,
      'react-hooks': reactHooksPlugin,
      'security': securityPlugin,
      'sonarjs': sonarjsPlugin,
    },
    settings: {
      react: { version: 'detect' },
    },
    rules: {
      ...js.configs.recommended.rules,
      ...tsPlugin.configs.recommended.rules,
      ...reactPlugin.configs.recommended.rules,
      ...reactHooksPlugin.configs.recommended.rules,
      ...securityPlugin.configs.recommended.rules,
      ...sonarjsPlugin.configs.recommended.rules,

      // React 17+ automatic runtime — no in-scope React needed.
      'react/react-in-jsx-scope': 'off',
      'react/prop-types': 'off',

      // Strictness boosts on top of the plugin defaults.
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
      '@typescript-eslint/explicit-function-return-type': 'off',
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/no-empty-object-type': 'off',
      'no-console': 'warn',

      // sonarjs cognitive-complexity tuned to match clippy.toml — keeps
      // the JS/TS and Rust review experiences in sync.
      'sonarjs/cognitive-complexity': ['error', 30],
      'sonarjs/no-duplicate-string': ['error', { threshold: 5 }],

      // security plugin: surface dangerous patterns even when ESLint
      // recommends warn. We can't ship XSS / RCE / regex-DOS to
      // vulnerable users; treat them as errors and require an inline
      // disable + comment from the reviewer if a true positive must
      // ship.
      'security/detect-eval-with-expression': 'error',
      'security/detect-non-literal-fs-filename': 'error',
      'security/detect-child-process': 'error',
      'security/detect-unsafe-regex': 'error',
      // Object-injection is noisy on TS code with strict indexing; leave
      // at warn so it surfaces in review without blocking iteration.
      'security/detect-object-injection': 'warn',
    },
  },
];
