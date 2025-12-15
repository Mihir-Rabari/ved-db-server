import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { Plus, Database, Trash2, Edit, Play, Square, Download, Upload } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { useConnectionStore } from '@/store';
import { ConnectionForm } from './ConnectionForm';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog';
import { exportConnection, importConnection } from '@/lib/connection-files';
import { useToast } from '@/hooks/use-toast';

interface ConnectionStatus {
  id: string;
  connected: boolean;
  last_error?: string;
}

export function ConnectionManager() {
  const { connections, activeConnection, setActiveConnection, removeConnection, addConnection } = useConnectionStore();
  const [connectionStatuses, setConnectionStatuses] = useState<Record<string, ConnectionStatus>>({});
  const [showForm, setShowForm] = useState(false);
  const [editingConnection, setEditingConnection] = useState<string | null>(null);
  const { toast } = useToast();

  // Load connection statuses
  useEffect(() => {
    const loadStatuses = async () => {
      const statuses: Record<string, ConnectionStatus> = {};
      for (const conn of connections) {
        try {
          const status = await invoke<ConnectionStatus>('get_connection_status', {
            connectionId: conn.id,
          });
          statuses[conn.id] = status;
        } catch (error) {
          statuses[conn.id] = {
            id: conn.id,
            connected: false,
            last_error: 'Status unknown',
          };
        }
      }
      setConnectionStatuses(statuses);
    };

    if (connections.length > 0) {
      loadStatuses();
    }
  }, [connections]);

  const handleConnect = async (connectionId: string) => {
    const connection = connections.find(c => c.id === connectionId);
    if (!connection) return;

    try {
      const status = await invoke<ConnectionStatus>('connect_to_server', {
        config: connection,
      });
      
      setConnectionStatuses(prev => ({
        ...prev,
        [connectionId]: status,
      }));

      if (status.connected) {
        setActiveConnection({ ...connection, isConnected: true });
        toast({
          title: "Connected",
          description: `Successfully connected to "${connection.name}".`,
          variant: "success",
        });
      }
    } catch (error) {
      console.error('Failed to connect:', error);
      const errorMsg = String(error);
      
      // Update connection status with latest error
      setConnectionStatuses(prev => ({
        ...prev,
        [connectionId]: {
          id: connectionId,
          connected: false,
          last_error: errorMsg,
        },
      }));
      
      // Categorize error types for better user feedback
      let errorType = 'Connection Failed';
      let errorDescription = errorMsg;
      
      if (errorMsg.includes('Failed to resolve hostname')) {
        errorType = 'DNS Resolution Failed';
        errorDescription = `Could not resolve hostname "${connection.host}". Please check the hostname and try again.`;
      } else if (errorMsg.includes('Connection refused')) {
        errorType = 'Connection Refused';
        errorDescription = `Server at ${connection.host}:${connection.port} refused the connection. Is the server running?`;
      } else if (errorMsg.includes('timeout')) {
        errorType = 'Connection Timeout';
        errorDescription = `Connection to ${connection.host}:${connection.port} timed out. Check your network and server status.`;
      } else if (errorMsg.includes('Authentication failed')) {
        errorType = 'Authentication Failed';
        errorDescription = 'Invalid username or password. Please check your credentials.';
      }
      
      toast({
        title: errorType,
        description: errorDescription,
        variant: "destructive",
      });
    }
  };

  const handleDisconnect = async (connectionId: string) => {
    try {
      await invoke('disconnect_from_server', { connectionId });
      
      setConnectionStatuses(prev => ({
        ...prev,
        [connectionId]: {
          ...prev[connectionId],
          connected: false,
        },
      }));

      if (activeConnection?.id === connectionId) {
        setActiveConnection(null);
      }
      
      const connection = connections.find(c => c.id === connectionId);
      if (connection) {
        toast({
          title: "Disconnected",
          description: `Disconnected from "${connection.name}".`,
          variant: "default",
        });
      }
    } catch (error) {
      console.error('Failed to disconnect:', error);
      toast({
        title: "Disconnect Failed",
        description: `Failed to disconnect: ${error}`,
        variant: "destructive",
      });
    }
  };

  const handleDelete = (connectionId: string) => {
    const connection = connections.find(c => c.id === connectionId);
    if (!connection) return;

    if (confirm(`Are you sure you want to delete the connection "${connection.name}"?`)) {
      if (activeConnection?.id === connectionId) {
        setActiveConnection(null);
      }
      removeConnection(connectionId);
      setConnectionStatuses(prev => {
        const newStatuses = { ...prev };
        delete newStatuses[connectionId];
        return newStatuses;
      });
      toast({
        title: "Connection Deleted",
        description: `Connection "${connection.name}" has been deleted.`,
        variant: "default",
      });
    }
  };

  const handleEdit = (connectionId: string) => {
    setEditingConnection(connectionId);
    setShowForm(true);
  };

  const handleFormClose = () => {
    setShowForm(false);
    setEditingConnection(null);
  };

  const handleExport = async (connection: any) => {
    try {
      await exportConnection(connection);
      toast({
        title: "Export Successful",
        description: `Connection "${connection.name}" has been exported successfully.`,
        variant: "success",
      });
    } catch (error) {
      console.error('Export failed:', error);
      toast({
        title: "Export Failed",
        description: `Failed to export connection: ${error}`,
        variant: "destructive",
      });
    }
  };

  const handleImport = async () => {
    try {
      const connectionData = await importConnection();
      if (connectionData) {
        addConnection(connectionData);
        toast({
          title: "Import Successful",
          description: `Connection "${connectionData.name}" has been imported successfully.`,
          variant: "success",
        });
      }
    } catch (error) {
      console.error('Import failed:', error);
      toast({
        title: "Import Failed",
        description: `Failed to import connection: ${error}`,
        variant: "destructive",
      });
    }
  };

  return (
    <div className="flex-1 p-6 space-y-6 overflow-auto">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold">Connections</h1>
          <p className="text-muted-foreground">
            Manage your VedDB server connections
          </p>
        </div>
        <div className="flex space-x-2">
          <Button variant="outline" onClick={handleImport}>
            <Upload className="h-4 w-4 mr-2" />
            Import
          </Button>
          <Dialog open={showForm} onOpenChange={setShowForm}>
            <DialogTrigger asChild>
              <Button onClick={() => setShowForm(true)}>
                <Plus className="h-4 w-4 mr-2" />
                New Connection
              </Button>
            </DialogTrigger>
            <DialogContent className="max-w-md">
              <DialogHeader>
                <DialogTitle>
                  {editingConnection ? 'Edit Connection' : 'New Connection'}
                </DialogTitle>
              </DialogHeader>
              <ConnectionForm
                connectionId={editingConnection}
                onClose={handleFormClose}
              />
            </DialogContent>
          </Dialog>
        </div>
      </div>

      {connections.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Database className="h-12 w-12 text-muted-foreground mb-4" />
            <h3 className="text-lg font-semibold mb-2">No connections</h3>
            <p className="text-muted-foreground text-center mb-4">
              Create your first connection to get started with VedDB Compass
            </p>
            <Button onClick={() => setShowForm(true)}>
              <Plus className="h-4 w-4 mr-2" />
              Add Connection
            </Button>
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {connections.map((connection) => {
            const status = connectionStatuses[connection.id];
            const isConnected = status?.connected || false;
            const isActive = activeConnection?.id === connection.id;

            return (
              <Card key={connection.id} className={isActive ? 'ring-2 ring-primary' : ''}>
                <CardHeader className="pb-3">
                  <div className="flex items-center justify-between">
                    <CardTitle className="text-lg">{connection.name}</CardTitle>
                    <div className="flex items-center space-x-1">
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => handleExport(connection)}
                        title="Export connection"
                      >
                        <Download className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => handleEdit(connection.id)}
                        title="Edit connection"
                      >
                        <Edit className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => handleDelete(connection.id)}
                        title="Delete connection"
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>
                  </div>
                  <CardDescription>
                    {connection.host}:{connection.port}
                    {connection.tls && ' (TLS)'}
                  </CardDescription>
                </CardHeader>
                <CardContent className="space-y-3">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center space-x-2">
                      <div
                        className={`w-2 h-2 rounded-full ${
                          isConnected ? 'bg-green-500' : 'bg-gray-400'
                        }`}
                      />
                      <span className="text-sm">
                        {isConnected ? 'Connected' : 'Disconnected'}
                      </span>
                    </div>
                    {isActive && (
                      <span className="text-xs bg-primary text-primary-foreground px-2 py-1 rounded">
                        Active
                      </span>
                    )}
                  </div>

                  {status?.last_error && (
                    <p className="text-sm text-destructive">{status.last_error}</p>
                  )}

                  <div className="flex space-x-2">
                    {isConnected ? (
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => handleDisconnect(connection.id)}
                        className="flex-1"
                      >
                        <Square className="h-4 w-4 mr-2" />
                        Disconnect
                      </Button>
                    ) : (
                      <Button
                        size="sm"
                        onClick={() => handleConnect(connection.id)}
                        className="flex-1"
                      >
                        <Play className="h-4 w-4 mr-2" />
                        Connect
                      </Button>
                    )}
                  </div>
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}
    </div>
  );
}