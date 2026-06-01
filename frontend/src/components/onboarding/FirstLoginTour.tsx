// Added (TMAIL-401): first-login product tour. Three sequential popovers
// anchored to elements marked with `data-tour="compose|inbox|settings"`.
//
// Decision: skip react-joyride / intro.js. Both add ≥30 KB gzipped for a
// fixed 3-step tour we render once per mailbox lifetime. A custom popover
// keeps the bundle lean, matches existing TASMail styling tokens, and
// has no React 19 compat risk. The package.json keeps no new
// dependency — see the TMAIL-401 note in CLAUDE.md.

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  fetchFirstLoginTourSeen,
  markFirstLoginTourSeen,
} from '../../api/preferences';
import './FirstLoginTour.css';

interface TourStep {
  // data-tour value the step is anchored to. Falls back to centered overlay
  // if the element is missing from the DOM (defensive — sidebar may be
  // collapsed on mobile).
  anchor: 'compose' | 'inbox' | 'settings';
  title: string;
  body: string;
  // Where the popover sits relative to the anchor. Sidebar entries are on
  // the left so we point the popover to the right of them.
  placement: 'right' | 'bottom';
}

const STEPS: TourStep[] = [
  {
    anchor: 'compose',
    title: 'Compose mail',
    body: 'Click here to send mail. The button stays in this corner — keep it within reach.',
    placement: 'right',
  },
  {
    anchor: 'inbox',
    title: 'Your inbox',
    body: 'Your inbox lives here. Sent, Drafts, Spam, Trash appear once you have messages.',
    placement: 'right',
  },
  {
    anchor: 'settings',
    title: 'Everything else',
    body: 'Signature, vacation responder, filters, 2FA, integrations — they all live under Settings.',
    placement: 'right',
  },
];

interface AnchorRect {
  top: number;
  left: number;
  width: number;
  height: number;
}

function readAnchorRect(anchor: TourStep['anchor']): AnchorRect | null {
  if (typeof document === 'undefined') return null;
  const el = document.querySelector<HTMLElement>(`[data-tour="${anchor}"]`);
  if (!el) return null;
  const rect = el.getBoundingClientRect();
  return {
    top: rect.top,
    left: rect.left,
    width: rect.width,
    height: rect.height,
  };
}

interface PopoverStyle {
  top: number;
  left: number;
}

// PURPOSE: position the popover next to the anchor element. When the
// anchor is missing (sidebar collapsed, element not yet mounted) we
// return null so the caller renders a centered fallback.
function computePopoverStyle(
  rect: AnchorRect | null,
  placement: TourStep['placement'],
  popoverWidth = 320,
  gap = 12,
): PopoverStyle | null {
  if (!rect) return null;
  if (placement === 'right') {
    return {
      top: Math.max(16, rect.top),
      left: rect.left + rect.width + gap,
    };
  }
  // 'bottom'
  return {
    top: rect.top + rect.height + gap,
    left: Math.max(16, rect.left + rect.width / 2 - popoverWidth / 2),
  };
}

export interface FirstLoginTourProps {
  // Lets tests force the tour open without depending on the network query.
  forceOpen?: boolean;
}

export function FirstLoginTour({ forceOpen = false }: FirstLoginTourProps) {
  const queryClient = useQueryClient();
  const { data, isLoading } = useQuery({
    queryKey: ['preferences', 'first-login-tour-seen'],
    queryFn: fetchFirstLoginTourSeen,
    // The flag never changes mid-session once dismissed.
    staleTime: Infinity,
    enabled: !forceOpen,
  });

  const dismissMutation = useMutation({
    mutationFn: markFirstLoginTourSeen,
    onSuccess: (result) => {
      queryClient.setQueryData(['preferences', 'first-login-tour-seen'], result);
    },
  });

  const [stepIndex, setStepIndex] = useState(0);
  const [, forceTick] = useState(0);

  // Reflow popover position on window resize so it tracks the anchor.
  useEffect(() => {
    const onResize = () => forceTick((n) => n + 1);
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);

  const shouldRender = forceOpen || (!isLoading && data?.seen === false);
  const step = STEPS[stepIndex];

  const handleDismiss = useCallback(() => {
    dismissMutation.mutate();
  }, [dismissMutation]);

  const handleNext = useCallback(() => {
    if (stepIndex < STEPS.length - 1) {
      setStepIndex((i) => i + 1);
    } else {
      handleDismiss();
    }
  }, [stepIndex, handleDismiss]);

  const rect = useMemo(() => (step ? readAnchorRect(step.anchor) : null), [step]);
  const popoverStyle = useMemo(
    () => computePopoverStyle(rect, step?.placement ?? 'right'),
    [rect, step?.placement],
  );

  if (!shouldRender || !step) return null;

  const isLast = stepIndex === STEPS.length - 1;

  return (
    <div
      className="first-login-tour"
      data-testid="first-login-tour"
      role="dialog"
      aria-modal="true"
      aria-labelledby="first-login-tour-title"
    >
      {/* TMAIL-405: backdrop is purely visual dimming. pointer-events:none
          in the stylesheet means it never receives clicks; users dismiss via
          the explicit Skip / Got it buttons in the popover. */}
      <div
        className="first-login-tour__backdrop"
        data-testid="first-login-tour-backdrop"
        aria-hidden="true"
      />
      <div
        className={`first-login-tour__popover ${
          popoverStyle ? '' : 'first-login-tour__popover--centered'
        }`}
        style={popoverStyle ?? undefined}
        data-testid="first-login-tour-popover"
        data-tour-step={step.anchor}
      >
        <div className="first-login-tour__step-count">
          Step {stepIndex + 1} of {STEPS.length}
        </div>
        <h3 id="first-login-tour-title" className="first-login-tour__title">
          {step.title}
        </h3>
        <p className="first-login-tour__body">{step.body}</p>
        <div className="first-login-tour__actions">
          <button
            type="button"
            className="btn btn--ghost"
            onClick={handleDismiss}
            data-testid="first-login-tour-skip"
          >
            Skip
          </button>
          <button
            type="button"
            className="btn btn--primary"
            onClick={handleNext}
            disabled={dismissMutation.isPending}
            data-testid="first-login-tour-next"
          >
            {isLast ? 'Got it' : 'Next'}
          </button>
        </div>
      </div>
    </div>
  );
}
