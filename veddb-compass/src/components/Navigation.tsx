
import { useAppStore, useConnectionStore } from '@/store';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import { Badge } from '@/components/ui/badge';
import { ThemeSwitcher } from '@/components/ui/theme-switcher';
import {
  Database,
  Search,
  BarChart3,
  Users,
  Upload,
  Key,
  Plug,
  Table,
} from 'lucide-react';

export function Navigation() {
  const { currentView, setCurrentView } = useAppStore();
  const { activeConnection } = useConnectionStore();

  const navigationItems = [
    {
      id: 'connections' as const,
      label: 'Connections',
      icon: Plug,
      description: 'Manage server connections',
      requiresConnection: false,
    },
    {
      id: 'explorer' as const,
      label: 'Explorer',
      icon: Table,
      description: 'Browse collections and data',
      requiresConnection: true,
    },
    {
      id: 'query' as const,
      label: 'Query Builder',
      icon: Search,
      description: 'Build and execute queries',
      requiresConnection: true,
    },
    {
      id: 'aggregation' as const,
      label: 'Aggregation',
      icon: Database,
      description: 'Build aggregation pipelines',
      requiresConnection: true,
    },
    {
      id: 'metrics' as const,
      label: 'Metrics',
      icon: BarChart3,
      description: 'Monitor performance',
      requiresConnection: true,
    },
    {
      id: 'users' as const,
      label: 'Users',
      icon: Users,
      description: 'Manage users and roles',
      requiresConnection: true,
    },
    {
      id: 'indexes' as const,
      label: 'Indexes',
      icon: Key,
      description: 'Manage database indexes',
      requiresConnection: true,
    },
    {
      id: 'import-export' as const,
      label: 'Import/Export',
      icon: Upload,
      description: 'Import and export data',
      requiresConnection: true,
    },
  ];

  return (
    <div className="w-64 bg-muted/30 border-r flex flex-col">
      {/* Header */}
      <div className="p-4 border-b">
        <div className="flex items-center gap-2">
          <Database className="h-6 w-6 text-primary" />
          <h2 className="font-semibold text-lg">VedDB Compass</h2>
        </div>
        {activeConnection && (
          <div className="mt-2">
            <div className="text-sm font-medium">{activeConnection.name}</div>
            <div className="text-xs text-muted-foreground">
              {activeConnection.host}:{activeConnection.port}
            </div>
            <Badge variant="secondary" className="mt-1 text-xs">
              Connected
            </Badge>
          </div>
        )}
      </div>

      {/* Navigation Items */}
      <div className="flex-1 p-2">
        <nav className="space-y-1">
          {navigationItems.map((item) => {
            const Icon = item.icon;
            const isActive = currentView === item.id;
            const isDisabled = item.requiresConnection && !activeConnection?.isConnected;

            return (
              <Button
                key={item.id}
                variant={isActive ? 'secondary' : 'ghost'}
                className={`w-full justify-start h-auto p-3 ${
                  isDisabled ? 'opacity-50 cursor-not-allowed' : ''
                }`}
                onClick={() => !isDisabled && setCurrentView(item.id)}
                disabled={isDisabled}
              >
                <div className="flex items-start gap-3 w-full">
                  <Icon className="h-5 w-5 mt-0.5 flex-shrink-0" />
                  <div className="text-left">
                    <div className="font-medium">{item.label}</div>
                    <div className="text-xs text-muted-foreground">
                      {item.description}
                    </div>
                  </div>
                </div>
              </Button>
            );
          })}
        </nav>

        {!activeConnection?.isConnected && (
          <>
            <Separator className="my-4" />
            <div className="p-3 bg-muted/50 rounded-lg">
              <div className="text-sm font-medium mb-1">No Connection</div>
              <div className="text-xs text-muted-foreground">
                Connect to a VedDB server to access database features.
              </div>
            </div>
          </>
        )}
      </div>

      {/* Footer */}
      <div className="p-4 border-t space-y-3">
        <div>
          <div className="text-xs font-medium mb-2">Theme</div>
          <ThemeSwitcher />
        </div>
        <div className="text-xs text-muted-foreground">
          VedDB Compass v0.2.0
        </div>
      </div>
    </div>
  );
}