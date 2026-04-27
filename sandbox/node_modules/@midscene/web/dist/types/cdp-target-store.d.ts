/**
 * Persistent store for the CDP-mode "current tab" targetId.
 *
 * The Midscene CLI runs each command as a fresh Node process, so anything
 * the previous command knew about which tab was being driven must survive
 * across processes. This store writes the chosen targetId to a temp file
 * after `connect`/`act`/etc. succeed, and the next command reads it back
 * to bind to the exact same tab — even when Chrome holds 14 of them.
 *
 * Owns nothing else. The CDP proxy lifecycle and its own metadata files
 * live in `cdp-proxy-manager.ts`.
 */
/**
 * Read the saved targetId, or null if no command has stored one yet.
 */
export declare function readSavedTargetId(): string | null;
/**
 * Save a targetId so the next CLI command can rebind to the same tab.
 */
export declare function saveTargetId(targetId: string): void;
/**
 * Discard the saved targetId — call when disconnecting or when the
 * upstream Chrome changes (the targetId would point into the old
 * browser's tab list).
 */
export declare function cleanupTargetIdFile(): void;
