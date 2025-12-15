import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { useConnectionStore } from '@/store';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { ScrollArea } from '@/components/ui/scroll-area';
import { useToast } from '@/hooks/use-toast';
import {
  ChevronRight,
  ChevronDown,
  Database,
  Table,
  Key,
  BarChart3,
  RefreshCw,
  Settings,
  FileText,
  Hash,
  Clock,
  HardDrive,
  Zap,
  Activity,
  Layers,
  Search,
  TrendingUp,
  Users,
} from 'lucide-react';

interface CollectionInfo {
  name: string;
  document_count: number;
  size_bytes: number;
  indexes: IndexInfo[];
}

interface IndexInfo {
  name: string;
  fields: string[];
  unique: boolean;
  size_bytes: number;
}

interface SchemaField {
  name: string;
  type: string;
  required: boolean;
  indexed: boolean;
  cached: boolean;
  cache_strategy?: 'write-through' | 'write-behind' | 'read-through' | 'none';
  ttl?: number;
}

interface CacheStatistics {
  hit_rate: number;
  miss_rate: number;
  eviction_count: number;
  memory_usage_bytes: number;
  key_count: number;
}

interface CollectionStatistics {
  avg_document_size: number;
  index_usage: Record<string, number>;
  query_performance: {
    avg_query_time_ms: number;
    slow_queries_count: number;
  };
  cache_stats?: CacheStatistics;
}

export function DatabaseExplorer() {
  const { activeConnection } = useConnectionStore();
  const { toast } = useToast();
  const [collections, setCollections] = useState<CollectionInfo[]>([]);
  const [selectedCollection, setSelectedCollection] = useState<string | null>(null);
  const [expandedCollections, setExpandedCollections] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [searchTerm, setSearchTerm] = useState('');

  // Enhanced mock data demonstrating VedDB v0.2.0 hybrid storage architecture
  // In production, this would come from the server's schema and cache configuration APIs
  const mockSchemas: Record<string, SchemaField[]> = {
    users: [
      { name: '_id', type: 'ObjectId', required: true, indexed: true, cached: false, cache_strategy: 'none' },
      { name: 'email', type: 'String', required: true, indexed: true, cached: true, cache_strategy: 'write-through', ttl: 3600 },
      { name: 'username', type: 'String', required: true, indexed: false, cached: true, cache_strategy: 'write-through', ttl: 7200 },
      { name: 'password_hash', type: 'String', required: true, indexed: false, cached: false, cache_strategy: 'none' },
      { name: 'profile', type: 'Object', required: false, indexed: false, cached: true, cache_strategy: 'read-through', ttl: 1800 },
      { name: 'profile.name', type: 'String', required: false, indexed: false, cached: true, cache_strategy: 'read-through', ttl: 1800 },
      { name: 'profile.avatar_url', type: 'String', required: false, indexed: false, cached: false, cache_strategy: 'none' },
      { name: 'created_at', type: 'DateTime', required: true, indexed: true, cached: false, cache_strategy: 'none' },
      { name: 'last_login', type: 'DateTime', required: false, indexed: true, cached: true, cache_strategy: 'write-behind', ttl: 900 },
      { name: 'preferences', type: 'Object', required: false, indexed: false, cached: true, cache_strategy: 'write-behind', ttl: 3600 },
    ],
    products: [
      { name: '_id', type: 'ObjectId', required: true, indexed: true, cached: false, cache_strategy: 'none' },
      { name: 'name', type: 'String', required: true, indexed: true, cached: true, cache_strategy: 'write-through', ttl: 7200 },
      { name: 'description', type: 'String', required: false, indexed: false, cached: false, cache_strategy: 'none' },
      { name: 'price', type: 'Number', required: true, indexed: true, cached: true, cache_strategy: 'write-through', ttl: 1800 },
      { name: 'category', type: 'String', required: true, indexed: true, cached: true, cache_strategy: 'read-through', ttl: 14400 },
      { name: 'tags', type: 'Array', required: false, indexed: true, cached: true, cache_strategy: 'read-through', ttl: 7200 },
      { name: 'inventory', type: 'Object', required: true, indexed: false, cached: true, cache_strategy: 'write-behind', ttl: 300 },
      { name: 'inventory.quantity', type: 'Number', required: true, indexed: true, cached: true, cache_strategy: 'write-behind', ttl: 300 },
      { name: 'inventory.warehouse', type: 'String', required: true, indexed: true, cached: false, cache_strategy: 'none' },
      { name: 'created_at', type: 'DateTime', required: true, indexed: true, cached: false, cache_strategy: 'none' },
      { name: 'updated_at', type: 'DateTime', required: true, indexed: false, cached: false, cache_strategy: 'none' },
    ],
  };

  // Mock collection statistics demonstrating hybrid storage metrics
  const mockCollectionStats: Record<string, CollectionStatistics> = {
    users: {
      avg_document_size: 2048,
      index_usage: {
        '_id': 1250,
        'email_idx': 890,
        'last_login_idx': 234,
      },
      query_performance: {
        avg_query_time_ms: 1.2,
        slow_queries_count: 3,
      },
      cache_stats: {
        hit_rate: 0.87,
        miss_rate: 0.13,
        eviction_count: 45,
        memory_usage_bytes: 524288, // 512 KB
        key_count: 1089,
      },
    },
    products: {
      avg_document_size: 4096,
      index_usage: {
        '_id': 5000,
        'name_idx': 3200,
        'category_idx': 1800,
        'price_idx': 950,
      },
      query_performance: {
        avg_query_time_ms: 2.8,
        slow_queries_count: 12,
      },
      cache_stats: {
        hit_rate: 0.92,
        miss_rate: 0.08,
        eviction_count: 128,
        memory_usage_bytes: 2097152, // 2 MB
        key_count: 4567,
      },
    },
  };

  const loadCollections = async () => {
    if (!activeConnection?.isConnected) {
      toast({
        title: 'No Connection',
        description: 'Please connect to a VedDB server first.',
        variant: 'destructive',
      });
      return;
    }

    setLoading(true);
    try {
      const result = await invoke<CollectionInfo[]>('get_collections', {
        connectionId: activeConnection.id,
      });
      setCollections(result);
    } catch (error) {
      toast({
        title: 'Error Loading Collections',
        description: error as string,
        variant: 'destructive',
      });
    } finally {
      setLoading(false);
    }
  };

  const refreshCollections = async () => {
    setRefreshing(true);
    await loadCollections();
    setRefreshing(false);
    toast({
      title: 'Collections Refreshed',
      description: 'Collection list has been updated.',
    });
  };

  useEffect(() => {
    if (activeConnection?.isConnected) {
      loadCollections();
    }
  }, [activeConnection]);

  const toggleCollection = (collectionName: string) => {
    const newExpanded = new Set(expandedCollections);
    if (newExpanded.has(collectionName)) {
      newExpanded.delete(collectionName);
    } else {
      newExpanded.add(collectionName);
    }
    setExpandedCollections(newExpanded);
  };

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const formatNumber = (num: number): string => {
    return new Intl.NumberFormat().format(num);
  };

  const getTypeIcon = (type: string) => {
    switch (type.toLowerCase()) {
      case 'objectid':
        return <Key className="h-3 w-3 text-yellow-600" />;
      case 'string':
        return <FileText className="h-3 w-3 text-blue-600" />;
      case 'number':
        return <Hash className="h-3 w-3 text-green-600" />;
      case 'datetime':
        return <Clock className="h-3 w-3 text-purple-600" />;
      case 'object':
        return <Settings className="h-3 w-3 text-orange-600" />;
      case 'array':
        return <BarChart3 className="h-3 w-3 text-red-600" />;
      default:
        return <FileText className="h-3 w-3 text-gray-600" />;
    }
  };

  const getCacheStrategyColor = (strategy?: string) => {
    switch (strategy) {
      case 'write-through':
        return 'bg-blue-50 text-blue-700 border-blue-200';
      case 'write-behind':
        return 'bg-green-50 text-green-700 border-green-200';
      case 'read-through':
        return 'bg-purple-50 text-purple-700 border-purple-200';
      case 'none':
        return 'bg-gray-50 text-gray-700 border-gray-200';
      default:
        return 'bg-gray-50 text-gray-700 border-gray-200';
    }
  };

  if (!activeConnection?.isConnected) {
    return (
      <div className="flex items-center justify-center h-full flex-1">
        <Card className="w-96">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Database className="h-5 w-5" />
              Database Explorer
            </CardTitle>
            <CardDescription>
              Connect to a VedDB server to explore your databases and collections.
            </CardDescription>
          </CardHeader>
        </Card>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-1">
      {/* Left Panel - Collection Tree */}
      <div className="w-80 border-r bg-muted/30">
        <div className="p-4 border-b space-y-3">
          <div className="flex items-center justify-between">
            <h2 className="text-lg font-semibold flex items-center gap-2">
              <Database className="h-5 w-5" />
              Collections
            </h2>
            <Button
              variant="ghost"
              size="sm"
              onClick={refreshCollections}
              disabled={refreshing}
            >
              <RefreshCw className={`h-4 w-4 ${refreshing ? 'animate-spin' : ''}`} />
            </Button>
          </div>
          
          {/* Search Collections */}
          <div className="relative">
            <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-muted-foreground" />
            <input
              type="text"
              placeholder="Search collections..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="w-full pl-10 pr-4 py-2 text-sm border rounded-md bg-background focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            />
          </div>
        </div>

        <ScrollArea className="flex-1">
          {loading ? (
            <div className="p-4 text-center text-muted-foreground">
              Loading collections...
            </div>
          ) : collections.length === 0 ? (
            <div className="p-4 text-center text-muted-foreground">
              No collections found
            </div>
          ) : (
            <div className="p-2">
              {collections
                .filter(collection => 
                  collection.name.toLowerCase().includes(searchTerm.toLowerCase())
                )
                .map((collection) => (
                <div key={collection.name} className="mb-2">
                  <div
                    className={`flex items-center gap-2 p-3 rounded-lg cursor-pointer hover:bg-muted/50 transition-colors ${
                      selectedCollection === collection.name ? 'bg-muted border-l-4 border-l-blue-500' : 'border border-transparent'
                    }`}
                    onClick={() => {
                      setSelectedCollection(collection.name);
                      toggleCollection(collection.name);
                    }}
                  >
                    {expandedCollections.has(collection.name) ? (
                      <ChevronDown className="h-4 w-4 text-muted-foreground" />
                    ) : (
                      <ChevronRight className="h-4 w-4 text-muted-foreground" />
                    )}
                    <div className="flex items-center gap-2 flex-1">
                      <Table className="h-4 w-4 text-blue-600" />
                      <div className="flex-1">
                        <div className="font-medium">{collection.name}</div>
                        <div className="text-xs text-muted-foreground">
                          {formatNumber(collection.document_count)} docs • {formatBytes(collection.size_bytes)}
                        </div>
                      </div>
                    </div>
                    <div className="flex items-center gap-1">
                      {mockCollectionStats[collection.name]?.cache_stats && (
                        <Badge variant="outline" className="text-xs bg-emerald-50 text-emerald-700 border-emerald-200">
                          <Zap className="h-3 w-3 mr-1" />
                          {(mockCollectionStats[collection.name].cache_stats!.hit_rate * 100).toFixed(0)}%
                        </Badge>
                      )}
                      <Badge variant="secondary" className="text-xs">
                        {collection.indexes.length} idx
                      </Badge>
                    </div>
                  </div>

                  {expandedCollections.has(collection.name) && (
                    <div className="ml-6 mt-2 space-y-2 pb-2">
                      {/* Storage Layers */}
                      <div className="space-y-1">
                        <div className="flex items-center gap-2 p-2 text-sm bg-blue-50 text-blue-700 rounded">
                          <HardDrive className="h-3 w-3" />
                          <span className="font-medium">Persistent Layer</span>
                          <span className="ml-auto text-xs">{formatBytes(collection.size_bytes)}</span>
                        </div>
                        {mockCollectionStats[collection.name]?.cache_stats && (
                          <div className="flex items-center gap-2 p-2 text-sm bg-emerald-50 text-emerald-700 rounded">
                            <Zap className="h-3 w-3" />
                            <span className="font-medium">Cache Layer</span>
                            <span className="ml-auto text-xs">
                              {formatBytes(mockCollectionStats[collection.name].cache_stats!.memory_usage_bytes)}
                            </span>
                          </div>
                        )}
                      </div>

                      {/* Performance Indicators */}
                      {mockCollectionStats[collection.name] && (
                        <div className="space-y-1">
                          <div className="flex items-center gap-2 p-1 text-xs text-muted-foreground">
                            <Activity className="h-3 w-3" />
                            Avg Query: {mockCollectionStats[collection.name].query_performance.avg_query_time_ms}ms
                          </div>
                          {mockCollectionStats[collection.name].cache_stats && (
                            <div className="flex items-center gap-2 p-1 text-xs text-muted-foreground">
                              <TrendingUp className="h-3 w-3" />
                              Cache Hit Rate: {(mockCollectionStats[collection.name].cache_stats!.hit_rate * 100).toFixed(1)}%
                            </div>
                          )}
                        </div>
                      )}

                      {/* Indexes */}
                      <div className="space-y-1">
                        <div className="text-xs font-medium text-muted-foreground flex items-center gap-1">
                          <Key className="h-3 w-3" />
                          Indexes ({collection.indexes.length})
                        </div>
                        {collection.indexes.map((index) => (
                          <div
                            key={index.name}
                            className="ml-4 flex items-center justify-between p-1 text-xs text-muted-foreground hover:bg-muted/30 rounded"
                          >
                            <div className="flex items-center gap-2">
                              <div className="w-2 h-2 rounded-full bg-purple-500" />
                              <span>{index.name}</span>
                            </div>
                            <div className="flex items-center gap-1">
                              {index.unique && (
                                <Badge variant="outline" className="text-xs">
                                  unique
                                </Badge>
                              )}
                              <span className="text-xs">{formatBytes(index.size_bytes)}</span>
                            </div>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </ScrollArea>
      </div>

      {/* Right Panel - Collection Details */}
      <div className="flex-1 flex flex-col">
        {selectedCollection ? (
          <>
            <div className="p-4 border-b">
              <h2 className="text-xl font-semibold flex items-center gap-2">
                <Table className="h-5 w-5" />
                {selectedCollection}
              </h2>
              <p className="text-muted-foreground">
                Collection details, statistics, and schema information
              </p>
            </div>

            <ScrollArea className="flex-1 p-4">
              <div className="space-y-6">
                {/* Collection Statistics */}
                <Card>
                  <CardHeader>
                    <CardTitle className="flex items-center gap-2">
                      <BarChart3 className="h-5 w-5" />
                      Collection Statistics
                    </CardTitle>
                    <CardDescription>
                      Document storage and performance metrics
                    </CardDescription>
                  </CardHeader>
                  <CardContent>
                    <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
                      {(() => {
                        const collection = collections.find(c => c.name === selectedCollection);
                        const stats = mockCollectionStats[selectedCollection || ''];
                        if (!collection) return null;
                        
                        return (
                          <>
                            <div className="text-center">
                              <div className="text-2xl font-bold text-blue-600">
                                {formatNumber(collection.document_count)}
                              </div>
                              <div className="text-sm text-muted-foreground">Documents</div>
                            </div>
                            <div className="text-center">
                              <div className="text-2xl font-bold text-green-600">
                                {formatBytes(collection.size_bytes)}
                              </div>
                              <div className="text-sm text-muted-foreground">Total Size</div>
                            </div>
                            <div className="text-center">
                              <div className="text-2xl font-bold text-purple-600">
                                {collection.indexes.length}
                              </div>
                              <div className="text-sm text-muted-foreground">Indexes</div>
                            </div>
                            <div className="text-center">
                              <div className="text-2xl font-bold text-orange-600">
                                {stats ? `${stats.query_performance.avg_query_time_ms}ms` : 'N/A'}
                              </div>
                              <div className="text-sm text-muted-foreground">Avg Query Time</div>
                            </div>
                          </>
                        );
                      })()}
                    </div>

                    {/* Performance Metrics */}
                    {(() => {
                      const stats = mockCollectionStats[selectedCollection || ''];
                      if (!stats) return null;

                      return (
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                          <div className="space-y-3">
                            <h4 className="font-medium flex items-center gap-2">
                              <Activity className="h-4 w-4" />
                              Performance
                            </h4>
                            <div className="space-y-2 text-sm">
                              <div className="flex justify-between">
                                <span className="text-muted-foreground">Avg Document Size:</span>
                                <span className="font-medium">{formatBytes(stats.avg_document_size)}</span>
                              </div>
                              <div className="flex justify-between">
                                <span className="text-muted-foreground">Slow Queries:</span>
                                <span className="font-medium">{stats.query_performance.slow_queries_count}</span>
                              </div>
                            </div>
                          </div>
                          
                          <div className="space-y-3">
                            <h4 className="font-medium flex items-center gap-2">
                              <TrendingUp className="h-4 w-4" />
                              Index Usage
                            </h4>
                            <div className="space-y-2 text-sm">
                              {Object.entries(stats.index_usage).slice(0, 3).map(([indexName, usage]) => (
                                <div key={indexName} className="flex justify-between">
                                  <span className="text-muted-foreground">{indexName}:</span>
                                  <span className="font-medium">{formatNumber(usage)} hits</span>
                                </div>
                              ))}
                            </div>
                          </div>
                        </div>
                      );
                    })()}
                  </CardContent>
                </Card>

                {/* Cache Statistics - VedDB v0.2.0 Hybrid Storage */}
                {(() => {
                  const stats = mockCollectionStats[selectedCollection || ''];
                  if (!stats?.cache_stats) return null;

                  return (
                    <Card>
                      <CardHeader>
                        <CardTitle className="flex items-center gap-2">
                          <Zap className="h-5 w-5" />
                          Cache Layer Statistics
                        </CardTitle>
                        <CardDescription>
                          In-memory cache performance for hybrid storage architecture
                        </CardDescription>
                      </CardHeader>
                      <CardContent>
                        <div className="grid grid-cols-2 md:grid-cols-5 gap-4 mb-4">
                          <div className="text-center">
                            <div className="text-2xl font-bold text-emerald-600">
                              {(stats.cache_stats.hit_rate * 100).toFixed(1)}%
                            </div>
                            <div className="text-sm text-muted-foreground">Hit Rate</div>
                          </div>
                          <div className="text-center">
                            <div className="text-2xl font-bold text-red-600">
                              {(stats.cache_stats.miss_rate * 100).toFixed(1)}%
                            </div>
                            <div className="text-sm text-muted-foreground">Miss Rate</div>
                          </div>
                          <div className="text-center">
                            <div className="text-2xl font-bold text-blue-600">
                              {formatNumber(stats.cache_stats.key_count)}
                            </div>
                            <div className="text-sm text-muted-foreground">Cached Keys</div>
                          </div>
                          <div className="text-center">
                            <div className="text-2xl font-bold text-purple-600">
                              {formatBytes(stats.cache_stats.memory_usage_bytes)}
                            </div>
                            <div className="text-sm text-muted-foreground">Memory Usage</div>
                          </div>
                          <div className="text-center">
                            <div className="text-2xl font-bold text-orange-600">
                              {formatNumber(stats.cache_stats.eviction_count)}
                            </div>
                            <div className="text-sm text-muted-foreground">Evictions</div>
                          </div>
                        </div>

                        {/* Cache Performance Indicator */}
                        <div className="mt-4 p-3 bg-muted/50 rounded-lg">
                          <div className="flex items-center justify-between mb-2">
                            <span className="text-sm font-medium">Cache Efficiency</span>
                            <Badge variant={stats.cache_stats.hit_rate > 0.8 ? "default" : stats.cache_stats.hit_rate > 0.6 ? "secondary" : "destructive"}>
                              {stats.cache_stats.hit_rate > 0.8 ? "Excellent" : stats.cache_stats.hit_rate > 0.6 ? "Good" : "Poor"}
                            </Badge>
                          </div>
                          <div className="w-full bg-muted rounded-full h-2">
                            <div 
                              className="bg-emerald-600 h-2 rounded-full transition-all duration-300" 
                              style={{ width: `${stats.cache_stats.hit_rate * 100}%` }}
                            />
                          </div>
                        </div>
                      </CardContent>
                    </Card>
                  );
                })()}

                {/* Indexes */}
                <Card>
                  <CardHeader>
                    <CardTitle className="flex items-center gap-2">
                      <Key className="h-5 w-5" />
                      Indexes
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <div className="space-y-3">
                      {(() => {
                        const collection = collections.find(c => c.name === selectedCollection);
                        if (!collection) return null;
                        
                        return collection.indexes.map((index) => (
                          <div key={index.name} className="flex items-center justify-between p-3 border rounded">
                            <div className="flex items-center gap-3">
                              <Key className="h-4 w-4 text-muted-foreground" />
                              <div>
                                <div className="font-medium">{index.name}</div>
                                <div className="text-sm text-muted-foreground">
                                  Fields: {index.fields.join(', ')}
                                </div>
                              </div>
                            </div>
                            <div className="flex items-center gap-2">
                              {index.unique && (
                                <Badge variant="secondary">Unique</Badge>
                              )}
                              <div className="text-sm text-muted-foreground">
                                {formatBytes(index.size_bytes)}
                              </div>
                            </div>
                          </div>
                        ));
                      })()}
                    </div>
                  </CardContent>
                </Card>

                {/* Schema Visualization */}
                <Card>
                  <CardHeader>
                    <CardTitle className="flex items-center gap-2">
                      <Layers className="h-5 w-5" />
                      Hybrid Storage Schema
                    </CardTitle>
                    <CardDescription>
                      Field definitions, types, and caching strategies for VedDB v0.2.0 dual-layer architecture
                    </CardDescription>
                  </CardHeader>
                  <CardContent>
                    {mockSchemas[selectedCollection] ? (
                      <div className="space-y-3">
                        {/* Storage Layer Summary */}
                        <div className="grid grid-cols-2 gap-4 mb-6 p-4 bg-muted/30 rounded-lg">
                          <div className="text-center">
                            <div className="flex items-center justify-center gap-2 mb-2">
                              <HardDrive className="h-5 w-5 text-blue-600" />
                              <span className="font-medium">Persistent Layer</span>
                            </div>
                            <div className="text-sm text-muted-foreground">
                              {mockSchemas[selectedCollection].filter(f => !f.cached).length} fields
                            </div>
                          </div>
                          <div className="text-center">
                            <div className="flex items-center justify-center gap-2 mb-2">
                              <Zap className="h-5 w-5 text-emerald-600" />
                              <span className="font-medium">Cache Layer</span>
                            </div>
                            <div className="text-sm text-muted-foreground">
                              {mockSchemas[selectedCollection].filter(f => f.cached).length} fields
                            </div>
                          </div>
                        </div>

                        {/* Field List */}
                        <div className="space-y-2">
                          {mockSchemas[selectedCollection].map((field) => (
                            <div key={field.name} className="flex items-center justify-between p-3 border rounded hover:bg-muted/50 transition-colors">
                              <div className="flex items-center gap-3">
                                {getTypeIcon(field.type)}
                                <div>
                                  <div className="font-medium flex items-center gap-2">
                                    {field.name}
                                    {field.required && (
                                      <Badge variant="destructive" className="text-xs">
                                        required
                                      </Badge>
                                    )}
                                  </div>
                                  <div className="text-sm text-muted-foreground flex items-center gap-2">
                                    <span>{field.type}</span>
                                    {field.cached && field.cache_strategy && (
                                      <span className={`text-xs px-2 py-1 rounded border ${getCacheStrategyColor(field.cache_strategy)}`}>
                                        {field.cache_strategy}
                                      </span>
                                    )}
                                  </div>
                                </div>
                              </div>
                              <div className="flex items-center gap-2">
                                {field.indexed && (
                                  <Badge variant="outline" className="text-xs">
                                    <Key className="h-3 w-3 mr-1" />
                                    indexed
                                  </Badge>
                                )}
                                {field.cached ? (
                                  <div className="flex items-center gap-1">
                                    <Badge variant="outline" className="text-xs bg-emerald-50 text-emerald-700 border-emerald-200">
                                      <Zap className="h-3 w-3 mr-1" />
                                      cached
                                    </Badge>
                                    {field.ttl && (
                                      <Badge variant="outline" className="text-xs bg-blue-50 text-blue-700 border-blue-200">
                                        <Clock className="h-3 w-3 mr-1" />
                                        {field.ttl}s TTL
                                      </Badge>
                                    )}
                                  </div>
                                ) : (
                                  <Badge variant="outline" className="text-xs bg-gray-50 text-gray-700 border-gray-200">
                                    <HardDrive className="h-3 w-3 mr-1" />
                                    persistent
                                  </Badge>
                                )}
                              </div>
                            </div>
                          ))}
                        </div>

                        {/* Cache Strategy Legend */}
                        <div className="mt-6 p-4 bg-muted/30 rounded-lg">
                          <h4 className="font-medium mb-3 flex items-center gap-2">
                            <Settings className="h-4 w-4" />
                            Cache Strategy Guide
                          </h4>
                          <div className="grid grid-cols-1 md:grid-cols-2 gap-3 text-sm">
                            <div className="flex items-center gap-2">
                              <div className="w-3 h-3 bg-blue-500 rounded-full"></div>
                              <span className="font-medium">write-through:</span>
                              <span className="text-muted-foreground">Update cache + persistent simultaneously</span>
                            </div>
                            <div className="flex items-center gap-2">
                              <div className="w-3 h-3 bg-green-500 rounded-full"></div>
                              <span className="font-medium">write-behind:</span>
                              <span className="text-muted-foreground">Update cache first, persist async</span>
                            </div>
                            <div className="flex items-center gap-2">
                              <div className="w-3 h-3 bg-purple-500 rounded-full"></div>
                              <span className="font-medium">read-through:</span>
                              <span className="text-muted-foreground">Load from persistent on cache miss</span>
                            </div>
                            <div className="flex items-center gap-2">
                              <div className="w-3 h-3 bg-gray-500 rounded-full"></div>
                              <span className="font-medium">none:</span>
                              <span className="text-muted-foreground">Persistent storage only</span>
                            </div>
                          </div>
                        </div>
                      </div>
                    ) : (
                      <div className="text-center text-muted-foreground py-8">
                        <Layers className="h-8 w-8 mx-auto mb-2 opacity-50" />
                        <div className="font-medium">Schema information not available</div>
                        <div className="text-sm">This collection doesn't have schema metadata available</div>
                      </div>
                    )}
                  </CardContent>
                </Card>
              </div>
            </ScrollArea>
          </>
        ) : (
          <div className="flex-1 p-6">
            {/* Database Overview */}
            <div className="max-w-4xl mx-auto space-y-6">
              <div className="text-center mb-8">
                <Database className="h-16 w-16 mx-auto text-muted-foreground mb-4" />
                <h2 className="text-2xl font-bold mb-2">Database Overview</h2>
                <p className="text-muted-foreground">
                  VedDB v0.2.0 Hybrid Storage Architecture - Select a collection to explore its schema and performance
                </p>
              </div>

              {/* Database Statistics */}
              <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                <Card>
                  <CardHeader className="pb-3">
                    <CardTitle className="text-base flex items-center gap-2">
                      <Table className="h-4 w-4" />
                      Collections
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <div className="text-2xl font-bold text-blue-600 mb-1">
                      {collections.length}
                    </div>
                    <div className="text-sm text-muted-foreground">
                      Total collections in database
                    </div>
                  </CardContent>
                </Card>

                <Card>
                  <CardHeader className="pb-3">
                    <CardTitle className="text-base flex items-center gap-2">
                      <FileText className="h-4 w-4" />
                      Documents
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <div className="text-2xl font-bold text-green-600 mb-1">
                      {formatNumber(collections.reduce((sum, c) => sum + c.document_count, 0))}
                    </div>
                    <div className="text-sm text-muted-foreground">
                      Total documents across all collections
                    </div>
                  </CardContent>
                </Card>

                <Card>
                  <CardHeader className="pb-3">
                    <CardTitle className="text-base flex items-center gap-2">
                      <HardDrive className="h-4 w-4" />
                      Storage
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <div className="text-2xl font-bold text-purple-600 mb-1">
                      {formatBytes(collections.reduce((sum, c) => sum + c.size_bytes, 0))}
                    </div>
                    <div className="text-sm text-muted-foreground">
                      Total persistent storage used
                    </div>
                  </CardContent>
                </Card>
              </div>

              {/* Hybrid Architecture Overview */}
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    <Layers className="h-5 w-5" />
                    Hybrid Storage Architecture
                  </CardTitle>
                  <CardDescription>
                    VedDB v0.2.0 combines MongoDB-like document storage with Redis-like caching
                  </CardDescription>
                </CardHeader>
                <CardContent>
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                    <div className="space-y-3">
                      <div className="flex items-center gap-2 text-blue-600">
                        <HardDrive className="h-5 w-5" />
                        <span className="font-medium">Persistent Layer</span>
                      </div>
                      <ul className="text-sm text-muted-foreground space-y-1 ml-7">
                        <li>• RocksDB-backed document storage</li>
                        <li>• ACID transactions and durability</li>
                        <li>• Complex queries and aggregations</li>
                        <li>• Automatic indexing and optimization</li>
                      </ul>
                    </div>
                    
                    <div className="space-y-3">
                      <div className="flex items-center gap-2 text-emerald-600">
                        <Zap className="h-5 w-5" />
                        <span className="font-medium">Cache Layer</span>
                      </div>
                      <ul className="text-sm text-muted-foreground space-y-1 ml-7">
                        <li>• In-memory Redis-like data structures</li>
                        <li>• Sub-millisecond response times</li>
                        <li>• TTL-based expiration policies</li>
                        <li>• Configurable cache strategies</li>
                      </ul>
                    </div>
                  </div>
                </CardContent>
              </Card>

              {/* Quick Actions */}
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    <Settings className="h-5 w-5" />
                    Quick Actions
                  </CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                    <Button variant="outline" className="h-auto p-4 flex flex-col items-center gap-2">
                      <Table className="h-5 w-5" />
                      <span className="text-sm">Create Collection</span>
                    </Button>
                    <Button variant="outline" className="h-auto p-4 flex flex-col items-center gap-2">
                      <Key className="h-5 w-5" />
                      <span className="text-sm">Manage Indexes</span>
                    </Button>
                    <Button variant="outline" className="h-auto p-4 flex flex-col items-center gap-2">
                      <Users className="h-5 w-5" />
                      <span className="text-sm">User Management</span>
                    </Button>
                    <Button variant="outline" className="h-auto p-4 flex flex-col items-center gap-2">
                      <BarChart3 className="h-5 w-5" />
                      <span className="text-sm">View Metrics</span>
                    </Button>
                  </div>
                </CardContent>
              </Card>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}