import { useState, useEffect } from 'react';
import { useConnectionStore } from '@/store';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Badge } from '@/components/ui/badge';
import { useToast } from '@/hooks/use-toast';
import { 
  Play, 
  History, 
  Download, 
  Copy,
  Database,
  FileText
} from 'lucide-react';
import { VisualQueryBuilder } from './query-builder/VisualQueryBuilder';
import { JsonQueryEditor } from './query-builder/JsonQueryEditor';
import { QueryResults } from './query-builder/QueryResults';
import { QueryHistory } from './query-builder/QueryHistory';

export interface QueryHistoryItem {
  id: string;
  query: string;
  collection: string;
  timestamp: Date;
  executionTime?: number;
  resultCount?: number;
  error?: string;
}

export interface QueryResult {
  documents: any[];
  totalCount: number;
  executionTime: number;
  error?: string;
}

export function QueryBuilder() {
  const { activeConnection } = useConnectionStore();
  const { toast } = useToast();
  
  // Query state
  const [activeTab, setActiveTab] = useState<'visual' | 'json'>('visual');
  const [selectedCollection, setSelectedCollection] = useState<string>('');
  const [jsonQuery, setJsonQuery] = useState<string>('{\n  "filter": {},\n  "projection": {},\n  "sort": {},\n  "limit": 100\n}');
  const [visualQuery, setVisualQuery] = useState<any>({
    filter: {},
    projection: {},
    sort: {},
    limit: 100
  });
  
  // Results and history
  const [queryResults, setQueryResults] = useState<QueryResult | null>(null);
  const [isExecuting, setIsExecuting] = useState(false);
  const [queryHistory, setQueryHistory] = useState<QueryHistoryItem[]>([]);
  const [showHistory, setShowHistory] = useState(false);
  
  // Collections list (mock data for now)
  const collections = ['users', 'products', 'orders', 'sessions', 'logs'];

  // Load query history from localStorage
  useEffect(() => {
    const savedHistory = localStorage.getItem('veddb-query-history');
    if (savedHistory) {
      try {
        const parsed = JSON.parse(savedHistory);
        setQueryHistory(parsed.map((item: any) => ({
          ...item,
          timestamp: new Date(item.timestamp)
        })));
      } catch (error) {
        console.error('Failed to load query history:', error);
      }
    }
  }, []);

  // Save query history to localStorage
  const saveQueryHistory = (newHistory: QueryHistoryItem[]) => {
    setQueryHistory(newHistory);
    localStorage.setItem('veddb-query-history', JSON.stringify(newHistory));
  };

  // Execute query
  const executeQuery = async () => {
    if (!selectedCollection) {
      toast({
        title: "No Collection Selected",
        description: "Please select a collection to query.",
        variant: "destructive",
      });
      return;
    }

    setIsExecuting(true);
    const startTime = Date.now();
    
    try {
      // Get the query based on active tab
      const query = activeTab === 'visual' ? JSON.stringify(visualQuery, null, 2) : jsonQuery;
      
      // Mock query execution (replace with actual VedDB API call)
      await new Promise(resolve => setTimeout(resolve, 500 + Math.random() * 1000));
      
      // Mock results
      const mockResults = {
        documents: Array.from({ length: Math.floor(Math.random() * 50) + 1 }, (_, i) => ({
          _id: `doc_${i + 1}`,
          name: `Document ${i + 1}`,
          value: Math.floor(Math.random() * 1000),
          created_at: new Date(Date.now() - Math.random() * 86400000 * 30).toISOString(),
          tags: ['tag1', 'tag2'].slice(0, Math.floor(Math.random() * 3))
        })),
        totalCount: Math.floor(Math.random() * 1000) + 100,
        executionTime: Date.now() - startTime
      };

      setQueryResults(mockResults);

      // Add to history
      const historyItem: QueryHistoryItem = {
        id: crypto.randomUUID(),
        query,
        collection: selectedCollection,
        timestamp: new Date(),
        executionTime: mockResults.executionTime,
        resultCount: mockResults.documents.length
      };

      const newHistory = [historyItem, ...queryHistory].slice(0, 50); // Keep last 50 queries
      saveQueryHistory(newHistory);

      toast({
        title: "Query Executed",
        description: `Found ${mockResults.documents.length} documents in ${mockResults.executionTime}ms`,
      });

    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error occurred';
      
      setQueryResults({
        documents: [],
        totalCount: 0,
        executionTime: Date.now() - startTime,
        error: errorMessage
      });

      // Add error to history
      const historyItem: QueryHistoryItem = {
        id: crypto.randomUUID(),
        query: activeTab === 'visual' ? JSON.stringify(visualQuery, null, 2) : jsonQuery,
        collection: selectedCollection,
        timestamp: new Date(),
        executionTime: Date.now() - startTime,
        error: errorMessage
      };

      const newHistory = [historyItem, ...queryHistory].slice(0, 50);
      saveQueryHistory(newHistory);

      toast({
        title: "Query Failed",
        description: errorMessage,
        variant: "destructive",
      });
    } finally {
      setIsExecuting(false);
    }
  };

  // Load query from history
  const loadQueryFromHistory = (historyItem: QueryHistoryItem) => {
    try {
      const parsedQuery = JSON.parse(historyItem.query);
      setVisualQuery(parsedQuery);
      setJsonQuery(historyItem.query);
      setSelectedCollection(historyItem.collection);
      setShowHistory(false);
      
      toast({
        title: "Query Loaded",
        description: "Query loaded from history",
      });
    } catch (error) {
      toast({
        title: "Failed to Load Query",
        description: "Invalid query format in history",
        variant: "destructive",
      });
    }
  };

  // Clear query history
  const clearHistory = () => {
    setQueryHistory([]);
    localStorage.removeItem('veddb-query-history');
    toast({
      title: "History Cleared",
      description: "Query history has been cleared",
    });
  };

  // Export results
  const exportResults = () => {
    if (!queryResults || queryResults.documents.length === 0) {
      toast({
        title: "No Results",
        description: "No query results to export",
        variant: "destructive",
      });
      return;
    }

    const dataStr = JSON.stringify(queryResults.documents, null, 2);
    const dataBlob = new Blob([dataStr], { type: 'application/json' });
    const url = URL.createObjectURL(dataBlob);
    
    const link = document.createElement('a');
    link.href = url;
    link.download = `veddb-query-results-${selectedCollection}-${new Date().toISOString().split('T')[0]}.json`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);

    toast({
      title: "Results Exported",
      description: "Query results exported to JSON file",
    });
  };

  // Copy query to clipboard
  const copyQuery = async () => {
    const query = activeTab === 'visual' ? JSON.stringify(visualQuery, null, 2) : jsonQuery;
    
    try {
      await navigator.clipboard.writeText(query);
      toast({
        title: "Query Copied",
        description: "Query copied to clipboard",
      });
    } catch (error) {
      toast({
        title: "Copy Failed",
        description: "Failed to copy query to clipboard",
        variant: "destructive",
      });
    }
  };

  if (!activeConnection?.isConnected) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Card className="w-96">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Database className="h-5 w-5" />
              No Connection
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-muted-foreground">
              Connect to a VedDB server to use the query builder.
            </p>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col h-screen">
      {/* Header */}
      <div className="border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
        <div className="flex items-center justify-between p-4">
          <div className="flex items-center gap-4">
            <h1 className="text-2xl font-semibold">Query Builder</h1>
            <Badge variant="outline" className="text-xs">
              {activeConnection.name}
            </Badge>
          </div>
          
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setShowHistory(!showHistory)}
              className="gap-2"
            >
              <History className="h-4 w-4" />
              History ({queryHistory.length})
            </Button>
            
            <Button
              variant="outline"
              size="sm"
              onClick={copyQuery}
              className="gap-2"
            >
              <Copy className="h-4 w-4" />
              Copy Query
            </Button>
            
            {queryResults && (
              <Button
                variant="outline"
                size="sm"
                onClick={exportResults}
                className="gap-2"
              >
                <Download className="h-4 w-4" />
                Export Results
              </Button>
            )}
            
            <Button
              onClick={executeQuery}
              disabled={isExecuting || !selectedCollection}
              className="gap-2"
            >
              <Play className="h-4 w-4" />
              {isExecuting ? 'Executing...' : 'Execute Query'}
            </Button>
          </div>
        </div>
      </div>

      <div className="flex-1 flex">
        {/* Query History Sidebar */}
        {showHistory && (
          <div className="w-80 border-r bg-muted/30">
            <QueryHistory
              history={queryHistory}
              onLoadQuery={loadQueryFromHistory}
              onClearHistory={clearHistory}
            />
          </div>
        )}

        {/* Main Content */}
        <div className="flex-1 flex flex-col">
          {/* Query Builder */}
          <div className="flex-1 p-4">
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 h-full">
              {/* Query Input */}
              <Card className="flex flex-col">
                <CardHeader className="pb-3">
                  <div className="flex items-center justify-between">
                    <CardTitle className="text-lg">Query</CardTitle>
                    <div className="flex items-center gap-2">
                      <select
                        value={selectedCollection}
                        onChange={(e) => setSelectedCollection(e.target.value)}
                        className="px-3 py-1 text-sm border rounded-md bg-background"
                      >
                        <option value="">Select Collection</option>
                        {collections.map((collection) => (
                          <option key={collection} value={collection}>
                            {collection}
                          </option>
                        ))}
                      </select>
                    </div>
                  </div>
                </CardHeader>
                
                <CardContent className="flex-1 flex flex-col">
                  <Tabs value={activeTab} onValueChange={(value) => setActiveTab(value as 'visual' | 'json')}>
                    <TabsList className="grid w-full grid-cols-2">
                      <TabsTrigger value="visual">Visual Builder</TabsTrigger>
                      <TabsTrigger value="json">JSON Editor</TabsTrigger>
                    </TabsList>
                    
                    <TabsContent value="visual" className="flex-1 mt-4">
                      <VisualQueryBuilder
                        query={visualQuery}
                        onChange={setVisualQuery}
                        collection={selectedCollection}
                      />
                    </TabsContent>
                    
                    <TabsContent value="json" className="flex-1 mt-4">
                      <JsonQueryEditor
                        value={jsonQuery}
                        onChange={setJsonQuery}
                      />
                    </TabsContent>
                  </Tabs>
                </CardContent>
              </Card>

              {/* Query Results */}
              <Card className="flex flex-col">
                <CardHeader className="pb-3">
                  <CardTitle className="text-lg flex items-center gap-2">
                    <FileText className="h-5 w-5" />
                    Results
                    {queryResults && (
                      <Badge variant="secondary" className="text-xs">
                        {queryResults.documents.length} docs
                      </Badge>
                    )}
                  </CardTitle>
                </CardHeader>
                
                <CardContent className="flex-1 flex flex-col">
                  <QueryResults
                    results={queryResults}
                    isLoading={isExecuting}
                  />
                </CardContent>
              </Card>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}