import { useEffect, useState } from 'react';
import { Button, Select } from './ui';
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from './motion/collapsible';
import { rpc } from '../utils/rpc';
import type { TaskSummary, JobStatus } from '../types/rpc';
import { generationEvidence } from '../utils/generation_evidence';

export default function TasksView() {
  const [tasks, setTasks] = useState<TaskSummary[]>([]);
  const [selected, setSelected] = useState('');
  const [output, setOutput] = useState('');
  const [error, setError] = useState('');
  const [truncated, setTruncated] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [details, setDetails] = useState<JobStatus | null>(null);
  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let cursor = 0;
    setOutput(''); setTruncated(false); setDetails(null);
    async function poll() {
      try {
        const result = await rpc.getJobs();
        if (cancelled) return;
        setTasks(result.jobs); setError(result.history_error || '');
        if (!selected && result.jobs.length) { setSelected(result.jobs[0].jobId); return; }
        if (selected) {
          const log = await rpc.getJobLog(selected, cursor);
          if (cancelled) return;
          cursor = log.cursor;
          if (log.truncated) setTruncated(true);
          setOutput(previous => (previous + log.output).slice(-65536));
          if (result.jobs.some(task => task.jobId === selected && task.jobType === 'generate')) {
            const status = await rpc.getJobStatus(selected);
            if (cancelled) return;
            setDetails(status);
          }
        }
      } catch (error) { if (!cancelled) setError((error as Error).message); }
      if (!cancelled) timer = setTimeout(poll, document.hidden ? 5000 : 1000);
    }
    void poll();
    return () => { cancelled = true; if (timer) clearTimeout(timer); };
  }, [selected]);
  const current = tasks.find(task => task.jobId === selected);
  const evidence = details?.jobId === selected ? generationEvidence(details.result, details.snapshot) : null;
  return <div className="view">
    <h2>Task history</h2>
    <p>Recent task state and bounded logs are saved locally. Tasks interrupted by service exit are retained for inspection and are never replayed automatically.</p>
    {error && <p role="alert">{error}</p>}
    <section className="card">
      <label htmlFor="task-history-select">Task</label>
      <Select id="task-history-select" value={selected} onValueChange={setSelected}>
        <option value="">Select task</option>
        {tasks.map(task => <option key={task.jobId} value={task.jobId}>{task.jobType} · {task.status} · {task.jobId}</option>)}
      </Select>
      {!tasks.length && <p>No saved tasks.</p>}
      {current && <>
        <p role="status">{current.jobType}: {current.status}</p>
        {current.status === 'queued' && <p>Waiting for the single execution slot. Position: {current.queue_position ?? 'pending'}. No actions have started.</p>}
        {current.error && <p role="alert">{current.error}</p>}
        {['queued', 'running', 'cancelling'].includes(current.status) && <Button disabled={stopping || current.status === 'cancelling'} onClick={async () => {
          setStopping(true);
          try { await rpc.stopJob(current.jobId); }
          catch (error) { setError((error as Error).message); }
          finally { setStopping(false); }
        }}>Stop task</Button>}
        {truncated && <p>Earlier log bytes exceeded the retention limit. This view shows the retained tail; immutable execution evidence remains in Reports.</p>}
        {current.jobType === 'generate' && details?.jobId === selected && <section aria-label="Generation model evidence">
          <h3>Actual model attempts</h3>
          {evidence ? <>
            <p role="status">{current.status === 'interrupted' ? 'Generation interrupted; last recorded attempt is retained' : evidence.status}</p>
            {current.status === 'interrupted' && <p>The service exited. A started attempt does not prove provider completion; no request was replayed.</p>}
            {!evidence.legacy && <Collapsible><CollapsibleTrigger>Inspect attempts and frozen model inputs</CollapsibleTrigger><CollapsibleContent>
              {evidence.rows.length ? evidence.rows.map(row => <p key={row.key}><strong>{row.phase} · {row.candidate}</strong> ({row.model}): {row.outcome}</p>) : <p>No model attempts have been recorded.</p>}
              <pre style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere'}}>{JSON.stringify({snapshot:details.snapshot,evidence:details.result}, null, 2)}</pre>
            </CollapsibleContent></Collapsible>}
          </> : <p>Actual model evidence is not available for this task.</p>}
        </section>}
        <div className="output-log" aria-label="Task log">{output || 'No log output.'}</div>
      </>}
    </section>
  </div>;
}
