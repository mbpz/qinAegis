import test from 'node:test';
import assert from 'node:assert/strict';
import { RunCleanupError, runWithCleanup } from './run_with_cleanup.js';

test('cleanup faults retain passing and failing assertion facts and attempt both cleanup stages', async () => {
  for (const passed of [true, false]) for (const failure of ['browser', 'temporary', 'both']) {
    const calls: string[] = [];
    const result = { case_id: 'reviewed-case', passed, duration_ms: 12,
      error_message: passed ? null : 'original assertion failed', failed_step: passed ? null : 'aiAssert' };
    const actual = await runWithCleanup(async () => result,
      async () => { calls.push('browser'); if (failure !== 'temporary') throw Error('private browser failure'); },
      async () => { calls.push('temporary'); if (failure !== 'browser') throw Error('private disk failure'); });
    assert.deepEqual(calls, ['browser', 'temporary']);
    assert.deepEqual({ ...actual, lifecycle_error: undefined }, { ...result, lifecycle_error: undefined });
    assert.equal(actual.lifecycle_error, failure === 'both' ? 'browser_and_temporary_cleanup_failed' : `${failure}_cleanup_failed`);
    assert.ok(!JSON.stringify(actual).includes('private'));
  }
});

test('successful cleanup adds no fault, while missing execution results keep the original exception', async () => {
  assert.deepEqual(await runWithCleanup(async () => ({passed:true}), async () => {}, async () => {}), {passed:true});
  const original = Error('original execution failure');
  let cleaned = 0;
  await assert.rejects(runWithCleanup(async () => { throw original; }, async () => { cleaned++; }, async () => { cleaned++; }), error => error === original);
  assert.equal(cleaned, 2);
});

test('startup or execution exceptions still carry a fixed cleanup marker without inventing a result', async () => {
  for (const failure of ['browser', 'temporary', 'both']) {
    const original = Error('original execution failure');
    let cleaned = 0;
    await assert.rejects(runWithCleanup(async () => { throw original; },
      async () => { cleaned++; if (failure !== 'temporary') throw Error('private browser failure'); },
      async () => { cleaned++; if (failure !== 'browser') throw Error('private disk failure'); }), error => {
      assert.ok(error instanceof RunCleanupError);
      assert.equal(error.original, original);
      assert.equal(error.lifecycle_error, failure === 'both' ? 'browser_and_temporary_cleanup_failed' : `${failure}_cleanup_failed`);
      assert.ok(!String(error).includes('private'));
      return true;
    });
    assert.equal(cleaned, 2);
  }
});
