import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SettingsRail, type RailItem } from './SettingsRail';

const items: ReadonlyArray<RailItem> = [
  { id: 'obs', label: 'OBS' },
  { id: 'chat', label: 'Chat' },
  { id: 'logs', label: 'Logs' },
];

describe('SettingsRail', () => {
  it('renders a vertical tablist with one selected tab', () => {
    render(<SettingsRail items={items} active="chat" onSelect={() => {}} ariaLabel="Settings" />);
    const list = screen.getByRole('tablist');
    expect(list).toHaveAttribute('aria-orientation', 'vertical');
    const tabs = screen.getAllByRole('tab');
    expect(tabs.map((t) => t.textContent)).toEqual(['OBS', 'Chat', 'Logs']);
    // Roving tabindex: only the active tab is in the Tab order.
    expect(tabs[1]).toHaveAttribute('aria-selected', 'true');
    expect(tabs[1]).toHaveAttribute('tabindex', '0');
    expect(tabs[0]).toHaveAttribute('tabindex', '-1');
  });

  it('ArrowDown selects the next section, ArrowUp the previous (wrapping)', () => {
    const onSelect = vi.fn();
    render(<SettingsRail items={items} active="chat" onSelect={onSelect} ariaLabel="Settings" />);
    const list = screen.getByRole('tablist');
    fireEvent.keyDown(list, { key: 'ArrowDown' });
    expect(onSelect).toHaveBeenLastCalledWith('logs');
    fireEvent.keyDown(list, { key: 'ArrowUp' });
    expect(onSelect).toHaveBeenLastCalledWith('obs');
  });

  it('Home/End jump to first/last; click selects directly', () => {
    const onSelect = vi.fn();
    render(<SettingsRail items={items} active="chat" onSelect={onSelect} ariaLabel="Settings" />);
    const list = screen.getByRole('tablist');
    fireEvent.keyDown(list, { key: 'End' });
    expect(onSelect).toHaveBeenLastCalledWith('logs');
    fireEvent.keyDown(list, { key: 'Home' });
    expect(onSelect).toHaveBeenLastCalledWith('obs');
    fireEvent.click(screen.getByRole('tab', { name: 'Logs' }));
    expect(onSelect).toHaveBeenLastCalledWith('logs');
  });
});
