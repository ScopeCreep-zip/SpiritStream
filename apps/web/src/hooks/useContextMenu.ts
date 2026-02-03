/**
 * Context Menu Hook
 *
 * Manages context menu state including position, visibility, and click-outside handling.
 * Extracted from SourcesPanel.tsx to reduce code duplication.
 */
import { useState, useRef, useEffect, useCallback } from 'react';

export interface UseContextMenuReturn {
  /** Whether the context menu is currently open */
  isOpen: boolean;
  /** The x/y position where the menu should appear */
  position: { x: number; y: number };
  /** Ref to attach to the menu container for click-outside detection */
  menuRef: React.RefObject<HTMLDivElement | null>;
  /** Handler to open the menu at the mouse position (use as onContextMenu) */
  openMenu: (e: React.MouseEvent) => void;
  /** Handler to close the menu */
  closeMenu: () => void;
}

/**
 * Hook to manage context menu state with click-outside handling
 *
 * @example
 * ```tsx
 * const { isOpen, position, menuRef, openMenu, closeMenu } = useContextMenu();
 *
 * return (
 *   <div onContextMenu={openMenu}>
 *     Right-click me
 *     {isOpen && (
 *       <div ref={menuRef} style={{ left: position.x, top: position.y }}>
 *         <button onClick={() => { doSomething(); closeMenu(); }}>Action</button>
 *       </div>
 *     )}
 *   </div>
 * );
 * ```
 */
export function useContextMenu(): UseContextMenuReturn {
  const [isOpen, setIsOpen] = useState(false);
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const menuRef = useRef<HTMLDivElement>(null);

  const openMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setPosition({ x: e.clientX, y: e.clientY });
    setIsOpen(true);
  }, []);

  const closeMenu = useCallback(() => {
    setIsOpen(false);
  }, []);

  // Close context menu when clicking outside
  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (e: globalThis.MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [isOpen]);

  return {
    isOpen,
    position,
    menuRef,
    openMenu,
    closeMenu,
  };
}
