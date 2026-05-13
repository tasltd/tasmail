// TMAIL-205: snooze dropdown surfaced from the MessageView toolbar.
//
// Consumes api/snooze.ts (the previously-orphan client) and the three preset
// snooze times it ships. Closes the orphan because the trace-check now sees a
// static import path.
import { useEffect, useRef, useState } from 'react';
import { Clock } from 'lucide-react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { getSnoozePresets, snoozeMessage } from '../../api/snooze';

interface SnoozeMenuProps {
  folder: string;
  uid: number;
  onSnoozed: () => void;
}

export function SnoozeMenu({ folder, uid, onSnoozed }: SnoozeMenuProps) {
  const [open, setOpen] = useState(false);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const queryClient = useQueryClient();

  // NOTE: Close on outside click. Cheap enough that we don't reach for a
  // dependency like react-aria or downshift just for one dropdown.
  useEffect(() => {
    if (!open) return;
    function handleClick(e: MouseEvent) {
      if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [open]);

  const snoozeMut = useMutation({
    mutationFn: (snoozeUntil: Date) =>
      snoozeMessage({
        folder,
        message_uid: uid,
        snooze_until: snoozeUntil.toISOString(),
      }),
    onSuccess: () => {
      // Refresh the message list — the snoozed message disappears from the
      // current folder until snooze_until.
      queryClient.invalidateQueries({ queryKey: ['messages', folder] });
      queryClient.invalidateQueries({ queryKey: ['folders'] });
      setOpen(false);
      onSnoozed();
    },
  });

  return (
    <div className="snooze-menu" ref={wrapperRef} style={{ position: 'relative', display: 'inline-block' }}>
      <button
        className="btn btn--icon"
        onClick={() => setOpen((v) => !v)}
        disabled={snoozeMut.isPending}
        title="Snooze"
        aria-haspopup="menu"
        aria-expanded={open}
      >
        <Clock size={20} />
      </button>
      {open && (
        <div
          className="snooze-menu__dropdown"
          role="menu"
          style={{
            position: 'absolute',
            top: 'calc(100% + 4px)',
            right: 0,
            minWidth: 200,
            background: 'var(--bg-elevated, white)',
            border: '1px solid var(--border, #e5e7eb)',
            borderRadius: 8,
            boxShadow: '0 8px 24px rgba(15, 23, 42, 0.12)',
            padding: 4,
            zIndex: 100,
          }}
        >
          {getSnoozePresets().map((preset) => (
            <button
              key={preset.label}
              className="snooze-menu__item"
              role="menuitem"
              onClick={() => snoozeMut.mutate(preset.getTime())}
              disabled={snoozeMut.isPending}
              style={{
                display: 'block',
                width: '100%',
                textAlign: 'left',
                padding: '8px 12px',
                background: 'transparent',
                border: 0,
                cursor: 'pointer',
                fontSize: 14,
                borderRadius: 6,
              }}
            >
              {preset.label}
              <span style={{ display: 'block', fontSize: 12, color: 'var(--text-muted, #64748b)' }}>
                {preset.getTime().toLocaleString(undefined, {
                  weekday: 'short',
                  hour: 'numeric',
                  minute: '2-digit',
                })}
              </span>
            </button>
          ))}
          {snoozeMut.isError && (
            <div role="alert" style={{ padding: '8px 12px', color: 'var(--danger, #dc2626)', fontSize: 12 }}>
              Couldn't snooze — try again.
            </div>
          )}
        </div>
      )}
    </div>
  );
}
