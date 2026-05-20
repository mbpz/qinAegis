import { useState, useEffect } from 'react';
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

  useEffect(() => {
    if (!jobId) {
      setStatus('idle');
      setOutput('');
      setError(null);
      return;
    }

    let cancelled = false;

    const poll = async () => {
      try {
        const s = await rpc.getJobStatus(jobId);
        if (cancelled) return;
        setStatus(s.status as JobStatusValue);
        setOutput(s.output);
        if (s.error) setError(s.error);

        if (s.status === 'completed' && onComplete) {
          onComplete(s.result);
        }
        if ((s.status === 'failed' || s.status === 'stopped') && s.error && onError) {
          onError(s.error);
        }
      } catch (e) {
        if (!cancelled) {
          setError('Failed to get job status');
        }
      }
    };

    poll();
    const interval = setInterval(poll, 2000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [jobId]);

  if (status === 'idle' || !jobId) return null;

  return (
    <div className="job-progress">
      {status === 'running' && (
        <div className="job-running">
          <div className="spinner" />
          <span>Running...</span>
          <button className="btn btn-danger" onClick={() => rpc.stopJob(jobId)}>Stop</button>
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