import { useState, useEffect } from 'react';

interface RunReport {
  run_id: string;
  total: number;
  passed: number;
  failed: number;
}

interface GateStatus {
  e2e_pass_rate: number | null;
  e2e_pass_rate_display: string;
  performance: string | null;
  stress: string | null;
  has_runs: boolean;
  last_run_id?: string;
  last_run_passed?: number;
  last_run_total?: number;
}

export default function ReportView() {
  const [reports, setReports] = useState<RunReport[]>([]);
  const [gate, setGate] = useState<GateStatus | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadReports();
  }, []);

  const loadReports = async () => {
    setLoading(true);
    try {
      const [reportsData, gateData] = await Promise.all([
        window.getReports('default'),
        window.getGateStatus('default'),
      ]);
      setReports(reportsData || []);
      setGate(gateData || null);
    } catch (e) {
      console.error('Failed to load reports:', e);
    } finally {
      setLoading(false);
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
        <div className="card-title">Quality Gate Status</div>
        <div className="gate-status">
          <div className="gate-card">
            <div className="gate-title">E2E Pass Rate</div>
            <div className="gate-value">
              {gate?.has_runs && gate?.e2e_pass_rate !== null
                ? gate.e2e_pass_rate_display
                : '--'}
            </div>
          </div>
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
              <div key={report.run_id} className="run-item">
                <div className="run-id">{report.run_id}</div>
                <div className="run-stats">
                  <span className="stat passed">{report.passed} passed</span>
                  <span className="stat failed">{report.failed} failed</span>
                  <span className="stat total">{report.total} total</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
