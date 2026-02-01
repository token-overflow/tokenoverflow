// The dist/ + exports layout sidesteps a Bun isolated-linker issue that
// mishandles TypeScript workspace packages publishing raw .ts sources.
// Tracked upstream: https://github.com/alchemy-run/alchemy/issues/994

export interface FontStack {
  readonly sans: string;
  readonly mono: string;
}

// Soft-prefer Inter / JetBrains Mono if the user has them installed locally;
// otherwise fall through to the closest-looking system fonts. No webfont
// shipped, no @font-face declared, no preload, no CSP-inline-style escape
// hatch needed: the page just uses native typography.
export const fontStack: FontStack = {
  sans: "'Inter', system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif",
  mono: "'JetBrains Mono', ui-monospace, Menlo, 'Cascadia Code', 'Cascadia Mono', Consolas, 'Liberation Mono', monospace",
} as const;

export interface BrandSizes {
  /** 13px label size used in dense UI (header nav, footer links, step copy, terminal rows). */
  readonly label: string;
  /** 11px micro size used in monospace meta rows (terminal logo, pipeline labels). */
  readonly micro: string;
}

export const brandSizes: BrandSizes = {
  label: "0.8125rem",
  micro: "0.6875rem",
} as const;
