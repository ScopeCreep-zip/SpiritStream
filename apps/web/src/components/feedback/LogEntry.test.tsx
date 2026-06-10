import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import '@/lib/i18n';
import { LogEntry } from './LogEntry';

// N3: sample component test. Proves the Vitest + jsdom + React-Testing-
// Library harness renders a real component with i18n wired — importing
// `@/lib/i18n` initialises the same default instance `useTranslation`
// reads, so labels resolve to real English strings (no mock). The
// level→label map is the behaviour worth pinning: a typo in a level key
// would silently mislabel error rows in the log viewer.

describe('LogEntry', () => {
  it('renders the timestamp and message verbatim', () => {
    render(<LogEntry time="12:00:00" level="info" message="stream started" />);
    expect(screen.getByText('12:00:00')).toBeInTheDocument();
    expect(screen.getByText('stream started')).toBeInTheDocument();
  });

  it('maps each level to its English label', () => {
    const { rerender } = render(<LogEntry time="t" level="info" message="m" />);
    expect(screen.getByText('INFO')).toBeInTheDocument();
    rerender(<LogEntry time="t" level="warn" message="m" />);
    expect(screen.getByText('WARN')).toBeInTheDocument();
    rerender(<LogEntry time="t" level="error" message="m" />);
    expect(screen.getByText('ERR')).toBeInTheDocument();
    rerender(<LogEntry time="t" level="debug" message="m" />);
    expect(screen.getByText('DBG')).toBeInTheDocument();
  });

  it('applies the level-specific text style class', () => {
    render(<LogEntry time="t" level="error" message="boom" />);
    expect(screen.getByText('ERR')).toHaveClass('text-error-text');
  });
});
