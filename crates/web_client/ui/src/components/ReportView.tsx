export default function ReportView() {
  return (
    <div className="view">
      <h2 className="view-title">Test Reports</h2>

      <div className="card">
        <div className="card-title">Recent Runs</div>
        <p style={{ color: 'var(--text-secondary)', fontSize: '14px' }}>
          No test reports available. Run some tests to see reports here.
        </p>
      </div>

      <div className="card">
        <div className="card-title">Quality Gate Status</div>
        <div className="gate-status">
          <div className="gate-card">
            <div className="gate-title">E2E Pass Rate</div>
            <div className="gate-value">--</div>
          </div>
          <div className="gate-card">
            <div className="gate-title">Performance</div>
            <div className="gate-value">--</div>
          </div>
          <div className="gate-card">
            <div className="gate-title">Stress</div>
            <div className="gate-value">--</div>
          </div>
        </div>
      </div>
    </div>
  );
}