import { Button, Loader } from './ui';
import { useState, useEffect, useRef } from 'react';
import { rpc } from '../utils/rpc';
import type { JobStatusValue } from '../types/rpc';

interface JobProgressProps {
  jobId: string | null;
  onComplete?: (result: unknown) => void;
  onError?: (error: string) => void;
}

export default function JobProgress({ jobId, onComplete, onError }: JobProgressProps) {
  const [status, setStatus] = useState<JobStatusValue>('idle');
  const [output, setOutput] = useState('');
  const [error, setError] = useState<string | null>(null);

  const callbacks = useRef({ onComplete, onError });
  callbacks.current = { onComplete, onError };

  useEffect(() => {
    if (!jobId) {
      setStatus('idle');
      setOutput('');
      setError(null);
      return;
    }

    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    setStatus('running');
    setOutput('');
    setError(null);

    const poll = async () => {
      try {
        const s = await rpc.getJobStatus(jobId);
        if (cancelled) return;
        setStatus(s.status);
        setOutput(s.output);
        setError(s.error || null);

        if (s.status === 'completed') {
          callbacks.current.onComplete?.(s.result);
          return;
        }
        if (s.status === 'failed' || s.status === 'stopped' || s.status === 'interrupted') {
          callbacks.current.onError?.(s.error || (s.status === 'stopped' ? 'Task stopped' : s.status === 'interrupted' ? 'Task interrupted by service exit' : 'Task failed'));
          return;
        }
      } catch (e) {
        if (!cancelled) {
          setError('Failed to get job status; reconnecting...');
        }
      }
      if (!cancelled) timer = setTimeout(poll, 2000);
    };

    poll();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [jobId]);

  if (status === 'idle' || !jobId) return null;

  return (
    <div className="job-progress">
      {status === 'running' && (
        <div className="job-running">
          <Loader />
          <span>Running...</span>
          <Button className="btn btn-danger" onClick={async () => {
            try {
              await rpc.stopJob(jobId);
              setStatus(current => current === 'running' ? 'cancelling' : current);
              setError(null);
            } catch (e) {
              setError('Failed to stop task: ' + (e as Error).message);
            }
          }}>Stop</Button>
        </div>
      )}
      {status === 'cancelling' && (
        <div className="job-running" role="status">
          <Loader />
          <span>Stopping… waiting for the worker to finish cleanup.</span>
        </div>
      )}
      {status === 'completed' && (
        <div className="job-completed">
          <span className="job-status-badge success">Completed</span>
        </div>
      )}
      {status === 'failed' && (
        <div className="job-failed">
          <span className="job-status-badge error">Failed</span>
        </div>
      )}
      {status === 'interrupted' && <div className="job-failed"><span className="job-status-badge warning">Interrupted — inspect the saved evidence before rerunning.</span></div>}
      {status === 'stopped' && (
        <div className="job-stopped">
          <span className="job-status-badge warning">Stopped</span>
        </div>
      )}
      {output && <div className="output-log">{output}</div>}
      {error && <div className="error-message">{error}</div>}
    </div>
  );
}