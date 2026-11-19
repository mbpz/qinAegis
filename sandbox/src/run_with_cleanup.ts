/** Keep completed assertion facts separate from browser/workspace cleanup faults. */
export type LifecycleError = 'browser_cleanup_failed' | 'temporary_cleanup_failed' | 'browser_and_temporary_cleanup_failed';
export class RunCleanupError extends Error {
  constructor(readonly original: unknown, readonly lifecycle_error: LifecycleError) {
    super('Execution failed and resource cleanup was not confirmed');
  }
}
export async function runWithCleanup<T extends object>(
  run: () => Promise<T>,
  closeBrowser: () => Promise<void>,
  removeTemporary: () => Promise<void>,
): Promise<T & { lifecycle_error?: LifecycleError }> {
  let result: T | undefined;
  let failed = false;
  let original: unknown;
  try { result = await run(); }
  catch (error) { failed = true; original = error; }
  let browserFailed = false, temporaryFailed = false;
  try { await closeBrowser(); } catch { browserFailed = true; }
  try { await removeTemporary(); } catch { temporaryFailed = true; }
  const lifecycle_error = browserFailed && temporaryFailed ? 'browser_and_temporary_cleanup_failed'
    : browserFailed ? 'browser_cleanup_failed' : temporaryFailed ? 'temporary_cleanup_failed' : undefined;
  // Preserve the original execution error while also retaining the cleanup fact,
  // even if startup failed before an assertion result existed.
  if (failed) {
    if (lifecycle_error) throw new RunCleanupError(original, lifecycle_error);
    throw original;
  }
  return { ...result!, ...(lifecycle_error ? { lifecycle_error } : {}) };
}
