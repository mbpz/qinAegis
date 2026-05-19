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
  const [updateCheckStatus, setUpdateCheckStatus] = useState<'idle' | 'checking' | 'upToDate' | 'available'>('idle');
  const [toastMessage, setToastMessage] = useState<string | null>(null);

  useEffect(() => {
    checkConfig();
    loadProjects();
    loadVersion();
  }, []);

  const loadVersion = async () => {
    try {
      const v = await rpcCall('getVersion');
      setAppVersion(v.version || 'unknown');
    } catch (e) {
      console.error('Failed to load version:', e);
    }
  };

  const showToast = (msg: string) => {
    setToastMessage(msg);
    setTimeout(() => setToastMessage(null), 2500);
  };

  const checkForUpdate = async () => {
    setUpdateCheckStatus('checking');
    try {
      const status = await rpcCall('checkUpdate');
      if (status.upToDate) {
        setUpdateCheckStatus('upToDate');
        showToast('✓ Already latest');
      } else {
        setUpdateCheckStatus('available');
        setUpdateStatus({ hasUpdate: true, latest: status.latest });
        showToast(`↑ New version: ${status.latest}`);
      }
    } catch (e) {
      console.error('Failed to check update:', e);
      setUpdateCheckStatus('idle');
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

  // Clear output when switching views so each view starts fresh
  useEffect(() => {
    setOutput('');
    // Clear backend output buffer too so old results don't persist
    rpcCall('clearOutput', {}).catch(() => {});
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
      // Handle both direct string and wrapped {output: string} format
      const outputStr = typeof result === 'string' ? result : (result?.output || '');
      setOutput(outputStr);
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
        <span
          className="version"
          onClick={checkForUpdate}
          title="Click to check for updates"
        >
          {updateCheckStatus === 'checking' ? 'Checking...' : appVersion}
        </span>
        {toastMessage && <span className="toast">{toastMessage}</span>}
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
