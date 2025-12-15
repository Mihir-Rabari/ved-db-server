import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { useConnectionStore } from '@/store';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { useToast } from '@/hooks/use-toast';
import {
  Key,
  Plus,
  Trash2,
  RefreshCw,
  Database,
  BarChart3,
  Clock,

  Search,
  AlertTriangle,
  CheckCircle,
  Info,
  TrendingUp,
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

interface IndexStatistics {
  name: string;
  collection: string;
  fields: string[];
  unique: boolean;
  size_bytes: number;
  usage_count: number;
  last_used: string | null;
  build_time_ms: number;
  cardinality: number;
  selectivity: number;
}

interface CreateIndexForm {
  collection: string;
  name: string;
  fields: string[];
  unique: boolean;
}

export function IndexManager() {
  const { activeConnection } = useConnectionStore();
  const { toast } = useToast();
  const [collections, setCollections] = useState<CollectionInfo[]>([]);
  const [selectedCollection, setSelectedCollection] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [searchTerm, setSearchTerm] = useState('');
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [indexToDelete, setIndexToDelete] = useState<{ collection: string; name: string } | null>(null);
  const [createForm, setCreateForm] = useState<CreateIndexForm>({
    collection: '',
    name: '',
    fields: [''],
    unique: false,
  });
  const [creating, setCreating] = useState(false);
  const [deleting, setDeleting] = useState(false);

  // Mock index statistics for demonstration
  const mockIndexStats: Record<string, IndexStatistics[]> = {
    users: [
      {
        name: '_id',
        collection: 'users',
        fields: ['_id'],
        unique: true,
        size_bytes: 32768,
        usage_count: 1250,
        last_used: '2024-11-25T10:30:00Z',
        build_time_ms: 45,
        cardinality: 1250,
        selectivity: 1.0,
      },
      {
        name: 'email_idx',
        collection: 'users',
        fields: ['email'],
        unique: true,
        size_bytes: 65536,
        usage_count: 890,
        last_used: '2024-11-25T09:15:00Z',
        build_time_ms: 120,
        cardinality: 1250,
        selectivity: 1.0,
      },
      {
        name: 'last_login_idx',
        collection: 'users',
        fields: ['last_login'],
        unique: false,
        size_bytes: 24576,
        usage_count: 234,
        last_used: '2024-11-24T16:45:00Z',
        build_time_ms: 89,
        cardinality: 890,
        selectivity: 0.712,
      },
    ],
    products: [
      {
        name: '_id',
        collection: 'products',
        fields: ['_id'],
        unique: true,
        size_bytes: 65536,
        usage_count: 5000,
        last_used: '2024-11-25T10:25:00Z',
        build_time_ms: 156,
        cardinality: 5000,
        selectivity: 1.0,
      },
      {
        name: 'name_idx',
        collection: 'products',
        fields: ['name'],
        unique: false,
        size_bytes: 98304,
        usage_count: 3200,
        last_used: '2024-11-25T08:30:00Z',
        build_time_ms: 234,
        cardinality: 4890,
        selectivity: 0.978,
      },
      {
        name: 'category_price_idx',
        collection: 'products',
        fields: ['category', 'price'],
        unique: false,
        size_bytes: 131072,
        usage_count: 1800,
        last_used: '2024-11-25T07:20:00Z',
        build_time_ms: 345,
        cardinality: 2340,
        selectivity: 0.468,
      },
    ],
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
      if (result.length > 0 && !selectedCollection) {
        setSelectedCollection(result[0].name);
      }
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

  const refreshData = async () => {
    setRefreshing(true);
    await loadCollections();
    setRefreshing(false);
    toast({
      title: 'Data Refreshed',
      description: 'Index information has been updated.',
    });
  };

  const handleCreateIndex = async () => {
    if (!activeConnection?.isConnected) return;
    if (!createForm.collection || !createForm.name || createForm.fields.length === 0) {
      toast({
        title: 'Invalid Form',
        description: 'Please fill in all required fields.',
        variant: 'destructive',
      });
      return;
    }

    setCreating(true);
    try {
      await invoke('create_index', {
        connectionId: activeConnection.id,
        collection: createForm.collection,
        indexName: createForm.name,
        fields: createForm.fields.filter(f => f.trim() !== ''),
        unique: createForm.unique,
      });

      toast({
        title: 'Index Created',
        description: `Index "${createForm.name}" has been created successfully.`,
      });

      setCreateDialogOpen(false);
      setCreateForm({
        collection: '',
        name: '',
        fields: [''],
        unique: false,
      });
      await loadCollections();
    } catch (error) {
      toast({
        title: 'Error Creating Index',
        description: error as string,
        variant: 'destructive',
      });
    } finally {
      setCreating(false);
    }
  };

  const handleDeleteIndex = async () => {
    if (!activeConnection?.isConnected || !indexToDelete) return;

    setDeleting(true);
    try {
      await invoke('drop_index', {
        connectionId: activeConnection.id,
        collection: indexToDelete.collection,
        indexName: indexToDelete.name,
      });

      toast({
        title: 'Index Deleted',
        description: `Index "${indexToDelete.name}" has been deleted successfully.`,
      });

      setDeleteDialogOpen(false);
      setIndexToDelete(null);
      await loadCollections();
    } catch (error) {
      toast({
        title: 'Error Deleting Index',
        description: error as string,
        variant: 'destructive',
      });
    } finally {
      setDeleting(false);
    }
  };

  const addFieldToForm = () => {
    setCreateForm(prev => ({
      ...prev,
      fields: [...prev.fields, ''],
    }));
  };

  const removeFieldFromForm = (index: number) => {
    setCreateForm(prev => ({
      ...prev,
      fields: prev.fields.filter((_, i) => i !== index),
    }));
  };

  const updateFieldInForm = (index: number, value: string) => {
    setCreateForm(prev => ({
      ...prev,
      fields: prev.fields.map((field, i) => i === index ? value : field),
    }));
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

  const formatDate = (dateStr: string | null): string => {
    if (!dateStr) return 'Never';
    return new Date(dateStr).toLocaleString();
  };

  const getIndexHealthColor = (stats: IndexStatistics): string => {
    if (stats.usage_count === 0) return 'text-red-600';
    if (stats.usage_count < 10) return 'text-yellow-600';
    return 'text-green-600';
  };

  const getIndexHealthIcon = (stats: IndexStatistics) => {
    if (stats.usage_count === 0) return <AlertTriangle className="h-4 w-4 text-red-600" />;
    if (stats.usage_count < 10) return <Info className="h-4 w-4 text-yellow-600" />;
    return <CheckCircle className="h-4 w-4 text-green-600" />;
  };

  useEffect(() => {
    if (activeConnection?.isConnected) {
      loadCollections();
    }
  }, [activeConnection]);

  if (!activeConnection?.isConnected) {
    return (
      <div className="flex items-center justify-center h-full flex-1">
        <Card className="w-96">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Key className="h-5 w-5" />
              Index Manager
            </CardTitle>
            <CardDescription>
              Connect to a VedDB server to manage database indexes.
            </CardDescription>
          </CardHeader>
        </Card>
      </div>
    );
  }

  const selectedCollectionData = collections.find(c => c.name === selectedCollection);
  const selectedIndexStats = selectedCollection ? mockIndexStats[selectedCollection] || [] : [];
  const filteredIndexes = selectedIndexStats.filter(index =>
    index.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
    index.fields.some(field => field.toLowerCase().includes(searchTerm.toLowerCase()))
  );

  return (
    <div className="flex h-full flex-1">
      {/* Left Panel - Collections */}
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
              onClick={refreshData}
              disabled={refreshing}
            >
              <RefreshCw className={`h-4 w-4 ${refreshing ? 'animate-spin' : ''}`} />
            </Button>
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
              {collections.map((collection) => (
                <div
                  key={collection.name}
                  className={`flex items-center justify-between p-3 rounded-lg cursor-pointer hover:bg-muted/50 transition-colors mb-2 ${
                    selectedCollection === collection.name ? 'bg-muted border-l-4 border-l-blue-500' : 'border border-transparent'
                  }`}
                  onClick={() => setSelectedCollection(collection.name)}
                >
                  <div className="flex items-center gap-3">
                    <Database className="h-4 w-4 text-blue-600" />
                    <div>
                      <div className="font-medium">{collection.name}</div>
                      <div className="text-xs text-muted-foreground">
                        {formatNumber(collection.document_count)} docs
                      </div>
                    </div>
                  </div>
                  <Badge variant="secondary" className="text-xs">
                    {collection.indexes.length} idx
                  </Badge>
                </div>
              ))}
            </div>
          )}
        </ScrollArea>
      </div>

      {/* Right Panel - Index Management */}
      <div className="flex-1 flex flex-col">
        {selectedCollection ? (
          <>
            <div className="p-4 border-b">
              <div className="flex items-center justify-between">
                <div>
                  <h2 className="text-xl font-semibold flex items-center gap-2">
                    <Key className="h-5 w-5" />
                    Indexes - {selectedCollection}
                  </h2>
                  <p className="text-muted-foreground">
                    Manage indexes for improved query performance
                  </p>
                </div>
                <Dialog open={createDialogOpen} onOpenChange={setCreateDialogOpen}>
                  <DialogTrigger asChild>
                    <Button>
                      <Plus className="h-4 w-4 mr-2" />
                      Create Index
                    </Button>
                  </DialogTrigger>
                  <DialogContent className="sm:max-w-[500px]">
                    <DialogHeader>
                      <DialogTitle>Create New Index</DialogTitle>
                      <DialogDescription>
                        Create a new index to improve query performance on specific fields.
                      </DialogDescription>
                    </DialogHeader>
                    <div className="space-y-4">
                      <div className="space-y-2">
                        <Label htmlFor="collection">Collection</Label>
                        <Select
                          value={createForm.collection}
                          onValueChange={(value) => setCreateForm(prev => ({ ...prev, collection: value }))}
                        >
                          <SelectTrigger>
                            <SelectValue placeholder="Select collection" />
                          </SelectTrigger>
                          <SelectContent>
                            {collections.map((collection) => (
                              <SelectItem key={collection.name} value={collection.name}>
                                {collection.name}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </div>
                      <div className="space-y-2">
                        <Label htmlFor="name">Index Name</Label>
                        <Input
                          id="name"
                          value={createForm.name}
                          onChange={(e) => setCreateForm(prev => ({ ...prev, name: e.target.value }))}
                          placeholder="e.g., email_idx"
                        />
                      </div>
                      <div className="space-y-2">
                        <Label>Fields</Label>
                        {createForm.fields.map((field, index) => (
                          <div key={index} className="flex items-center gap-2">
                            <Input
                              value={field}
                              onChange={(e) => updateFieldInForm(index, e.target.value)}
                              placeholder="Field name"
                            />
                            {createForm.fields.length > 1 && (
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => removeFieldFromForm(index)}
                              >
                                <Trash2 className="h-4 w-4" />
                              </Button>
                            )}
                          </div>
                        ))}
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={addFieldToForm}
                          className="w-full"
                        >
                          <Plus className="h-4 w-4 mr-2" />
                          Add Field
                        </Button>
                      </div>
                      <div className="flex items-center space-x-2">
                        <Switch
                          id="unique"
                          checked={createForm.unique}
                          onCheckedChange={(checked) => setCreateForm(prev => ({ ...prev, unique: checked }))}
                        />
                        <Label htmlFor="unique">Unique Index</Label>
                      </div>
                    </div>
                    <DialogFooter>
                      <Button variant="outline" onClick={() => setCreateDialogOpen(false)}>
                        Cancel
                      </Button>
                      <Button onClick={handleCreateIndex} disabled={creating}>
                        {creating ? 'Creating...' : 'Create Index'}
                      </Button>
                    </DialogFooter>
                  </DialogContent>
                </Dialog>
              </div>
            </div>

            <div className="p-4 border-b">
              <div className="relative">
                <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-muted-foreground" />
                <Input
                  type="text"
                  placeholder="Search indexes..."
                  value={searchTerm}
                  onChange={(e) => setSearchTerm(e.target.value)}
                  className="pl-10"
                />
              </div>
            </div>

            <ScrollArea className="flex-1 p-4">
              <div className="space-y-6">
                {/* Collection Statistics */}
                {selectedCollectionData && (
                  <Card>
                    <CardHeader>
                      <CardTitle className="flex items-center gap-2">
                        <BarChart3 className="h-5 w-5" />
                        Collection Overview
                      </CardTitle>
                    </CardHeader>
                    <CardContent>
                      <div className="grid grid-cols-3 gap-4">
                        <div className="text-center">
                          <div className="text-2xl font-bold text-blue-600">
                            {formatNumber(selectedCollectionData.document_count)}
                          </div>
                          <div className="text-sm text-muted-foreground">Documents</div>
                        </div>
                        <div className="text-center">
                          <div className="text-2xl font-bold text-green-600">
                            {selectedCollectionData.indexes.length}
                          </div>
                          <div className="text-sm text-muted-foreground">Indexes</div>
                        </div>
                        <div className="text-center">
                          <div className="text-2xl font-bold text-purple-600">
                            {formatBytes(selectedCollectionData.size_bytes)}
                          </div>
                          <div className="text-sm text-muted-foreground">Total Size</div>
                        </div>
                      </div>
                    </CardContent>
                  </Card>
                )}

                {/* Index List */}
                <Card>
                  <CardHeader>
                    <CardTitle className="flex items-center gap-2">
                      <Key className="h-5 w-5" />
                      Indexes ({filteredIndexes.length})
                    </CardTitle>
                    <CardDescription>
                      Database indexes with performance statistics and usage metrics
                    </CardDescription>
                  </CardHeader>
                  <CardContent>
                    {filteredIndexes.length === 0 ? (
                      <div className="text-center py-8 text-muted-foreground">
                        <Key className="h-8 w-8 mx-auto mb-2 opacity-50" />
                        <div className="font-medium">No indexes found</div>
                        <div className="text-sm">
                          {searchTerm ? 'Try adjusting your search terms' : 'Create your first index to improve query performance'}
                        </div>
                      </div>
                    ) : (
                      <div className="space-y-4">
                        {filteredIndexes.map((index) => (
                          <div key={index.name} className="border rounded-lg p-4 hover:bg-muted/30 transition-colors">
                            <div className="flex items-start justify-between mb-3">
                              <div className="flex items-start gap-3">
                                {getIndexHealthIcon(index)}
                                <div>
                                  <div className="font-medium flex items-center gap-2">
                                    {index.name}
                                    {index.unique && (
                                      <Badge variant="secondary" className="text-xs">
                                        Unique
                                      </Badge>
                                    )}
                                    {index.name === '_id' && (
                                      <Badge variant="outline" className="text-xs">
                                        Primary
                                      </Badge>
                                    )}
                                  </div>
                                  <div className="text-sm text-muted-foreground">
                                    Fields: {index.fields.join(', ')}
                                  </div>
                                </div>
                              </div>
                              {index.name !== '_id' && (
                                <Button
                                  variant="ghost"
                                  size="sm"
                                  onClick={() => {
                                    setIndexToDelete({ collection: index.collection, name: index.name });
                                    setDeleteDialogOpen(true);
                                  }}
                                  className="text-red-600 hover:text-red-700 hover:bg-red-50"
                                >
                                  <Trash2 className="h-4 w-4" />
                                </Button>
                              )}
                            </div>

                            <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
                              <div>
                                <div className="text-muted-foreground">Size</div>
                                <div className="font-medium">{formatBytes(index.size_bytes)}</div>
                              </div>
                              <div>
                                <div className="text-muted-foreground">Usage Count</div>
                                <div className={`font-medium ${getIndexHealthColor(index)}`}>
                                  {formatNumber(index.usage_count)}
                                </div>
                              </div>
                              <div>
                                <div className="text-muted-foreground">Cardinality</div>
                                <div className="font-medium">{formatNumber(index.cardinality)}</div>
                              </div>
                              <div>
                                <div className="text-muted-foreground">Selectivity</div>
                                <div className="font-medium">{(index.selectivity * 100).toFixed(1)}%</div>
                              </div>
                            </div>

                            <div className="mt-3 pt-3 border-t grid grid-cols-2 gap-4 text-sm">
                              <div>
                                <div className="text-muted-foreground flex items-center gap-1">
                                  <Clock className="h-3 w-3" />
                                  Last Used
                                </div>
                                <div className="font-medium">{formatDate(index.last_used)}</div>
                              </div>
                              <div>
                                <div className="text-muted-foreground flex items-center gap-1">
                                  <TrendingUp className="h-3 w-3" />
                                  Build Time
                                </div>
                                <div className="font-medium">{index.build_time_ms}ms</div>
                              </div>
                            </div>

                            {/* Performance Indicator */}
                            <div className="mt-3 pt-3 border-t">
                              <div className="flex items-center justify-between mb-2">
                                <span className="text-sm font-medium">Performance Impact</span>
                                <Badge variant={index.usage_count > 100 ? "default" : index.usage_count > 10 ? "secondary" : "destructive"}>
                                  {index.usage_count > 100 ? "High" : index.usage_count > 10 ? "Medium" : "Low"}
                                </Badge>
                              </div>
                              <div className="w-full bg-muted rounded-full h-2">
                                <div 
                                  className={`h-2 rounded-full transition-all duration-300 ${
                                    index.usage_count > 100 ? 'bg-green-600' : 
                                    index.usage_count > 10 ? 'bg-yellow-600' : 'bg-red-600'
                                  }`}
                                  style={{ width: `${Math.min((index.usage_count / 1000) * 100, 100)}%` }}
                                />
                              </div>
                            </div>
                          </div>
                        ))}
                      </div>
                    )}
                  </CardContent>
                </Card>
              </div>
            </ScrollArea>
          </>
        ) : (
          <div className="flex-1 p-6">
            <div className="max-w-2xl mx-auto text-center space-y-6">
              <div className="text-center mb-8">
                <Key className="h-16 w-16 mx-auto text-muted-foreground mb-4" />
                <h2 className="text-2xl font-bold mb-2">Index Management</h2>
                <p className="text-muted-foreground">
                  Select a collection from the sidebar to view and manage its indexes
                </p>
              </div>

              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    <Info className="h-5 w-5" />
                    About Database Indexes
                  </CardTitle>
                </CardHeader>
                <CardContent className="text-left space-y-4">
                  <div>
                    <h4 className="font-medium mb-2">What are indexes?</h4>
                    <p className="text-sm text-muted-foreground">
                      Indexes are data structures that improve query performance by creating shortcuts to your data. 
                      They work like an index in a book, allowing the database to quickly locate specific information.
                    </p>
                  </div>
                  <div>
                    <h4 className="font-medium mb-2">When to create indexes?</h4>
                    <ul className="text-sm text-muted-foreground space-y-1">
                      <li>• Fields frequently used in WHERE clauses</li>
                      <li>• Fields used for sorting (ORDER BY)</li>
                      <li>• Fields used in JOIN operations</li>
                      <li>• Unique constraints and primary keys</li>
                    </ul>
                  </div>
                  <div>
                    <h4 className="font-medium mb-2">Index types in VedDB:</h4>
                    <ul className="text-sm text-muted-foreground space-y-1">
                      <li>• <strong>Single:</strong> Index on one field</li>
                      <li>• <strong>Compound:</strong> Index on multiple fields</li>
                      <li>• <strong>Unique:</strong> Ensures field values are unique</li>
                    </ul>
                  </div>
                </CardContent>
              </Card>
            </div>
          </div>
        )}
      </div>

      {/* Delete Confirmation Dialog */}
      <Dialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <AlertTriangle className="h-5 w-5 text-red-600" />
              Delete Index
            </DialogTitle>
            <DialogDescription>
              Are you sure you want to delete the index "{indexToDelete?.name}"? This action cannot be undone and may impact query performance.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteDialogOpen(false)}>
              Cancel
            </Button>
            <Button variant="destructive" onClick={handleDeleteIndex} disabled={deleting}>
              {deleting ? 'Deleting...' : 'Delete Index'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}