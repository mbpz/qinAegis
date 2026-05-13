import { useState, useEffect } from 'react';
import Sidebar from './components/Sidebar';
import Dashboard from './components/Dashboard';
import ExploreView from './components/ExploreView';
import GenerateView from './components/GenerateView';
import RunView from './components/RunView';
import ReportView from './components/ReportView';
import SettingsView from './components/SettingsView';
import './styles.css';

type View = 'dashboard' | 'explore' | 'generate' | 'run' | 'reports' | 'settings';

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
    getReports: () => Promise<any>;
    getGateStatus: () => Promise<any>;
  }
}

function App() {
  const [currentView, setCurrentView] = useState<View>('dashboard');
  const [projects, setProjects] = useState<string[]>([]);
  const [output, setOutput] = useState<string>('');

  useEffect(() => {
    loadProjects();
    const interval = setInterval(loadOutput, 2000);
    return () => clearInterval(interval);
  }, []);

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
      // ignore
    }
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
      case 'settings':
        return <SettingsView />;
      default:
        return <Dashboard projects={projects} onNavigate={setCurrentView} />;
    }
  };

  return (
    <div className="app">
      <header className="app-header">
        <h1>QinAegis</h1>
        <span className="version">v0.1.0</span>
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