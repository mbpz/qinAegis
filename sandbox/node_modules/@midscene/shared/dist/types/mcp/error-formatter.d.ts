/**
 * Extract a human-readable message from an unknown thrown value.
 *
 * Many SDK/transport layers reject with structured objects (e.g.
 * `{ code, message }`, `{ error: { message } }`, `{ cause: { message } }`)
 * rather than `Error` instances. `String(obj)` collapses those to
 * `"[object Object]"`, which is useless for diagnostics. This helper walks
 * the common shapes, falls back to `JSON.stringify`, and finally to
 * `Object.prototype.toString.call` so that callers always get something
 * actionable in logs and surfaced tool results.
 */
export declare function getErrorMessage(error: unknown): string;
