// commitlint config — enforced by .github/workflows/pr-quality.yml and
// the local lefthook commit-msg hook.
//
// Type set + scope enum match .claude/rules/git-workflow.md exactly.
// Body requirement on feat / fix forces the "why" line that AI-generated
// commit messages reliably omit.

module.exports = {
  extends: ['@commitlint/config-conventional'],
  rules: {
    'type-enum': [
      2,
      'always',
      ['feat', 'fix', 'docs', 'style', 'refactor', 'test', 'chore', 'perf', 'build', 'ci', 'revert'],
    ],
    'scope-enum': [
      2,
      'always',
      [
        'server',
        'frontend',
        'desktop',
        'mobile',
        'models',
        'services',
        'api',
        'core',
        'transport-http',
        'transport-cli',
        'transport-veilid',
        'ui',
        'a11y',
        'build',
        'ci',
        'deps',
        'release',
        'docs',
        'security',
      ],
    ],
    // empty scope only allowed on chore / revert / release-meta commits
    'scope-empty': [2, 'never'],
    'subject-case': [2, 'never', ['upper-case', 'pascal-case', 'start-case']],
    'subject-empty': [2, 'never'],
    'subject-full-stop': [2, 'never', '.'],
    'header-max-length': [2, 'always', 100],
    'body-leading-blank': [2, 'always'],
    'footer-leading-blank': [2, 'always'],
    // Force a body on feat / fix so we capture *why* the change exists.
    // Conventional commits + the body is the contributor's chance to
    // explain motivation; missing it is a frequent AI-slop tell.
    'body-empty': [
      1,
      'never',
    ],
  },
};
