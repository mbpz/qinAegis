import { useEffect, useState } from 'react';
import { Button, Select } from './ui';
import { rpc } from '../utils/rpc';
import type { TaskSummary } from '../types/rpc';

export default function TasksView() {
  const [tasks, setTasks] = useState<TaskSummary[]>([]);
  const [selected, setSelected] = useState('');
  const [output, setOutput] = useState('');
  const [error, setError] = useState('');
  const [truncated, setTruncated] = useState(false);
  const [stopping, setStopping] = useState(false);
  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let cursor = 0;
    setOutput(''); setTruncated(false);
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
        }
      } catch (error) { if (!cancelled) setError((error as Error).message); }
      if (!cancelled) timer = setTimeout(poll, document.hidden ? 5000 : 1000);
    }
    void poll();
    return () => { cancelled = true; if (timer) clearTimeout(timer); };
  }, [selected]);
  const current = tasks.find(task => task.jobId === selected);
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
        {current.error && <p role="alert">{current.error}</p>}
        {['running', 'cancelling'].includes(current.status) && <Button disabled={stopping || current.status === 'cancelling'} onClick={async () => {
          setStopping(true);
          try { await rpc.stopJob(current.jobId); }
          catch (error) { setError((error as Error).message); }
          finally { setStopping(false); }
        }}>Stop task</Button>}
        {truncated && <p>Earlier log bytes exceeded the retention limit. This view shows the retained tail; immutable execution evidence remains in Reports.</p>}
        <div className="output-log" aria-label="Task log">{output || 'No log output.'}</div>
      </>}
    </section>
  </div>;
}
