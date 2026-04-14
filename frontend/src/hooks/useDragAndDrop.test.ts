// Added: Unit tests for drag-and-drop hooks (TMAIL-122)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import {
  serializeDragData,
  parseDragData,
  useMessageDrag,
  useFolderDrop,
  type DragData,
} from './useDragAndDrop';

// Added: Helper to create a mock DragEvent with dataTransfer
function createMockDragEvent(data?: Record<string, string>): React.DragEvent<HTMLElement> {
  const store: Record<string, string> = { ...data };
  return {
    preventDefault: vi.fn(),
    dataTransfer: {
      setData: vi.fn((key: string, value: string) => { store[key] = value; }),
      getData: vi.fn((key: string) => store[key] || ''),
      effectAllowed: '' as string,
      dropEffect: '' as string,
    },
  } as unknown as React.DragEvent<HTMLElement>;
}

describe('serializeDragData', () => {
  it('serializes DragData to JSON string', () => {
    const dragData: DragData = { type: 'message', uid: 42, folder: 'INBOX' };
    const serialized = serializeDragData(dragData);
    expect(JSON.parse(serialized)).toEqual(dragData);
  });
});

describe('parseDragData', () => {
  it('parses valid DragData JSON', () => {
    const raw = JSON.stringify({ type: 'message', uid: 7, folder: 'Sent' });
    const result = parseDragData(raw);
    expect(result).toEqual({ type: 'message', uid: 7, folder: 'Sent' });
  });

  it('returns null for null input', () => {
    expect(parseDragData(null)).toBeNull();
  });

  it('returns null for undefined input', () => {
    expect(parseDragData(undefined)).toBeNull();
  });

  it('returns null for empty string', () => {
    expect(parseDragData('')).toBeNull();
  });

  it('returns null for invalid JSON', () => {
    expect(parseDragData('not-json')).toBeNull();
  });

  it('returns null for wrong type field', () => {
    const raw = JSON.stringify({ type: 'folder', uid: 1, folder: 'INBOX' });
    expect(parseDragData(raw)).toBeNull();
  });

  it('returns null for missing uid', () => {
    const raw = JSON.stringify({ type: 'message', folder: 'INBOX' });
    expect(parseDragData(raw)).toBeNull();
  });

  it('returns null for non-number uid', () => {
    const raw = JSON.stringify({ type: 'message', uid: 'abc', folder: 'INBOX' });
    expect(parseDragData(raw)).toBeNull();
  });
});

describe('useMessageDrag', () => {
  it('returns draggable as true', () => {
    const { result } = renderHook(() => useMessageDrag(1, 'INBOX'));
    expect(result.current.draggable).toBe(true);
  });

  it('sets isDragging to true on dragStart and false on dragEnd', () => {
    const { result } = renderHook(() => useMessageDrag(1, 'INBOX'));
    expect(result.current.isDragging).toBe(false);

    const startEvent = createMockDragEvent();
    act(() => {
      result.current.onDragStart(startEvent);
    });
    expect(result.current.isDragging).toBe(true);

    const endEvent = createMockDragEvent();
    act(() => {
      result.current.onDragEnd(endEvent);
    });
    expect(result.current.isDragging).toBe(false);
  });

  it('serializes DragData into dataTransfer on dragStart', () => {
    const { result } = renderHook(() => useMessageDrag(99, 'Drafts'));
    const mockEvent = createMockDragEvent();

    act(() => {
      result.current.onDragStart(mockEvent);
    });

    expect(mockEvent.dataTransfer.setData).toHaveBeenCalledWith(
      'application/x-tasmail-drag',
      expect.any(String),
    );

    // NOTE: Verify the serialized data is correct
    const serializedArg = (mockEvent.dataTransfer.setData as ReturnType<typeof vi.fn>).mock.calls[0][1];
    expect(JSON.parse(serializedArg)).toEqual({ type: 'message', uid: 99, folder: 'Drafts' });
  });
});

describe('useFolderDrop', () => {
  let dropHandler: (dragData: DragData) => void;

  beforeEach(() => {
    dropHandler = vi.fn() as unknown as (dragData: DragData) => void;
  });

  it('sets isOver to true on dragEnter and false on dragLeave', () => {
    const { result } = renderHook(() => useFolderDrop('Trash', dropHandler));
    expect(result.current.isOver).toBe(false);

    act(() => {
      result.current.onDragEnter(createMockDragEvent());
    });
    expect(result.current.isOver).toBe(true);

    act(() => {
      result.current.onDragLeave();
    });
    expect(result.current.isOver).toBe(false);
  });

  it('calls onDrop handler with parsed DragData on valid drop', () => {
    const { result } = renderHook(() => useFolderDrop('Trash', dropHandler));
    const dragData: DragData = { type: 'message', uid: 5, folder: 'INBOX' };

    const mockEvent = createMockDragEvent({
      'application/x-tasmail-drag': JSON.stringify(dragData),
    });

    act(() => {
      result.current.onDrop(mockEvent);
    });

    expect(dropHandler).toHaveBeenCalledWith(dragData);
    expect(mockEvent.preventDefault).toHaveBeenCalled();
  });

  it('prevents drop on the source folder (same folder)', () => {
    const { result } = renderHook(() => useFolderDrop('INBOX', dropHandler));
    const dragData: DragData = { type: 'message', uid: 5, folder: 'INBOX' };

    const mockEvent = createMockDragEvent({
      'application/x-tasmail-drag': JSON.stringify(dragData),
    });

    act(() => {
      result.current.onDrop(mockEvent);
    });

    // NOTE: Handler should NOT be called when source equals target
    expect(dropHandler).not.toHaveBeenCalled();
  });

  it('does not call handler when dataTransfer has no valid data', () => {
    const { result } = renderHook(() => useFolderDrop('Trash', dropHandler));

    const mockEvent = createMockDragEvent({});

    act(() => {
      result.current.onDrop(mockEvent);
    });

    expect(dropHandler).not.toHaveBeenCalled();
  });

  it('resets isOver to false on drop', () => {
    const { result } = renderHook(() => useFolderDrop('Sent', dropHandler));

    // NOTE: Simulate entering then dropping
    act(() => {
      result.current.onDragEnter(createMockDragEvent());
    });
    expect(result.current.isOver).toBe(true);

    const dragData: DragData = { type: 'message', uid: 3, folder: 'INBOX' };
    const dropEvent = createMockDragEvent({
      'application/x-tasmail-drag': JSON.stringify(dragData),
    });

    act(() => {
      result.current.onDrop(dropEvent);
    });
    expect(result.current.isOver).toBe(false);
  });

  it('prevents default on dragOver for valid drop target', () => {
    const { result } = renderHook(() => useFolderDrop('Trash', dropHandler));
    const mockEvent = createMockDragEvent();

    act(() => {
      result.current.onDragOver(mockEvent);
    });

    expect(mockEvent.preventDefault).toHaveBeenCalled();
  });
});
