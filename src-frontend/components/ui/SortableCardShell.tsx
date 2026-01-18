import type React from "react";
import {useSortable} from "@dnd-kit/sortable";
import {CSS} from "@dnd-kit/utilities";


export interface SortableCardShellProps {
  id: string;
  children: React.ReactNode;
}

export default function SortableCardShell(props: SortableCardShellProps) {
  const {id, children} = props;
  const { setNodeRef, transform, transition, attributes, listeners, isDragging } = useSortable({ id });

  const style: React.CSSProperties = {
    transform: transform
      ? CSS.Transform.toString({ ...transform, scaleX: 1, scaleY: 1 })
      : undefined,
    transition,
    opacity: isDragging ? 0.85 : 1,
  };

  return(
    <div ref={setNodeRef} className={isDragging ? "cursor-grabbing" : "cursor-pointer"} style={style} {...attributes} {...listeners}>
    {children}
    </div>
  );
}
