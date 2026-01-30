/**
 * Text Source Renderer
 * Renders text sources with CSS-based styling
 */
import type { CSSProperties } from 'react';
import type { TextSource } from '@/types/source';

interface TextSourceRendererProps {
  source: TextSource;
  width?: number;
  height?: number;
}

/**
 * Convert hex color to rgba with opacity
 */
function hexToRgba(hex: string, opacity: number): string {
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
  if (!result) return hex;
  const r = parseInt(result[1], 16);
  const g = parseInt(result[2], 16);
  const b = parseInt(result[3], 16);
  return `rgba(${r}, ${g}, ${b}, ${opacity})`;
}

export function TextSourceRenderer({ source, width, height }: TextSourceRendererProps) {
  const style: CSSProperties = {
    fontFamily: source.fontFamily,
    fontSize: `${source.fontSize}px`,
    fontWeight: source.fontWeight,
    fontStyle: source.fontStyle,
    color: source.textColor,
    backgroundColor: source.backgroundColor
      ? hexToRgba(source.backgroundColor, source.backgroundOpacity)
      : 'transparent',
    textAlign: source.textAlign,
    lineHeight: source.lineHeight,
    padding: source.padding,
    whiteSpace: 'pre-wrap',
    wordBreak: 'break-word',
    width: width ? `${width}px` : '100%',
    height: height ? `${height}px` : '100%',
    display: 'flex',
    alignItems: 'center',
    justifyContent:
      source.textAlign === 'center'
        ? 'center'
        : source.textAlign === 'right'
          ? 'flex-end'
          : 'flex-start',
    overflow: 'hidden',
  };

  // Add text outline via text-shadow if enabled
  if (source.outline?.enabled && source.outline.width > 0) {
    const w = source.outline.width;
    const c = source.outline.color;
    style.textShadow = `
      -${w}px -${w}px 0 ${c},
       ${w}px -${w}px 0 ${c},
      -${w}px  ${w}px 0 ${c},
       ${w}px  ${w}px 0 ${c}
    `;
  }

  return (
    <div style={style}>
      <span>{source.content || 'Text'}</span>
    </div>
  );
}
