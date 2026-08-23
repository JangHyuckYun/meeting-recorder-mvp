import animate from "tailwindcss-animate";

/**
 * Tailwind maps 1:1 onto the token layer in src/styles/global.css.
 * Every value here dereferences a CSS custom property — no literal colors,
 * no literal radii, no literal shadows.
 *
 * @type {import("tailwindcss").Config}
 */
export default {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        background: "hsl(var(--background) / <alpha-value>)",
        foreground: "hsl(var(--foreground) / <alpha-value>)",
        surface: {
          DEFAULT: "hsl(var(--surface) / <alpha-value>)",
          sunken: "hsl(var(--surface-sunken) / <alpha-value>)",
        },
        border: {
          DEFAULT: "hsl(var(--border) / <alpha-value>)",
          strong: "hsl(var(--border-strong) / <alpha-value>)",
        },
        input: "hsl(var(--input) / <alpha-value>)",
        ring: "hsl(var(--ring) / <alpha-value>)",
        card: {
          DEFAULT: "hsl(var(--card) / <alpha-value>)",
          foreground: "hsl(var(--card-foreground) / <alpha-value>)",
        },
        popover: {
          DEFAULT: "hsl(var(--popover) / <alpha-value>)",
          foreground: "hsl(var(--popover-foreground) / <alpha-value>)",
        },
        primary: {
          DEFAULT: "hsl(var(--primary) / <alpha-value>)",
          foreground: "hsl(var(--primary-foreground) / <alpha-value>)",
          soft: "hsl(var(--primary-soft) / <alpha-value>)",
          "soft-foreground": "hsl(var(--primary-soft-foreground) / <alpha-value>)",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary) / <alpha-value>)",
          foreground: "hsl(var(--secondary-foreground) / <alpha-value>)",
        },
        muted: {
          DEFAULT: "hsl(var(--muted) / <alpha-value>)",
          foreground: "hsl(var(--muted-foreground) / <alpha-value>)",
        },
        subtle: {
          foreground: "hsl(var(--subtle-foreground) / <alpha-value>)",
        },
        accent: {
          DEFAULT: "hsl(var(--accent) / <alpha-value>)",
          foreground: "hsl(var(--accent-foreground) / <alpha-value>)",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive) / <alpha-value>)",
          foreground: "hsl(var(--destructive-foreground) / <alpha-value>)",
          soft: "hsl(var(--destructive-soft) / <alpha-value>)",
          "soft-foreground": "hsl(var(--destructive-soft-foreground) / <alpha-value>)",
        },
        success: {
          DEFAULT: "hsl(var(--success) / <alpha-value>)",
          foreground: "hsl(var(--success-foreground) / <alpha-value>)",
          soft: "hsl(var(--success-soft) / <alpha-value>)",
          "soft-foreground": "hsl(var(--success-soft-foreground) / <alpha-value>)",
        },
        warning: {
          DEFAULT: "hsl(var(--warning) / <alpha-value>)",
          foreground: "hsl(var(--warning-foreground) / <alpha-value>)",
          soft: "hsl(var(--warning-soft) / <alpha-value>)",
          "soft-foreground": "hsl(var(--warning-soft-foreground) / <alpha-value>)",
        },
        info: {
          DEFAULT: "hsl(var(--info) / <alpha-value>)",
          soft: "hsl(var(--info-soft) / <alpha-value>)",
          "soft-foreground": "hsl(var(--info-soft-foreground) / <alpha-value>)",
        },
        state: {
          partial: "hsl(var(--state-partial) / <alpha-value>)",
          stable: "hsl(var(--state-stable) / <alpha-value>)",
          committed: "hsl(var(--state-committed) / <alpha-value>)",
          revised: "hsl(var(--state-revised) / <alpha-value>)",
        },
        speaker: {
          1: "hsl(var(--speaker-1) / <alpha-value>)",
          2: "hsl(var(--speaker-2) / <alpha-value>)",
          3: "hsl(var(--speaker-3) / <alpha-value>)",
          4: "hsl(var(--speaker-4) / <alpha-value>)",
          5: "hsl(var(--speaker-5) / <alpha-value>)",
          6: "hsl(var(--speaker-6) / <alpha-value>)",
          unknown: "hsl(var(--speaker-unknown) / <alpha-value>)",
        },
        wave: {
          active: "hsl(var(--wave-active) / <alpha-value>)",
          idle: "hsl(var(--wave-idle) / <alpha-value>)",
          baseline: "hsl(var(--wave-baseline) / <alpha-value>)",
          cursor: "hsl(var(--wave-cursor) / <alpha-value>)",
          overlap: "hsl(var(--wave-overlap) / <alpha-value>)",
        },
      },
      borderRadius: {
        xs: "var(--radius-xs)",
        sm: "var(--radius-sm)",
        md: "var(--radius-md)",
        lg: "var(--radius-lg)",
        xl: "var(--radius-xl)",
        "2xl": "var(--radius-2xl)",
      },
      boxShadow: {
        xs: "var(--shadow-xs)",
        sm: "var(--shadow-sm)",
        md: "var(--shadow-md)",
        lg: "var(--shadow-lg)",
        overlay: "var(--shadow-overlay)",
      },
      fontFamily: {
        sans: "var(--font-sans)",
        mono: "var(--font-mono)",
      },
      fontSize: {
        eyebrow: ["0.625rem", { lineHeight: "1", letterSpacing: "0.14em", fontWeight: "800" }],
        caption: ["0.6875rem", { lineHeight: "1.45" }],
        label: ["0.75rem", { lineHeight: "1.4", fontWeight: "700" }],
        body: ["0.8125rem", { lineHeight: "1.6" }],
        "body-lg": ["0.875rem", { lineHeight: "1.65" }],
        h3: ["1rem", { lineHeight: "1.4", letterSpacing: "-0.015em", fontWeight: "700" }],
        h2: ["1.25rem", { lineHeight: "1.3", letterSpacing: "-0.025em", fontWeight: "700" }],
        h1: ["1.625rem", { lineHeight: "1.2", letterSpacing: "-0.035em", fontWeight: "700" }],
        display: [
          "clamp(2.375rem, 5vw, 3.625rem)",
          { lineHeight: "1", letterSpacing: "-0.04em", fontWeight: "650" },
        ],
        timecode: ["0.75rem", { lineHeight: "1", letterSpacing: "0.02em" }],
      },
      spacing: {
        rail: "var(--rail-width)",
        statusbar: "var(--statusbar-height)",
      },
      transitionDuration: {
        fast: "var(--motion-fast)",
        base: "var(--motion-base)",
        slow: "var(--motion-slow)",
      },
      transitionTimingFunction: {
        out: "var(--ease-out)",
        "in-out": "var(--ease-in-out)",
      },
      keyframes: {
        "pulse-dot": {
          "50%": { opacity: "0.4" },
        },
        "sweep-x": {
          "0%": { transform: "translateX(-100%)" },
          "100%": { transform: "translateX(100%)" },
        },
      },
      animation: {
        "pulse-dot": "pulse-dot 1.6s var(--ease-in-out) infinite",
        "sweep-x": "sweep-x 1.8s var(--ease-in-out) infinite",
      },
    },
  },
  plugins: [animate],
};
