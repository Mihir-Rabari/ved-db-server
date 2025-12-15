import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { useConnectionStore } from '@/store';
import { Loader2, TestTube } from 'lucide-react';
import { useToast } from '@/hooks/use-toast';

interface ConnectionFormProps {
  connectionId?: string | null;
  onClose: () => void;
}

export function ConnectionForm({ connectionId, onClose }: ConnectionFormProps) {
  const { connections, addConnection, updateConnection } = useConnectionStore();
  const [formData, setFormData] = useState({
    name: '',
    host: 'localhost',
    port: 50051,
    username: '',
    password: '',
    tls: false,
  });
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [saving, setSaving] = useState(false);
  const [connectionStatus, setConnectionStatus] = useState<'idle' | 'resolving' | 'connecting' | 'connected' | 'error'>('idle');
  const { toast } = useToast();

  // Load existing connection data if editing
  useEffect(() => {
    if (connectionId) {
      const connection = connections.find(c => c.id === connectionId);
      if (connection) {
        setFormData({
          name: connection.name,
          host: connection.host,
          port: connection.port,
          username: connection.username || '',
          password: connection.password || '',
          tls: connection.tls,
        });
      }
    }
  }, [connectionId, connections]);

  const handleInputChange = (field: string, value: string | number | boolean) => {
    setFormData(prev => ({
      ...prev,
      [field]: value,
    }));
    // Clear test result and error when form changes
    setTestResult(null);
    setConnectionStatus('idle');
  };

  const handleTestConnection = async () => {
    setTesting(true);
    setTestResult(null);
    setConnectionStatus('resolving');

    try {
      const config = {
        id: connectionId || crypto.randomUUID(),
        name: formData.name,
        host: formData.host,
        port: formData.port,
        username: formData.username || undefined,
        password: formData.password || undefined,
        tls: formData.tls,
      };

      setConnectionStatus('connecting');
      const success = await invoke<boolean>('test_connection', { config });
      
      if (success) {
        setConnectionStatus('connected');
        setTestResult({
          success: true,
          message: 'Connection successful!',
        });
        toast({
          title: "Connection Test Successful",
          description: `Successfully connected to ${formData.host}:${formData.port}`,
          variant: "success",
        });
      } else {
        setConnectionStatus('error');
        const errorMsg = 'Connection failed';
        setTestResult({
          success: false,
          message: errorMsg,
        });
      }
    } catch (error) {
      setConnectionStatus('error');
      const errorMsg = String(error);
      
      // Categorize error types for better user feedback
      let errorType = 'Connection Error';
      let errorDescription = errorMsg;
      
      if (errorMsg.includes('Failed to resolve hostname')) {
        errorType = 'DNS Resolution Failed';
        errorDescription = `Could not resolve hostname "${formData.host}". Please check the hostname and try again.`;
      } else if (errorMsg.includes('Connection refused')) {
        errorType = 'Connection Refused';
        errorDescription = `Server at ${formData.host}:${formData.port} refused the connection. Is the server running?`;
      } else if (errorMsg.includes('timeout')) {
        errorType = 'Connection Timeout';
        errorDescription = `Connection to ${formData.host}:${formData.port} timed out. Check your network and server status.`;
      } else if (errorMsg.includes('Authentication failed')) {
        errorType = 'Authentication Failed';
        errorDescription = 'Invalid username or password. Please check your credentials.';
      }
      
      setTestResult({
        success: false,
        message: errorDescription,
      });
      
      toast({
        title: errorType,
        description: errorDescription,
        variant: "destructive",
      });
    } finally {
      setTesting(false);
    }
  };

  const handleSave = async () => {
    setSaving(true);

    try {
      const connectionData = {
        name: formData.name,
        host: formData.host,
        port: formData.port,
        username: formData.username || undefined,
        password: formData.password || undefined,
        tls: formData.tls,
      };

      if (connectionId) {
        // Update existing connection
        updateConnection(connectionId, connectionData);
        toast({
          title: "Connection Updated",
          description: `Connection "${formData.name}" has been updated successfully.`,
          variant: "success",
        });
      } else {
        // Add new connection
        addConnection(connectionData);
        toast({
          title: "Connection Created",
          description: `Connection "${formData.name}" has been created successfully.`,
          variant: "success",
        });
      }

      onClose();
    } catch (error) {
      console.error('Failed to save connection:', error);
      toast({
        title: "Save Failed",
        description: `Failed to save connection: ${error}`,
        variant: "destructive",
      });
    } finally {
      setSaving(false);
    }
  };

  const isFormValid = formData.name.trim() && formData.host.trim() && formData.port > 0;

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="name">Connection Name</Label>
        <Input
          id="name"
          placeholder="My VedDB Server"
          value={formData.name}
          onChange={(e) => handleInputChange('name', e.target.value)}
        />
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div className="space-y-2">
          <Label htmlFor="host">Host</Label>
          <Input
            id="host"
            placeholder="localhost"
            value={formData.host}
            onChange={(e) => handleInputChange('host', e.target.value)}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="port">Port</Label>
          <Input
            id="port"
            type="number"
            placeholder="50051"
            value={formData.port}
            onChange={(e) => handleInputChange('port', parseInt(e.target.value) || 0)}
          />
        </div>
      </div>

      <div className="space-y-2">
        <Label htmlFor="username">Username (optional)</Label>
        <Input
          id="username"
          placeholder="admin"
          value={formData.username}
          onChange={(e) => handleInputChange('username', e.target.value)}
        />
      </div>

      <div className="space-y-2">
        <Label htmlFor="password">Password (optional)</Label>
        <Input
          id="password"
          type="password"
          placeholder="••••••••"
          value={formData.password}
          onChange={(e) => handleInputChange('password', e.target.value)}
        />
      </div>

      <div className="flex items-center space-x-2">
        <Switch
          id="tls"
          checked={formData.tls}
          onCheckedChange={(checked) => handleInputChange('tls', checked)}
        />
        <Label htmlFor="tls">Enable TLS encryption</Label>
      </div>

      {connectionStatus !== 'idle' && (
        <div className="space-y-2">
          {/* Connection status indicator */}
          {testing && (
            <div className="flex items-center space-x-2 text-sm text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" />
              <span>
                {connectionStatus === 'resolving' && 'Resolving hostname...'}
                {connectionStatus === 'connecting' && 'Connecting to server...'}
              </span>
            </div>
          )}
          
          {/* Latest error message display */}
          {testResult && (
            <div
              className={`p-3 rounded-md text-sm ${
                testResult.success
                  ? 'bg-green-50 text-green-800 border border-green-200 dark:bg-green-950 dark:text-green-200 dark:border-green-800'
                  : 'bg-red-50 text-red-800 border border-red-200 dark:bg-red-950 dark:text-red-200 dark:border-red-800'
              }`}
            >
              {testResult.message}
            </div>
          )}
        </div>
      )}

      <div className="flex justify-between pt-4">
        <Button
          variant="outline"
          onClick={handleTestConnection}
          disabled={!isFormValid || testing}
        >
          {testing ? (
            <Loader2 className="h-4 w-4 mr-2 animate-spin" />
          ) : (
            <TestTube className="h-4 w-4 mr-2" />
          )}
          Test Connection
        </Button>

        <div className="space-x-2">
          <Button variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button
            onClick={handleSave}
            disabled={!isFormValid || saving}
          >
            {saving ? (
              <Loader2 className="h-4 w-4 mr-2 animate-spin" />
            ) : null}
            {connectionId ? 'Update' : 'Save'}
          </Button>
        </div>
      </div>
    </div>
  );
}