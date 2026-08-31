/** The 10 screens of the redesigned shell (see tiro-teardown/redesign/mockups). */
export type RouteId = "s1" | "s2" | "s3" | "s4" | "s5" | "s6" | "s7" | "s8" | "s9" | "s10";

/** Routes that open a specific note carry its recordingId; the rest take no params. */
export type Route =
  | { id: "s1" | "s2" | "s3" | "s5" | "s6" | "s8" }
  | { id: "s4" | "s7" | "s9" | "s10"; recordingId: string };

/** One representative Route per screen id, for exhaustive route tests. */
export const ROUTES: readonly Route[] = [
  { id: "s1" },
  { id: "s2" },
  { id: "s3" },
  { id: "s4", recordingId: "mock-recording" },
  { id: "s5" },
  { id: "s6" },
  { id: "s7", recordingId: "mock-recording" },
  { id: "s8" },
  { id: "s9", recordingId: "mock-recording" },
  { id: "s10", recordingId: "mock-recording" },
];
