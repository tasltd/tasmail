// TMAIL-193: shared TASMail mark for in-app surfaces (header, login, signup, …).
// The geometry mirrors branding/build/svg/logo-primary.svg exactly so the
// favicon, the social card, and every in-product render are byte-identical.
//
// Default colours follow the brand palette (charcoal envelope, teal @). Pass
// `variant="dark"` for white-on-dark surfaces, `"mono"` for single-colour use.

import type { CSSProperties } from 'react';

type Variant = 'primary' | 'dark' | 'mono-black' | 'mono-white';

const COLORS = {
  primary:    { stroke: '#0f172a', accent: '#2dd4bf' },
  dark:       { stroke: '#ffffff', accent: '#2dd4bf' },
  'mono-black': { stroke: '#0f172a', accent: '#0f172a' },
  'mono-white': { stroke: '#ffffff', accent: '#ffffff' },
} as const;

interface Props {
  /** Pixel size (square). Defaults to 40. */
  size?: number;
  variant?: Variant;
  className?: string;
  style?: CSSProperties;
  /** Aria label for screen readers. Defaults to "TASMail". */
  label?: string;
}

export function TasmailLogo({
  size = 40,
  variant = 'primary',
  className,
  style,
  label = 'TASMail',
}: Props) {
  const { stroke, accent } = COLORS[variant];
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      width={size}
      height={size}
      role="img"
      aria-label={label}
      className={className}
      style={style}
    >
      <title>{label}</title>
      {/* envelope body */}
      <rect
        x="3.5" y="6" width="17" height="12"
        rx="1.4" ry="1.4"
        fill="none" stroke={stroke}
        strokeWidth="1.6" strokeLinejoin="round"
      />
      {/* flap (inverted V) */}
      <path
        d="M 3.5 6 L 12 13.2 L 20.5 6"
        fill="none" stroke={stroke}
        strokeWidth="1.6" strokeLinejoin="round" strokeLinecap="round"
      />
      {/* inner t@s wordmark */}
      <g
        fontFamily="'Inter','SF Pro Display','Segoe UI',system-ui,sans-serif"
        fontWeight={700}
        textAnchor="middle"
      >
        <text x="8.6"  y="16.7" fontSize="4.6" letterSpacing="-0.12" fill={stroke}>t</text>
        <text x="12"   y="16.95" fontSize="5.2" letterSpacing="-0.12" fill={accent}>@</text>
        <text x="15.4" y="16.7" fontSize="4.6" letterSpacing="-0.12" fill={stroke}>s</text>
      </g>
    </svg>
  );
}
