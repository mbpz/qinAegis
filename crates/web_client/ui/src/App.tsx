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
import './styles.css';

type View = 'dashboard' | 'explore' | 'generate' | 'run' | 'review' | 'reports' | 'settings';

declare global {
  interface Window {
    rpc: (method: string, params: object) => Promise<any>;
    getState: () => Promise<any>;
    setConfig: (config: any) => Promise<any>;
    runExplore: (url: string, depth: number) => Promise<any>;
    runGenerate: (requirement: string, spec?: string) => Promise<any>;
    runTests: (project: string, type: string) => Promise<any>;
    getOutput: () => Promise<any>;
    clearOutput: () => Promise<any>;
    getProjects: () => Promise<any>;
    getReports: (project: string) => Promise<any>;
    getGateStatus: (project: string) => Promise<any>;
    createProject: (name: string, url: string, tech_stack: string[]) => Promise<any>;
    checkConfig: () => Promise<{configured: boolean}>;
    getReportHtml: (project: string, run_id: string) => Promise<any>;
    exportProject: (project: string) => Promise<any>;
    getReviewCases: (project: string) => Promise<any>;
    updateCaseStatus: (project: string, case_id: string, status: string) => Promise<any>;
    previewTestPlan: (project: string, testType: string) => Promise<any>;
    getVersion: () => Promise<any>;
    checkUpdate: () => Promise<any>;
  }
}

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
      const v = await window.getVersion();
      setAppVersion(v.version || 'unknown');
      const status = await window.checkUpdate();
      setUpdateStatus(status.upToDate ? null : { hasUpdate: true, latest: status.latest });
    } catch (e) {
      console.error('Failed to load version:', e);
    }
  };

  const checkConfig = async () => {
    try {
      const result = await window.checkConfig();
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
      const result = await window.getProjects();
      setProjects(result || []);
    } catch (e) {
      console.error('Failed to load projects:', e);
    }
  };

  const loadOutput = async () => {
    try {
      const result = await window.getOutput();
      setOutput(result || '');
    } catch (e) {
      console.error('Failed to fetch output:', e);
    }
  };

  const handleWizardComplete = () => {
    setIsConfigured(true);
  };

  const renderView = () => {
    switch (currentView) {
      case 'dashboard':
        return <Dashboard projects={projects} onNavigate={setCurrentView} />;
      case 'explore':
        return <ExploreView output={output} onClear={async () => { await window.clearOutput(); setOutput(''); }} />;
      case 'generate':
        return <GenerateView output={output} onClear={async () => { await window.clearOutput(); setOutput(''); }} />;
      case 'run':
        return <RunView output={output} onClear={async () => { await window.clearOutput(); setOutput(''); }} />;
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
        <span className="version">v{appVersion}</span>
        {updateStatus?.hasUpdate && (
          <span
            className="update-badge"
            title={`New version available: ${updateStatus.latest}`}
            onClick={() => window.open('https://github.com/your-repo/releases', '_blank')}
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
