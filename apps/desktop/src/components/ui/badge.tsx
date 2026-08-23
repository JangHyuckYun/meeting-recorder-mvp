import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type HTMLAttributes } from "react";

import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center gap-1.5 whitespace-nowrap rounded-full border border-transparent px-2.5 py-1 text-caption font-bold",
  {
    variants: {
      variant: {
        neutral: "bg-muted text-muted-foreground",
        outline: "border-border text-muted-foreground",
        primary: "bg-primary-soft text-primary-soft-foreground",
        success: "bg-success-soft text-success-soft-foreground",
        warning: "bg-warning-soft text-warning-soft-foreground",
        destructive: "bg-destructive-soft text-destructive-soft-foreground",
        solid: "bg-foreground text-background",
        /* Caption lifecycle — mirrors the ASR event contract states. */
        partial: "border-dashed border-state-partial text-state-partial",
        stable: "bg-muted text-state-stable",
        committed: "bg-muted text-state-committed",
        revised: "bg-info-soft text-info-soft-foreground",
      },
      size: {
        sm: "px-2 py-0.5 text-eyebrow tracking-normal",
        md: "",
      },
    },
    defaultVariants: {
      variant: "neutral",
      size: "md",
    },
  },
);

export interface BadgeProps
  extends HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {
  /** Renders a leading status dot in the current text color. */
  dot?: boolean;
  /** Pulses the dot — reserved for genuinely live states such as recording. */
  pulse?: boolean;
}

const Badge = forwardRef<HTMLSpanElement, BadgeProps>(
  ({ className, variant, size, dot = false, pulse = false, children, ...props }, ref) => (
    <span ref={ref} className={cn(badgeVariants({ variant, size }), className)} {...props}>
      {dot && (
        <span
          aria-hidden="true"
          className={cn("size-1.5 rounded-full bg-current", pulse && "animate-pulse-dot")}
        />
      )}
      {children}
    </span>
  ),
);
Badge.displayName = "Badge";

export { Badge, badgeVariants };
