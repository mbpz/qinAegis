import { View } from '../App';

interface SidebarProps {
  currentView: View;
  onNavigate: (view: View) => void;
}

const items: { view: View; icon: string; label: string }[] = [
  { view: 'dashboard', icon: '🏠', label: 'Dashboard' },
  { view: 'explore', icon: '🔍', label: 'Explore' },
  { view: 'generate', icon: '✨', label: 'Generate' },
  { view: 'run', icon: '▶️', label: 'Run Tests' },
  { view: 'reports', icon: '📊', label: 'Reports' },
  { view: 'settings', icon: '⚙️', label: 'Settings' },
];

export default function Sidebar({ currentView, onNavigate }: SidebarProps) {
  return (
    <nav className="sidebar">
      {items.map((item) => (
        <button
          key={item.view}
          className={`sidebar-item ${currentView === item.view ? 'active' : ''}`}
          onClick={() => onNavigate(item.view)}
        >
          <span className="icon">{item.icon}</span>
          <span>{item.label}</span>
        </button>
      ))}
    </nav>
  );
}