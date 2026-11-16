import { Button, Select } from './ui';
import { useState, useEffect, useRef } from 'react';
import { rpc } from '../utils/rpc';
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from './motion/collapsible';
import type { RunEvidence, RunReport, GateStatus } from '../types/rpc';


export default function ReportView() {
  const [allureHtml, setAllureHtml] = useState('');
  const [allureVersion, setAllureVersion] = useState('');
  const [allureError, setAllureError] = useState('');
  const [allureBusy, setAllureBusy] = useState(false);
  const [allureMessage, setAllureMessage] = useState('');
  const allureTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const allurePending = useRef(false);
  const [evidence, setEvidence] = useState<RunEvidence | null>(null);
  const [evidenceError, setEvidenceError] = useState('');
  const listRequest = useRef(0);
  const [projects, setProjects] = useState<string[]>([]);
  const [selectedProject, setSelectedProject] = useState('');
  const [reports, setReports] = useState<RunReport[]>([]);
  const [gate, setGate] = useState<GateStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [selectedRun, setSelectedRun] = useState<string | null>(null);
  const [reportHtml, setReportHtml] = useState<string | null>(null);
  const [loadingHtml, setLoadingHtml] = useState(false);
  const [reportError, setReportError] = useState('');
  const reportRequest = useRef(0);

  useEffect(() => {
    rpc.getProjects().then(ps => { setProjects(ps); setSelectedProject(current => ps.includes(current) ? current : ps[0] ?? ''); }).catch(() => {});
  }, []);

  useEffect(() => {
    reportRequest.current++;
    resetAllure();
    setEvidence(null); setEvidenceError('');
    setSelectedRun(null);
    setReportHtml(null);
    setReportError('');
    loadReports();
    return () => { ++reportRequest.current; ++listRequest.current; if (allureTimer.current) clearTimeout(allureTimer.current); allurePending.current = false; };
  }, [selectedProject]);

  const loadReports = async () => {
    const token = ++listRequest.current;
    if (!selectedProject) { setReports([]); setGate(null); setLoading(false); return; }
    setLoading(true);
    try {
      const [reportsData, gateData] = await Promise.all([
        rpc.getReports(selectedProject),
        rpc.getGateStatus(selectedProject),
      ]);
      if (token !== listRequest.current) return;
      setReports(reportsData || []);
      setGate(gateData || null);
    } catch (e) {
      if (token === listRequest.current) setReportError(String(e));
    } finally {
      if (token === listRequest.current) setLoading(false);
    }
  };

  const handleRunClick = async (runId: string) => {
    const requestId = ++reportRequest.current;
    resetAllure();
    setSelectedRun(runId);
    void loadAllure(selectedProject, runId, requestId, true);
    setReportError('');
    setEvidence(null); setEvidenceError('');
    void rpc.getRunEvidence(selectedProject, runId).then(data => { if (requestId === reportRequest.current) setEvidence(data); }).catch(error => { if (requestId === reportRequest.current) setEvidenceError(String(error)); });
    setLoadingHtml(true);
    setReportHtml(null);
    try {
      const result = await rpc.getReportHtml(selectedProject, runId);
      if (requestId !== reportRequest.current) return;
      if (result.ok && result.html) {
        setReportHtml(result.html);
      } else {
        setReportError(result.error || 'Failed to load report');
      }
    } catch (e) {
      if (requestId === reportRequest.current) setReportError('Error: ' + (e as Error).message);
    } finally {
      if (requestId === reportRequest.current) setLoadingHtml(false);
    }
  };

  function resetAllure() {
    if (allureTimer.current) clearTimeout(allureTimer.current);
    allureTimer.current = null; allurePending.current = false;
    setAllureHtml(''); setAllureVersion(''); setAllureError(''); setAllureBusy(false); setAllureMessage('');
  }

  async function loadAllure(project: string, runId: string, token: number, quiet = false) {
    try {
      const result = await rpc.getAllureReportHtml(project, runId);
      if (token !== reportRequest.current) return;
      if (result.ok && result.html) { setAllureHtml(result.html); setAllureVersion(result.version ?? ''); setAllureError(''); }
      else if (!quiet) setAllureError(result.error || 'No Allure HTML artifact is available. Generate it explicitly.');
    } catch (error) { if (!quiet && token === reportRequest.current) setAllureError(String(error)); }
  }

  async function generateAllure() {
    if (!selectedRun || !evidence || allurePending.current) return;
    const token = reportRequest.current;
    const project = selectedProject, runId = selectedRun;
    allurePending.current = true; setAllureBusy(true); setAllureError(''); setAllureMessage('Starting Allure report generation…');
    try {
      const job = await rpc.generateAllureReport(project, runId);
      if (token !== reportRequest.current) return;
      if (!job.ok || !job.jobId) throw new Error('The local service did not start report generation.');
      const poll = async () => {
        if (token !== reportRequest.current) return;
        try {
          const status = await rpc.getJobStatus(job.jobId);
          if (token !== reportRequest.current) return;
          if (status.status === 'completed') {
            allurePending.current = false; setAllureBusy(false); setAllureMessage('Allure generation completed.');
            await loadAllure(project, runId, token);
          } else if (['failed', 'stopped'].includes(status.status)) {
            allurePending.current = false; setAllureBusy(false); setAllureMessage(''); setAllureError(status.error || `Report generation ${status.status}. Execution results are unchanged.`);
          } else {
            setAllureMessage(`Allure generation: ${status.status}`);
            allureTimer.current = setTimeout(() => void poll(), 1000);
          }
        } catch (error) { if (token === reportRequest.current) { allurePending.current = false; setAllureBusy(false); setAllureMessage(''); setAllureError(String(error)); } }
      };
      await poll();
    } catch (error) { if (token === reportRequest.current) { allurePending.current = false; setAllureBusy(false); setAllureMessage(''); setAllureError(String(error)); } }
  }

  const handleExport = async () => {
    try {
      const data = await rpc.exportProject(selectedProject);
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `qinaegis-report-${selectedProject}-${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      setReportError('Export failed: ' + e);
    }
  };

  if (loading) {
    return (
      <div className="view">
        <h2 className="view-title">Test Reports</h2>
        <p style={{ color: 'var(--text-secondary)' }}>Loading...</p>
      </div>
    );
  }

  return (
    <div className="view">
      <h2 className="view-title">Test Reports</h2>

      <div className="card">
        <div className="card-title">Project</div>
        <Select aria-label="Report project"
          className="input"
          value={selectedProject}
          onValueChange={setSelectedProject}
          style={{ maxWidth: '300px' }}
        >
          {projects.map((p) => (
            <option key={p} value={p}>{p}</option>
          ))}
        </Select>
      </div>

      <div className="card">
        <div className="card-title">Quality Gate Status</div>
        <div className="gate-status">
          <div className="gate-card">
            <div className="gate-title">E2E Pass Rate</div>
            <div className="gate-value">
              {gate?.has_runs && (gate.e2e_pass_rate !== null || gate.execution_blocked)
                ? gate.e2e_pass_rate_display
                : '--'}
            </div>
          </div>
          {gate?.execution_blocked && <p role="status">Execution {gate.execution_status ?? 'blocked'}. Review the recorded execution evidence; individual script passes do not clear this gate.</p>}
          <div className="gate-card">
            <div className="gate-title">Performance</div>
            <div className="gate-value">
              {gate?.performance || '--'}
            </div>
          </div>
          <div className="gate-card">
            <div className="gate-title">Stress</div>
            <div className="gate-value">
              {gate?.stress || '--'}
            </div>
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-title">Recent Runs</div>
        {reports.length === 0 ? (
          <p style={{ color: 'var(--text-secondary)', fontSize: '14px' }}>
            No test reports available. Run some tests to see reports here.
          </p>
        ) : (
          <div className="run-list">
            {reports.slice(0, 10).map((report) => (
              <Button
                key={report.run_id}
                variant="ghost"
                aria-pressed={selectedRun === report.run_id}
                className={`run-item ${selectedRun === report.run_id ? 'selected' : ''}`}
                onClick={() => handleRunClick(report.run_id)}
                style={{ width: '100%', height: 'auto', textAlign: 'left', flexWrap: 'wrap' }}
              >
                <span className="run-id">{report.run_id}</span>
                <span className="run-stats">
                  <span className="stat passed">{report.passed} passed</span>
                  <span className="stat failed">{report.failed} {report.has_evidence ? 'failed or not executed' : 'failed'}</span>
                  <span className="stat total">{report.total} total</span>
                  {report.status && <span>{report.status}</span>}
                  {report.executed !== undefined && <span>{report.executed}/{report.total} executed</span>}
                </span>
              </Button>
            ))}
          </div>
        )}
      </div>

      {selectedRun && (
        <div className="card">
          <div className="card-title">Report: {selectedRun}</div>
          {evidenceError && <p role="status">Execution evidence unavailable: {evidenceError}. Legacy reports may have no recorded source.</p>}
          {evidence && <section aria-label="Execution evidence"><p>Status: {evidence.status} · Engine: {evidence.engine_version ?? 'unrecorded'} · {evidence.started_at}</p><p>Contract semantic validation: not independently verified. Script execution results and recorded generation sources follow.</p>{evidence.error && <p role="alert">{evidence.error}</p>}
            {evidence.results.map(result => <p key={result.case_id}>{result.case_id}: {result.passed ? 'script passed' : 'script failed'} · {result.duration_ms} ms {result.error_message}</p>)}
            <Collapsible><CollapsibleTrigger>Frozen inputs, sources and result evidence</CollapsibleTrigger><CollapsibleContent><pre style={{whiteSpace: 'pre-wrap', overflowWrap: 'anywhere'}}>{JSON.stringify(evidence, null, 2)}</pre></CollapsibleContent></Collapsible>
            <p>This download contains project inputs, scripts and screenshots. Review it before sharing.</p>
            <Button onClick={() => { const url = URL.createObjectURL(new Blob([JSON.stringify(evidence, null, 2)], {type: 'application/json'})); const link = document.createElement('a'); link.href = url; link.download = `${evidence.project}-${evidence.run_id}-evidence.json`; link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000); }}>Download run evidence</Button>
          </section>}
          <section aria-label="Allure report">
            <h3>Allure report {allureVersion && `· ${allureVersion}`}</h3>
            <p>Generate the official Allure presentation from this run's recorded results. Report generation does not rerun tests or change the quality gate.</p>
            {!evidence && evidenceError && <p>Allure generation requires a saved execution record. Summary-only legacy reports cannot be converted.</p>}
            <Button disabled={!evidence || allureBusy || !!allureHtml} onClick={() => void generateAllure()}>{allureBusy ? 'Generating Allure…' : allureHtml ? 'Allure generated' : 'Generate Allure report'}</Button>{' '}
            <Button disabled={allureBusy} onClick={() => { if (selectedRun) void loadAllure(selectedProject, selectedRun, reportRequest.current); }}>View saved Allure report</Button>
            {allureMessage && <p role="status">{allureMessage}</p>}
            {allureError && <p role="alert">Allure rendering: {allureError}</p>}
            {allureHtml && <>
              <p>The self-contained HTML includes result details and attachments. Review it before sharing.</p>
              <Button onClick={() => { const url = URL.createObjectURL(new Blob([allureHtml], {type: 'text/html'})); const link = document.createElement('a'); link.href = url; link.download = `${selectedProject}-${selectedRun}-allure.html`; link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000); }}>Download Allure HTML</Button>
              {/* Navigate to the authenticated HTML endpoint so the report receives its own CSP; srcDoc would inherit the main page script restriction. */}
              <iframe title={`Allure ${selectedRun}`} sandbox="allow-scripts" referrerPolicy="no-referrer" src={`/api/v2/allure/${encodeURIComponent(selectedProject)}/${encodeURIComponent(selectedRun)}`} style={{width: '100%', height: '700px', border: '1px solid var(--border-color)', marginTop: '12px'}} />
            </>}
          </section>
          {loadingHtml ? (
            <p style={{ color: 'var(--text-secondary)' }}>Loading report...</p>
          ) : reportError && evidence && /report\.html.*not found|report\.html.*missing/i.test(reportError) ? (
            !allureHtml && <p>This run records structured execution evidence above. A separate HTML report was not generated.</p>
          ) : reportError ? (
            <p className="error-message" role="alert">{reportError}</p>
          ) : reportHtml ? (
            <iframe
              title={`Report ${selectedRun}`}
              className="report-viewer"
              sandbox=""
              referrerPolicy="no-referrer"
              srcDoc={'<!doctype html><meta http-equiv="Content-Security-Policy" content="default-src \'none\'; style-src \'unsafe-inline\'; img-src data:; base-uri \'none\'; form-action \'none\'">' + reportHtml}
              style={{
                background: '#0d1117',
                border: '1px solid var(--border-color)',
                borderRadius: '8px',
                padding: '16px',
                width: '100%',
                height: '500px',
              }}
            />
          ) : (
            <p style={{ color: 'var(--text-secondary)' }}>No HTML report available for this run.</p>
          )}
        </div>
      )}

      <div className="card">
        <div className="card-title">Export</div>
        <p style={{ color: 'var(--text-secondary)', fontSize: '14px', marginBottom: '12px' }}>
          Export all test reports and results as JSON for sharing or archival.
        </p>
        <Button className="btn btn-primary" onClick={handleExport}>
          Export All Reports (JSON)
        </Button>
      </div>
    </div>
  );
}
