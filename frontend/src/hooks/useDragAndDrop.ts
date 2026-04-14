// Added: Custom hooks for HTML5 drag-and-drop message-to-folder operations (TMAIL-122)
import { useState, useCallback, type DragEvent } from 'react';

/**
 * PURPOSE: Data structure serialized into dataTransfer during message drag
 * CONSTRAINTS: Only 'message' type is supported currently
 */
export interface DragData {
  type: 'message';
  uid: number;
  folder: string;
}

// Added: MIME type constant for drag data identification
const DRAG_MIME_TYPE = 'application/x-tasmail-drag';

/**
 * PURPOSE: Serialize DragData to a JSON string for dataTransfer
 * CONSTRAINTS: Must produce valid JSON parseable by parseDragData
 */
export function serializeDragData(dragData: DragData): string {
  return JSON.stringify(dragData);
}

/**
 * PURPOSE: Parse DragData from a dataTransfer JSON string
 * CONSTRAINTS: Returns null if data is missing, malformed, or not a message type
 */
export function parseDragData(raw: string | null | undefined): DragData | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw);
    if (
      parsed &&
      parsed.type === 'message' &&
      typeof parsed.uid === 'number' &&
      typeof parsed.folder === 'string'
    ) {
      return parsed as DragData;
    }
    return null;
  } catch {
    return null;
  }
}

/**
 * PURPOSE: Hook that makes a message row draggable with proper dataTransfer setup
 * CONSTRAINTS: uid must be a valid message UID, folder must be the current folder name
 * EXTERNAL: Uses native HTML5 Drag and Drop API
 */
export function useMessageDrag(uid: number, folder: string) {
  const [isDragging, setIsDragging] = useState(false);

  // Added: Set drag data and visual feedback on drag start
  const onDragStart = useCallback(
    (event: DragEvent<HTMLElement>) => {
      const dragData: DragData = { type: 'message', uid, folder };
      event.dataTransfer.setData(DRAG_MIME_TYPE, serializeDragData(dragData));
      event.dataTransfer.effectAllowed = 'move';
      setIsDragging(true);
    },
    [uid, folder],
  );

  // Added: Clear dragging state when drag ends
  const onDragEnd = useCallback((_event?: DragEvent<HTMLElement>) => {
    setIsDragging(false);
  }, []);

  return {
    draggable: true as const,
    onDragStart,
    onDragEnd,
    isDragging,
  };
}

/**
 * PURPOSE: Hook that makes a folder item a valid drop target for message drags
 * CONSTRAINTS: Prevents dropping a message onto its source folder
 * EXTERNAL: Uses native HTML5 Drag and Drop API
 */
export function useFolderDrop(
  folderName: string,
  onDropHandler: (dragData: DragData) => void,
) {
  const [isOver, setIsOver] = useState(false);

  // Added: Allow drop by preventing default (required for HTML5 DnD)
  const onDragOver = useCallback((event: DragEvent<HTMLElement>) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
  }, []);

  // Added: Track when a dragged item enters the folder drop zone
  const onDragEnter = useCallback((event: DragEvent<HTMLElement>) => {
    event.preventDefault();
    setIsOver(true);
  }, []);

  // Added: Track when a dragged item leaves the folder drop zone
  const onDragLeave = useCallback(() => {
    setIsOver(false);
  }, []);

  // Added: Handle the drop — parse data, validate, and invoke handler
  const onDrop = useCallback(
    (event: DragEvent<HTMLElement>) => {
      event.preventDefault();
      setIsOver(false);

      const raw = event.dataTransfer.getData(DRAG_MIME_TYPE);
      const dragData = parseDragData(raw);

      if (!dragData) return;

      // NOTE: Prevent no-op moves to the same folder
      if (dragData.folder === folderName) return;

      onDropHandler(dragData);
    },
    [folderName, onDropHandler],
  );

  return {
    onDragOver,
    onDragEnter,
    onDragLeave,
    onDrop,
    isOver,
  };
}
