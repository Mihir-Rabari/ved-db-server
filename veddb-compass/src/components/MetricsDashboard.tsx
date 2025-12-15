import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { useConnectionStore } from '@/store';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  LineChart,
  Line,
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';
import {
  Activity,
  Clock,
  Database,
  Zap,
  Users,
  Timer,
  TrendingUp,
  RefreshCw,
} from 'lucide-react';
import { useToast } from '@/hooks/use-toast';

interface ServerMetrics {
  ops_per_second: number;
  latency_p99: number;
  memory_usage_bytes: number;
  cache_hit_rate: number;
  connection_count: number;
  uptime_seconds: number;
}

interface MetricsHistory {
  timestamp: string;
  ops_per_second: number;
  latency_p99: number;
  memory_usage_mb: number;
  cache_hit_rate: number;
  connection_count: number;
}

export function MetricsDashboard() {
  const { activeConnection } = useConnectionStore();
  const { toast } = useToast();
  const [metrics, setMetrics] = useState<ServerMetrics | null>(null);
  const [metricsHistory, setMetricsHistory] = useState<MetricsHistory[]>([]);
  const [loading, setLoading] = useState(false);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null);

  const fetchMetrics = async () => {
    if (!activeConnection?.isConnected) {
      return;
    }

    setLoading(true);
    try {
      const serverMetrics = await invoke<ServerMetrics>('get_server_metrics', {
        connectionId: activeConnection.id,
      });

      setMetrics(serverMetrics);
      setLastUpdated(new Date());

      // Add to history (keep last 60 data points for 1-minute history)
      const historyEntry: MetricsHistory = {
        timestamp: new Date().toLocaleTimeString(),
        ops_per_second: serverMetrics.ops_per_second,
        latency_p99: serverMetrics.latency_p99,
        memory_usage_mb: Math.round(serverMetrics.memory_usage_bytes / (1024 * 1024)),
        cache_hit_rate: serverMetrics.cache_hit_rate * 100,
        connection_count: serverMetrics.connection_count,
      };

      setMetricsHistory(prev => {
        const newHistory = [...prev, historyEntry];
        return newHistory.slice(-60); // Keep last 60 entries
      });
    } catch (error) {
      console.error('Failed to fetch metrics:', error);
      toast({
        title: 'Error',
        description: 'Failed to fetch server metrics',
        variant: 'destructive',
      });
    } finally {
      setLoading(false);
    }
  };

  // Auto-refresh effect
  useEffect(() => {
    if (!autoRefresh || !activeConnection?.isConnected) {
      return;
    }

    // Initial fetch
    fetchMetrics();

    // Set up interval for auto-refresh (1 second)
    const interval = setInterval(fetchMetrics, 1000);

    return () => clearInterval(interval);
  }, [autoRefresh, activeConnection?.isConnected]);

  // Manual refresh
  const handleRefresh = () => {
    fetchMetrics();
  };

  // Format bytes to human readable
  const formatBytes = (bytes: number): string => {
    const sizes = ['B', 'KB', 'MB', 'GB'];
    if (bytes === 0) return '0 B';
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return `${Math.round(bytes / Math.pow(1024, i) * 100) / 100} ${sizes[i]}`;
  };

  // Format uptime to human readable
  const formatUptime = (seconds: number): string => {
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    
    if (days > 0) {
      return `${days}d ${hours}h ${minutes}m`;
    } else if (hours > 0) {
      return `${hours}h ${minutes}m`;
    } else {
      return `${minutes}m`;
    }
  };

  if (!activeConnection?.isConnected) {
    return (
      <div className="p-6">
        <div className="text-center py-12">
          <Database className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h3 className="text-lg font-medium mb-2">No Connection</h3>
          <p className="text-muted-foreground">
            Connect to a VedDB server to view metrics dashboard.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Metrics Dashboard</h1>
          <p className="text-muted-foreground">
            Real-time performance monitoring for {activeConnection.name}
          </p>
        </div>
        <div className="flex items-center gap-2">
          {lastUpdated && (
            <span className="text-sm text-muted-foreground">
              Last updated: {lastUpdated.toLocaleTimeString()}
            </span>
          )}
          <Button
            variant="outline"
            size="sm"
            onClick={handleRefresh}
            disabled={loading}
          >
            <RefreshCw className={`h-4 w-4 mr-2 ${loading ? 'animate-spin' : ''}`} />
            Refresh
          </Button>
          <Button
            variant={autoRefresh ? 'default' : 'outline'}
            size="sm"
            onClick={() => setAutoRefresh(!autoRefresh)}
          >
            <Activity className="h-4 w-4 mr-2" />
            Auto-refresh
          </Button>
        </div>
      </div>

      {metrics && (
        <>
          {/* Key Metrics Cards */}
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Operations/sec</CardTitle>
                <Zap className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {metrics.ops_per_second.toLocaleString(undefined, { maximumFractionDigits: 1 })}
                </div>
                <p className="text-xs text-muted-foreground">
                  Current throughput
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Latency P99</CardTitle>
                <Clock className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {metrics.latency_p99.toFixed(2)}ms
                </div>
                <p className="text-xs text-muted-foreground">
                            </p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Memory Usage</CardTitle>
                <Database className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {formatBytes(metrics.memory_usage_bytes)}
                </div>
                <p className="text-xs text-muted-foreground">
                  Current usage
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Cache Hit Rate</CardTitle>
                <TrendingUp className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {(metrics.cache_hit_rate * 100).toFixed(1)}%
                </div>
                <p className="text-xs text-muted-foreground">
                  Cache efficiency
                </p>
              </CardContent>
            </Card>
          </div>

          {/* Additional Info Cards */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Active Connections</CardTitle>
                <Users className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{metrics.connection_count}</div>
                <p className="text-xs text-muted-foreground">
                  Currently connected clients
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Server Uptime</CardTitle>
                <Timer className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{formatUptime(metrics.uptime_seconds)}</div>
                <p className="text-xs text-muted-foreground">
                  Time since last restart
                </p>
              </CardContent>
            </Card>
          </div>

          {/* Historical Charts */}
          {metricsHistory.length > 1 && (
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
              {/* Operations per Second Chart */}
              <Card>
                <CardHeader>
                  <CardTitle>Operations per Second</CardTitle>
                  <CardDescription>
                    Real-time throughput over the last minute
                  </CardDescription>
                </CardHeader>
                <CardContent>
                  <ResponsiveContainer width="100%" height={200}>
                    <AreaChart data={metricsHistory}>
                      <CartesianGrid strokeDasharray="3 3" />
                      <XAxis 
                        dataKey="timestamp" 
                        tick={{ fontSize: 12 }}
                        interval="preserveStartEnd"
                      />
                      <YAxis tick={{ fontSize: 12 }} />
                      <Tooltip />
                      <Area
                        type="monotone"
                        dataKey="ops_per_second"
                        stroke="#8884d8"
                        fill="#8884d8"
                        fillOpacity={0.3}
                      />
                    </AreaChart>
                  </ResponsiveContainer>
                </CardContent>
              </Card>

              {/* Latency Chart */}
              <Card>
                <CardHeader>
                  <CardTitle>Latency P99</CardTitle>
                  <CardDescription>
                    99th percentile latency in milliseconds
                  </CardDescription>
                </CardHeader>
                <CardContent>
                  <ResponsiveContainer width="100%" height={200}>
                    <LineChart data={metricsHistory}>
                      <CartesianGrid strokeDasharray="3 3" />
                      <XAxis 
                        dataKey="timestamp" 
                        tick={{ fontSize: 12 }}
                        interval="preserveStartEnd"
                      />
                      <YAxis tick={{ fontSize: 12 }} />
                      <Tooltip />
                      <Line
                        type="monotone"
                        dataKey="latency_p99"
                        stroke="#82ca9d"
                        strokeWidth={2}
                        dot={false}
                      />
                    </LineChart>
                  </ResponsiveContainer>
                </CardContent>
              </Card>

              {/* Memory Usage Chart */}
              <Card>
                <CardHeader>
                  <CardTitle>Memory Usage</CardTitle>
                  <CardDescription>
                    Memory consumption in MB
                  </CardDescription>
                </CardHeader>
                <CardContent>
                  <ResponsiveContainer width="100%" height={200}>
                    <AreaChart data={metricsHistory}>
                      <CartesianGrid strokeDasharray="3 3" />
                      <XAxis 
                        dataKey="timestamp" 
                        tick={{ fontSize: 12 }}
                        interval="preserveStartEnd"
                      />
                      <YAxis tick={{ fontSize: 12 }} />
                      <Tooltip />
                      <Area
                        type="monotone"
                        dataKey="memory_usage_mb"
                        stroke="#ffc658"
                        fill="#ffc658"
                        fillOpacity={0.3}
                      />
                    </AreaChart>
                  </ResponsiveContainer>
                </CardContent>
              </Card>

              {/* Cache Hit Rate Chart */}
              <Card>
                <CardHeader>
                  <CardTitle>Cache Hit Rate</CardTitle>
                  <CardDescription>
                    Cache efficiency percentage
                  </CardDescription>
                </CardHeader>
                <CardContent>
                  <ResponsiveContainer width="100%" height={200}>
                    <LineChart data={metricsHistory}>
                      <CartesianGrid strokeDasharray="3 3" />
                      <XAxis 
                        dataKey="timestamp" 
                        tick={{ fontSize: 12 }}
                        interval="preserveStartEnd"
                      />
                      <YAxis 
                        tick={{ fontSize: 12 }}
                        domain={[0, 100]}
                      />
                      <Tooltip />
                      <Line
                        type="monotone"
                        dataKey="cache_hit_rate"
                        stroke="#ff7300"
                        strokeWidth={2}
                        dot={false}
                      />
                    </LineChart>
                  </ResponsiveContainer>
                </CardContent>
              </Card>
            </div>
          )}

          {/* Status Indicators */}
          <Card>
            <CardHeader>
              <CardTitle>System Status</CardTitle>
              <CardDescription>
                Current system health indicators
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div className="flex items-center justify-between p-3 border rounded-lg">
                  <span className="text-sm font-medium">Performance</span>
                  <Badge variant={metrics.ops_per_second > 1000 ? 'default' : 'secondary'}>
                    {metrics.ops_per_second > 1000 ? 'Excellent' : 'Good'}
                  </Badge>
                </div>
                <div className="flex items-center justify-between p-3 border rounded-lg">
                  <span className="text-sm font-medium">Latency</span>
                  <Badge variant={metrics.latency_p99 < 5 ? 'default' : 'secondary'}>
                    {metrics.latency_p99 < 5 ? 'Low' : 'Normal'}
                  </Badge>
                </div>
                <div className="flex items-center justify-between p-3 border rounded-lg">
                  <span className="text-sm font-medium">Cache</span>
                  <Badge variant={metrics.cache_hit_rate > 0.8 ? 'default' : 'secondary'}>
                    {metrics.cache_hit_rate > 0.8 ? 'Efficient' : 'Moderate'}
                  </Badge>
                </div>
              </div>
            </CardContent>
          </Card>
        </>
      )}

      {!metrics && !loading && (
        <div className="text-center py-12">
          <Activity className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h3 className="text-lg font-medium mb-2">No Metrics Available</h3>
          <p className="text-muted-foreground mb-4">
            Click refresh to load server metrics.
          </p>
          <Button onClick={handleRefresh}>
            <RefreshCw className="h-4 w-4 mr-2" />
            Load Metrics
          </Button>
        </div>
      )}
    </div>
  );
}