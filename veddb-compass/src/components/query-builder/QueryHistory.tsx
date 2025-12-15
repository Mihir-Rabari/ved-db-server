import { useState } from 'react';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { 
  History, 
  Search, 
  Trash2, 
  Clock, 
  Database,
  AlertCircle,
  Play,
  Copy
} from 'lucide-react';
import { QueryHistoryItem } from '../QueryBuilder';

interface QueryHistoryProps {
  history: QueryHistoryItem[];
  onLoadQuery: (item: QueryHistoryItem) => void;
  onClearHistory: () => void;
}

export function QueryHistory({ history, onLoadQuery, onClearHistory }: QueryHistoryProps) {
  const [searchTerm, setSearchTerm] = useState('');
  const [selectedCollection, setSelectedCollection] = useState<string>('');

  // Get unique collections from history
  const collections = Array.from(new Set(history.map(item => item.collection))).filter(Boolean);

  // Filter history based on search and collection
  const filteredHistory = history.filter(item => {
    const matchesSearch = !searchTerm || 
      item.query.toLowerCase().includes(searchTerm.toLowerCase()) ||
      item.collection.toLowerCase().includes(searchTerm.toLowerCase());
    
    const matchesCollection = !selectedCollection || item.collection === selectedCollection;
    
    return matchesSearch && matchesCollection;
  });

  const formatTimestamp = (timestamp: Date) => {
    const now = new Date();
    const diff = now.getTime() - timestamp.getTime();
    
    if (diff < 60000) { // Less than 1 minute
      return 'Just now';
    } else if (diff < 3600000) { // Less than 1 hour
      const minutes = Math.floor(diff / 60000);
      return `${minutes}m ago`;
    } else if (diff < 86400000) { // Less than 1 day
      const hours = Math.floor(diff / 3600000);
      return `${hours}h ago`;
    } else {
      const days = Math.floor(diff / 86400000);
      return `${days}d ago`;
    }
  };

  const formatQuery = (query: string) => {
    try {
      const parsed = JSON.parse(query);
      
      // Create a summary of the query
      const parts = [];
      
      if (parsed.filter && Object.keys(parsed.filter).length > 0) {
        const filterKeys = Object.keys(parsed.filter);
        parts.push(`Filter: ${filterKeys.slice(0, 2).join(', ')}${filterKeys.length > 2 ? '...' : ''}`);
      }
      
      if (parsed.sort && Object.keys(parsed.sort).length > 0) {
        const sortKeys = Object.keys(parsed.sort);
        parts.push(`Sort: ${sortKeys[0]}`);
      }
      
      if (parsed.limit) {
        parts.push(`Limit: ${parsed.limit}`);
      }
      
      return parts.length > 0 ? parts.join(' • ') : 'Empty query';
    } catch {
      return 'Invalid JSON';
    }
  };

  const copyQuery = async (query: string) => {
    try {
      await navigator.clipboard.writeText(query);
    } catch (error) {
      console.error('Failed to copy query:', error);
    }
  };

  if (history.length === 0) {
    return (
      <div className="h-full flex flex-col">
        <div className="p-4 border-b">
          <h3 className="font-medium flex items-center gap-2">
            <History className="h-4 w-4" />
            Query History
          </h3>
        </div>
        
        <div className="flex-1 flex items-center justify-center p-4">
          <div className="text-center">
            <History className="h-12 w-12 mx-auto mb-4 text-muted-foreground" />
            <h4 className="font-medium mb-2">No Query History</h4>
            <p className="text-sm text-muted-foreground">
              Execute queries to see them appear here.
            </p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-4 border-b">
        <div className="flex items-center justify-between mb-3">
          <h3 className="font-medium flex items-center gap-2">
            <History className="h-4 w-4" />
            Query History
          </h3>
          
          <Button
            variant="outline"
            size="sm"
            onClick={onClearHistory}
            className="gap-1 text-xs"
          >
            <Trash2 className="h-3 w-3" />
            Clear
          </Button>
        </div>
        
        {/* Search */}
        <div className="space-y-2">
          <div className="relative">
            <Search className="absolute left-2 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              placeholder="Search queries..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="pl-8 text-sm"
            />
          </div>
          
          {/* Collection Filter */}
          {collections.length > 0 && (
            <select
              value={selectedCollection}
              onChange={(e) => setSelectedCollection(e.target.value)}
              className="w-full px-2 py-1 text-sm border rounded bg-background"
            >
              <option value="">All Collections</option>
              {collections.map(collection => (
                <option key={collection} value={collection}>
                  {collection}
                </option>
              ))}
            </select>
          )}
        </div>
        
        {/* Results count */}
        <div className="mt-2 text-xs text-muted-foreground">
          {filteredHistory.length} of {history.length} queries
        </div>
      </div>

      {/* History List */}
      <ScrollArea className="flex-1">
        <div className="p-2 space-y-2">
          {filteredHistory.map((item) => (
            <div
              key={item.id}
              className="border rounded-lg p-3 hover:bg-muted/50 transition-colors"
            >
              {/* Item Header */}
              <div className="flex items-start justify-between mb-2">
                <div className="flex items-center gap-2 min-w-0 flex-1">
                  <Badge variant="outline" className="text-xs shrink-0">
                    <Database className="h-3 w-3 mr-1" />
                    {item.collection}
                  </Badge>
                  
                  <span className="text-xs text-muted-foreground flex items-center gap-1 shrink-0">
                    <Clock className="h-3 w-3" />
                    {formatTimestamp(item.timestamp)}
                  </span>
                </div>
                
                <div className="flex items-center gap-1 shrink-0">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => copyQuery(item.query)}
                    className="h-6 w-6 p-0"
                  >
                    <Copy className="h-3 w-3" />
                  </Button>
                  
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => onLoadQuery(item)}
                    className="h-6 w-6 p-0"
                  >
                    <Play className="h-3 w-3" />
                  </Button>
                </div>
              </div>
              
              {/* Query Summary */}
              <div className="text-sm mb-2">
                {formatQuery(item.query)}
              </div>
              
              {/* Execution Info */}
              <div className="flex items-center gap-3 text-xs text-muted-foreground">
                {item.error ? (
                  <Badge variant="destructive" className="gap-1 text-xs">
                    <AlertCircle className="h-3 w-3" />
                    Error
                  </Badge>
                ) : (
                  <>
                    {item.resultCount !== undefined && (
                      <span>{item.resultCount} results</span>
                    )}
                    {item.executionTime !== undefined && (
                      <span>{item.executionTime}ms</span>
                    )}
                  </>
                )}
              </div>
              
              {/* Error Message */}
              {item.error && (
                <div className="mt-2 p-2 bg-destructive/10 border border-destructive/20 rounded text-xs text-destructive">
                  {item.error}
                </div>
              )}
              
              {/* Query Preview */}
              <details className="mt-2">
                <summary className="text-xs text-muted-foreground cursor-pointer hover:text-foreground">
                  View Query
                </summary>
                <pre className="mt-1 p-2 bg-muted/50 rounded text-xs overflow-x-auto">
                  {item.query}
                </pre>
              </details>
            </div>
          ))}
        </div>
      </ScrollArea>
    </div>
  );
}