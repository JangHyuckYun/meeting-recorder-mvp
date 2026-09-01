import { clsx, type ClassValue } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

/* Custom font-size tokens (tailwind.config fontSize). Without this, twMerge
 * reads `text-caption` as a text *color* and drops `text-primary-foreground`. */
const twMerge = extendTailwindMerge({
  extend: {
    classGroups: {
      "font-size": [
        { text: ["eyebrow", "caption", "label", "body", "body-lg", "h3", "h2", "h1", "display"] },
      ],
    },
  },
});

/** Merge conditional class names, letting later Tailwind utilities win. */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
