import { useEffect, useRef } from "react";
import {useSortable} from "@dnd-kit/sortable";
import {CSS} from "@dnd-kit/utilities";


export interface SortableCardShellProps {
  id: string;
  children: React.ReactNode;
}

export default function SortableCardShell(props: SortableCardShellProps) {
  const {id, children} = props;
  const { setNodeRef, transform, transition, attributes, listeners, isDragging } = useSortable({ id });
  const localRef = useRef<HTMLDivElement>(null);

  // Merge dnd-kit ref with local ref
  const mergedRef = (el: HTMLDivElement | null) => {
    setNodeRef(el);
    (localRef as React.MutableRefObject<HTMLDivElement | null>).current = el;
  };

  useEffect(() => {
    const el = localRef.current;
    if (!el) return;
    el.style.transform = (transform
      ? CSS.Transform.toString({ ...transform, scaleX: 1, scaleY: 1 })
      : '') ?? '';
    el.style.transition = transition || '';
    el.style.opacity = isDragging ? '0.85' : '1';
  }, [transform, transition, isDragging]);

  return(
    <div ref={mergedRef} className={`h-full ${isDragging ? "cursor-grabbing" : "cursor-pointer"}`} {...attributes} {...listeners}>
    {children}
    </div>
  );
}
