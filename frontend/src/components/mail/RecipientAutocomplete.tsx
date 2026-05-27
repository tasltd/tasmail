// Added: TMAIL-119 — contact autocomplete for Composer recipient fields.
// Suggestions are loaded from /api/contacts?q=<token> for the last comma-delimited token
// once the user has typed 2+ characters. Keyboard nav (ArrowUp/Down/Enter/Escape) is supported.
import { useEffect, useMemo, useRef, useState } from 'react';
import { fetchContacts } from '../../api/contacts';
import type { Contact } from '../../api/contacts';

type Props = {
  value: string;
  onChange: (next: string) => void;
  placeholder?: string;
  label?: string;
  // PURPOSE: stable id used by aria-controls / list role linking
  inputId?: string;
};

// PURPOSE: Split a recipient string into committed tokens + the active (in-progress) token.
// Auto-complete only ever rewrites the active token; previously typed entries are untouched.
export function splitRecipientTokens(value: string): { committed: string; active: string } {
  const idx = value.lastIndexOf(',');
  if (idx < 0) return { committed: '', active: value };
  return { committed: value.slice(0, idx + 1), active: value.slice(idx + 1) };
}

// PURPOSE: Format a contact as a recipient-line entry. Quotes display names that contain a comma.
export function formatContactToken(c: Contact): string {
  if (!c.display_name || c.display_name.trim() === '') return c.email;
  const needsQuoting = /[,<>]/.test(c.display_name);
  const name = needsQuoting ? `"${c.display_name.replace(/"/g, '\\"')}"` : c.display_name;
  return `${name} <${c.email}>`;
}

export function RecipientAutocomplete({ value, onChange, placeholder, label, inputId }: Props) {
  const [suggestions, setSuggestions] = useState<Contact[]>([]);
  const [open, setOpen] = useState(false);
  const [highlight, setHighlight] = useState(0);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);

  const { committed, active } = useMemo(() => splitRecipientTokens(value), [value]);
  const query = active.trim();

  // PURPOSE: Debounced lookup. Skips when query is too short to keep noise down.
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (query.length < 2) {
      setSuggestions([]);
      setOpen(false);
      return;
    }
    debounceRef.current = setTimeout(async () => {
      try {
        const results = await fetchContacts(query);
        setSuggestions(results.slice(0, 8));
        setOpen(results.length > 0);
        setHighlight(0);
      } catch {
        setSuggestions([]);
        setOpen(false);
      }
    }, 200);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [query]);

  // PURPOSE: Click-outside closes the dropdown.
  useEffect(() => {
    const onDocClick = (e: MouseEvent) => {
      if (!containerRef.current) return;
      if (!containerRef.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', onDocClick);
    return () => document.removeEventListener('mousedown', onDocClick);
  }, []);

  const applySuggestion = (c: Contact) => {
    const token = formatContactToken(c);
    const leadingSpace = committed.length > 0 && !committed.endsWith(' ') ? ' ' : '';
    onChange(`${committed}${leadingSpace}${token}, `);
    setOpen(false);
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (!open || suggestions.length === 0) return;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setHighlight((h) => (h + 1) % suggestions.length);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setHighlight((h) => (h - 1 + suggestions.length) % suggestions.length);
    } else if (e.key === 'Enter' || e.key === 'Tab') {
      e.preventDefault();
      applySuggestion(suggestions[highlight]);
    } else if (e.key === 'Escape') {
      setOpen(false);
    }
  };

  const listboxId = inputId ? `${inputId}-listbox` : undefined;

  return (
    <div ref={containerRef} style={{ position: 'relative', flex: 1 }}>
      {label && <label htmlFor={inputId}>{label}</label>}
      <input
        id={inputId}
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={onKeyDown}
        onFocus={() => suggestions.length > 0 && setOpen(true)}
        placeholder={placeholder}
        role="combobox"
        aria-autocomplete="list"
        aria-expanded={open}
        aria-controls={listboxId}
        autoComplete="off"
      />
      {open && suggestions.length > 0 && (
        <ul
          id={listboxId}
          role="listbox"
          style={{
            position: 'absolute',
            top: '100%',
            left: 0,
            right: 0,
            zIndex: 20,
            background: 'var(--color-bg, #fff)',
            border: '1px solid var(--color-border, #ccc)',
            borderRadius: '4px',
            margin: 0,
            padding: '4px 0',
            maxHeight: '240px',
            overflowY: 'auto',
            listStyle: 'none',
            boxShadow: '0 4px 12px rgba(0,0,0,0.08)',
          }}
        >
          {suggestions.map((c, i) => (
            <li
              key={c.id}
              role="option"
              aria-selected={i === highlight}
              onMouseDown={(e) => {
                // PURPOSE: mousedown not click — fires before input blur, keeps focus stable
                e.preventDefault();
                applySuggestion(c);
              }}
              onMouseEnter={() => setHighlight(i)}
              style={{
                padding: '6px 10px',
                cursor: 'pointer',
                background: i === highlight ? 'var(--color-bg-hover, #eef)' : 'transparent',
              }}
            >
              <div style={{ fontWeight: 500 }}>{c.display_name || c.email}</div>
              {c.display_name && (
                <div style={{ fontSize: '0.85em', color: 'var(--color-text-secondary, #666)' }}>{c.email}</div>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
