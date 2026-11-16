import { Button } from './components/ui';
import { AnimatedToastStack, useAnimatedToastStack } from './components/motion/animated-toast-stack';
import { useState, useEffect, useRef } from 'react';
import Sidebar from './components/Sidebar';
import Dashboard from './components/Dashboard';
import ExploreView from './components/ExploreView';
import GenerateView from './components/GenerateView';
import RunView from './components/RunView';
import ReportView from './components/ReportView';
import ReviewView from './components/ReviewView';
import SettingsView from './components/SettingsView';
import ContractsView from './components/ContractsView';
import ModelsView from './components/ModelsView';
import { rpc } from './utils/rpc';
import './styles.css';

type View = 'dashboard' | 'explore' | 'generate' | 'run' | 'review' | 'reports' | 'settings' | 'contracts' | 'models';

function App() {
  const [currentView, setCurrentView] = useState<View>('dashboard');
  const [projects, setProjects] = useState<string[]>([]);
  const projectsRequest = useRef(0);
  const [isConfigured, setIsConfigured] = useState<boolean | null>(null);
  const [connectionError, setConnectionError] = useState('');
  const [appVersion, setAppVersion] = useState<string>('');
  const [updateStatus, setUpdateStatus] = useState<{hasUpdate: boolean; latest: string} | null>(null);
  const [updateCheckStatus, setUpdateCheckStatus] = useState<'idle' | 'checking' | 'upToDate' | 'available'>('idle');
  const { toasts, showToast, dismissToast } = useAnimatedToastStack({ limit: 3, defaultDuration: 4200 });

  useEffect(() => {
    checkConfig();
    loadProjects();
    loadVersion();
  }, []);

  const loadVersion = async () => {
    try {
      const v = await rpc.getVersion();
      setAppVersion(v.version || 'unknown');
    } catch (e) {
      console.error('Failed to load version:', e);
    }
  };

  const checkForUpdate = async () => {
    setUpdateCheckStatus('checking');
    try {
      const status = await rpc.checkUpdate();
      if (status.upToDate) {
        setUpdateCheckStatus('upToDate');
        setUpdateStatus(null);
        showToast({ title: 'Already latest', status: 'success' });
      } else {
        setUpdateCheckStatus('available');
        setUpdateStatus({ hasUpdate: true, latest: status.latest });
        showToast({ title: 'New version available', description: status.latest, status: 'info' });
      }
    } catch (e) {
      console.error('Failed to check update:', e);
      setUpdateCheckStatus('idle');
      showToast({ title: 'Could not check for updates', description: 'Please try again later.', status: 'error' });
    }
  };

  const checkConfig = async () => {
    try {
      const result = await rpc.checkConfig();
      setIsConfigured(result.configured);
      setConnectionError('');
    } catch (e) {
      console.error('Failed to check config:', e);
      setConnectionError((e as Error).message);
    }
  };

  const loadProjects = async () => {
    const request = ++projectsRequest.current;
    try {
      const result = await rpc.getProjects();
      if (projectsRequest.current === request) setProjects(result);
    } catch (e) {
      console.error('Failed to load projects:', e);
      if (projectsRequest.current === request) setConnectionError('Could not refresh projects: ' + String(e));
    }
  };

  const renderView = () => {
    switch (currentView) {
      case 'dashboard':
        return <Dashboard projects={projects} onNavigate={setCurrentView} onProjectsChanged={loadProjects} />;
      case 'explore':
        return <ExploreView />;
      case 'generate':
        return <GenerateView projects={projects} />;
      case 'run':
        return <RunView />;
      case 'reports':
        return <ReportView />;
      case 'review':
        return <ReviewView />;
      case 'settings':
        return <SettingsView onSaved={checkConfig} />;
      case 'contracts':
        return <ContractsView projects={projects} />;
      case 'models':
        return <ModelsView onSaved={checkConfig} />;
      default:
        return <Dashboard projects={projects} onNavigate={setCurrentView} onProjectsChanged={loadProjects} />;
    }
  };

  return (
    <div className="app">
      <header className="app-header">
        <h1>QinAegis</h1>
        <Button
          variant="ghost"
          className="version"
          onClick={checkForUpdate}
          disabled={updateCheckStatus === 'checking'}
          title="Click to check for updates"
        >
          {updateCheckStatus === 'checking' ? 'Checking...' : appVersion}
        </Button>
        {updateStatus?.hasUpdate && (
          <Button
            variant="ghost"
            className="update-badge"
            title={`New version available: ${updateStatus.latest}`}
            onClick={() => { window.open('https://github.com/mbpz/qinAegis/releases', '_blank', 'noopener,noreferrer'); }}
          >
            Update
          </Button>
        )}
      </header>
      <div className="app-body">
        <Sidebar currentView={currentView} onNavigate={setCurrentView} />
        <main className="app-main">
          {connectionError && <div className="card" role="alert">
            <p>Cannot connect to the local service: {connectionError}</p>
            <Button className="btn btn-primary" onClick={() => { checkConfig(); loadProjects(); loadVersion(); }}>Retry connection</Button>
          </div>}
          {isConfigured === false && currentView !== 'settings' && <div className="card">
            <p>AI is not configured. You can browse projects and reports, or configure a model to run AI tasks.</p>
            <Button className="btn btn-primary" onClick={() => setCurrentView('settings')}>Configure AI model</Button>
          </div>}
          {renderView()}
        </main>
      </div>
      <AnimatedToastStack toasts={toasts} onDismiss={dismissToast} maxVisible={3} placement="fixed" position="bottom-right" />
    </div>
  );
}

export type { View };
export default App;
