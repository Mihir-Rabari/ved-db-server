import { useEffect } from "react";
import { useThemeStore, useAppStore } from "@/store";
import { ConnectionManager } from "@/components/ConnectionManager";
import { DatabaseExplorer } from "@/components/DatabaseExplorer";
import { QueryBuilder } from "@/components/QueryBuilder";
import { AggregationBuilder } from "@/components/AggregationBuilder";
import { MetricsDashboard } from "@/components/MetricsDashboard";
import { UserManagement } from "@/components/UserManagement";
import { IndexManager } from "@/components/IndexManager";
import { ImportExport } from "@/components/ImportExport";
import { Navigation } from "@/components/Navigation";
import { Toaster } from "@/components/ui/toaster";

function App() {
  const { theme } = useThemeStore();
  const { currentView } = useAppStore();

  useEffect(() => {
    const root = window.document.documentElement;
    
    const applyTheme = () => {
      if (theme === 'dark') {
        root.classList.add('dark');
      } else if (theme === 'light') {
        root.classList.remove('dark');
      } else {
        // System theme
        const systemTheme = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
        if (systemTheme === 'dark') {
          root.classList.add('dark');
        } else {
          root.classList.remove('dark');
        }
      }
    };

    applyTheme();

    // Listen for system theme changes when using system theme
    if (theme === 'system') {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      const handleChange = () => applyTheme();
      
      mediaQuery.addEventListener('change', handleChange);
      
      return () => {
        mediaQuery.removeEventListener('change', handleChange);
      };
    }
  }, [theme]);

  const renderCurrentView = () => {
    switch (currentView) {
      case 'connections':
        return <ConnectionManager />;
      case 'explorer':
        return <DatabaseExplorer />;
      case 'query':
        return <QueryBuilder />;
      case 'aggregation':
        return <AggregationBuilder />;
      case 'metrics':
        return <MetricsDashboard />;
      case 'users':
        return <UserManagement />;
      case 'indexes':
        return <IndexManager />;
      case 'import-export':
        return <ImportExport />;
      default:
        return <ConnectionManager />;
    }
  };

  return (
    <div className="min-h-screen bg-background text-foreground flex">
      <Navigation />
      <div className="flex-1 flex flex-col">
        {renderCurrentView()}
      </div>
      <Toaster />
    </div>
  );
}

export default App;
