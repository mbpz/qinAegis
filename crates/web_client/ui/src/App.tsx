import { useState, useEffect } from 'react';
import Sidebar from './components/Sidebar';
import Dashboard from './components/Dashboard';
import ExploreView from './components/ExploreView';
import GenerateView from './components/GenerateView';
import RunView from './components/RunView';
import ReportView from './components/ReportView';
import ReviewView from './components/ReviewView';
import SettingsView from './components/SettingsView';
import InitWizard from './components/InitWizard';
import { rpcCall } from './utils/rpc';
import './styles.css';

type View = 'dashboard' | 'explore' | 'generate' | 'run' | 'review' | 'reports' | 'settings';

function App() {
  const [currentView, setCurrentView] = useState<View>('dashboard');
  const [projects, setProjects] = useState<string[]>([]);
  const [output, setOutput] = useState<string>('');
  const [isConfigured, setIsConfigured] = useState<boolean | null>(null);
  const [appVersion, setAppVersion] = useState<string>('');
  const [updateStatus, setUpdateStatus] = useState<{hasUpdate: boolean; latest: string} | null>(null);

  useEffect(() => {
    checkConfig();
    loadProjects();
    loadVersion();
  }, []);

  const loadVersion = async () => {
    try {
      const v = await rpcCall('getVersion');
      setAppVersion(v.version || 'unknown');
      const status = await rpcCall('checkUpdate');
      setUpdateStatus(status.upToDate ? null : { hasUpdate: true, latest: status.latest });
    } catch (e) {
      console.error('Failed to load version:', e);
    }
  };

  const checkConfig = async () => {
    try {
      const result = await rpcCall('checkConfig');
      setIsConfigured(result.configured);
    } catch (e) {
      console.error('Failed to check config:', e);
      setIsConfigured(false);
    }
  };

  useEffect(() => {
    if (currentView !== 'explore' && currentView !== 'generate' && currentView !== 'run') {
      return;
    }
    const interval = setInterval(loadOutput, 2000);
    return () => clearInterval(interval);
  }, [currentView]);

  const loadProjects = async () => {
    try {
      const result = await rpcCall('getProjects');
      setProjects(result || []);
    } catch (e) {
      console.error('Failed to load projects:', e);
    }
  };

  const loadOutput = async () => {
    try {
      const result = await rpcCall('getOutput');
      setOutput(result || '');
    } catch (e) {
      console.error('Failed to fetch output:', e);
    }
  };

  const handleClearOutput = async () => {
    await rpcCall('clearOutput', {});
    setOutput('');
  };

  const handleWizardComplete = () => {
    setIsConfigured(true);
  };

  const renderView = () => {
    switch (currentView) {
      case 'dashboard':
        return <Dashboard projects={projects} onNavigate={setCurrentView} />;
      case 'explore':
        return <ExploreView output={output} onClear={handleClearOutput} />;
      case 'generate':
        return <GenerateView output={output} onClear={handleClearOutput} />;
      case 'run':
        return <RunView output={output} onClear={handleClearOutput} />;
      case 'reports':
        return <ReportView />;
      case 'review':
        return <ReviewView />;
      case 'settings':
        return <SettingsView />;
      default:
        return <Dashboard projects={projects} onNavigate={setCurrentView} />;
    }
  };

  if (isConfigured === null) {
    return (
      <div className="app" style={{ justifyContent: 'center', alignItems: 'center' }}>
        <div className="spinner" />
      </div>
    );
  }

  if (!isConfigured) {
    return <InitWizard onComplete={handleWizardComplete} />;
  }

  return (
    <div className="app">
      <header className="app-header">
        <h1>QinAegis</h1>
        <span className="version">{appVersion}</span>
        {updateStatus?.hasUpdate && (
          <span
            className="update-badge"
            title={`New version available: ${updateStatus.latest}`}
            onClick={() => { const a = document.createElement('a'); a.href = 'https://github.com/your-repo/releases'; a.target = '_blank'; a.click(); }}
          >
            Update
          </span>
        )}
      </header>
      <div className="app-body">
        <Sidebar currentView={currentView} onNavigate={setCurrentView} />
        <main className="app-main">
          {renderView()}
        </main>
      </div>
    </div>
  );
}

export type { View };
export default App;
