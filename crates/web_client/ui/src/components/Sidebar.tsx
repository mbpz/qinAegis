import { Button } from './ui';
import { View } from '../App';

interface SidebarProps {
  currentView: View;
  onNavigate: (view: View) => void;
}

const items: { view: View; icon: string; label: string }[] = [
  { view: 'dashboard', icon: '🏠', label: 'Dashboard' },
  { view: 'contracts', icon: '📜', label: 'Contracts' },
  { view: 'models', icon: '🔀', label: 'Models' },
  { view: 'explore', icon: '🔍', label: 'Explore' },
  { view: 'generate', icon: '✨', label: 'Generate' },
  { view: 'review', icon: '🔎', label: 'Review' },
  { view: 'run', icon: '▶️', label: 'Run Tests' },
  { view: 'reports', icon: '📊', label: 'Reports' },
  { view: 'settings', icon: '⚙️', label: 'Settings' },
];

export default function Sidebar({ currentView, onNavigate }: SidebarProps) {
  return (
    <nav className="sidebar">
      {items.map((item) => (
        <Button
          key={item.view}
          variant="ghost"
          aria-current={currentView === item.view ? 'page' : undefined}
          className={`sidebar-item ${currentView === item.view ? 'active' : ''}`}
          onClick={() => onNavigate(item.view)}
        >
          <span className="icon">{item.icon}</span>
          <span>{item.label}</span>
        </Button>
      ))}
    </nav>
  );
}
