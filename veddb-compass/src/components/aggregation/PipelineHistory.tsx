import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { ScrollArea } from '@/components/ui/scroll-area';
import { 
  Search, 
  Trash2, 
  Clock, 
  Database,
  AlertCircle,
  Play,
  Filter
} from 'lucide-react';
import { PipelineHistoryItem } from '../AggregationBuilder';

interface PipelineHistoryProps {
  history: PipelineHistoryItem[];
  onLoadPipeline: (item: PipelineHistoryItem) => void;
  onClearHistory: () => void;
}

export function PipelineHistory({ 
  history, 
  onLoadPipeline, 
  onClearHistory 
}: PipelineHistoryProps) {
  const [searchTerm, setSearchTerm] = useState('');
  const [filterCollection, setFilterCollection] = useState('');

  // Get unique collections from history
  const collections = Array.from(new Set(history.map(item => item.collection)));

  // Filter history based on search and collection filter
  const filteredHistory = history.filter(item => {
    const matchesSearch = searchTerm === '' || 
      item.collection.toLowerCase().includes(searchTerm.toLowerCase()) ||
      JSON.stringify(item.pipeline).toLowerCase().includes(searchTerm.toLowerCase());
    
    const matchesCollection = filterCollection === '' || item.collection === filterCollection;
    
    return matchesSearch && matchesCollection;
  });

  const formatTimestamp = (timestamp: Date) => {
    const now = new Date();
    const diff = now.getTime() - timestamp.getTime();
    const minutes = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);

    if (minutes < 1) return 'Just now';
    if (minutes < 60) return `${minutes}m ago`;
    if (hours < 24) return `${hours}h ago`;
    if (days < 7) return `${days}d ago`;
    
    return timestamp.toLocaleDateString();
  };

  const getPipelineSummary = (pipeline: any[]) => {
    const stageTypes = pipeline.map(stage => stage.type || Object.keys(stage)[0]);
    return stageTypes.join(' → ');
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="p-4 border-b">
        <h3 className="font-semibold mb-3">Pipeline History</h3>
        
        {/* Search */}
        <div className="relative mb-3">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search pipelines..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="pl-9"
          />
        </div>

        {/* Collection Filter */}
        <select
          value={filterCollection}
          onChange={(e) => setFilterCollection(e.target.value)}
          className="w-full px-3 py-2 text-sm border rounded-md bg-background"
        >
          <option value="">All Collections</option>
          {collections.map(collection => (
            <option key={collection} value={collection}>
              {collection}
            </option>
          ))}
        </select>
      </div>

      {/* History List */}
      <ScrollArea className="flex-1">
        <div className="p-4 space-y-3">
          {filteredHistory.length === 0 ? (
            <div className="text-center text-muted-foreground py-8">
              {history.length === 0 ? (
                <>
                  <Clock className="h-8 w-8 mx-auto mb-2 opacity-50" />
                  <p className="text-sm">No pipeline history</p>
                  <p className="text-xs">Execute pipelines to see them here</p>
                </>
              ) : (
                <>
                  <Filter className="h-8 w-8 mx-auto mb-2 opacity-50" />
                  <p className="text-sm">No matching pipelines</p>
                  <p className="text-xs">Try adjusting your search or filter</p>
                </>
              )}
            </div>
          ) : (
            filteredHistory.map((item) => (
              <div
                key={item.id}
                className="border rounded-lg p-3 hover:bg-muted/30 cursor-pointer transition-colors"
                onClick={() => onLoadPipeline(item)}
              >
                {/* Header */}
                <div className="flex items-start justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <Database className="h-4 w-4 text-muted-foreground" />
                    <span className="font-medium text-sm">{item.collection}</span>
                    {item.error && (
                      <AlertCircle className="h-4 w-4 text-destructive" />
                    )}
                  </div>
                  <span className="text-xs text-muted-foreground">
                    {formatTimestamp(item.timestamp)}
                  </span>
                </div>

                {/* Pipeline Summary */}
                <div className="mb-2">
                  <div className="text-xs text-muted-foreground mb-1">
                    Pipeline ({item.pipeline.length} stages):
                  </div>
                  <div className="text-xs font-mono bg-muted/50 p-2 rounded">
                    {getPipelineSummary(item.pipeline)}
                  </div>
                </div>

                {/* Stats */}
                <div className="flex items-center gap-2 text-xs">
                  {item.error ? (
                    <Badge variant="destructive" className="text-xs">
                      Error
                    </Badge>
                  ) : (
                    <>
                      <Badge variant="secondary" className="text-xs">
                        {item.resultCount} docs
                      </Badge>
                      <Badge variant="outline" className="text-xs">
                        {item.executionTime}ms
                      </Badge>
                    </>
                  )}
                </div>

                {/* Error Message */}
                {item.error && (
                  <div className="mt-2 text-xs text-destructive bg-destructive/10 p-2 rounded">
                    {item.error}
                  </div>
                )}

                {/* Load Button */}
                <div className="mt-2 flex justify-end">
                  <Button
                    variant="ghost"
                    size="sm"
                    className="gap-2 text-xs"
                    onClick={(e) => {
                      e.stopPropagation();
                      onLoadPipeline(item);
                    }}
                  >
                    <Play className="h-3 w-3" />
                    Load Pipeline
                  </Button>
                </div>
              </div>
            ))
          )}
        </div>
      </ScrollArea>

      {/* Footer */}
      {history.length > 0 && (
        <div className="p-4 border-t">
          <Button
            variant="outline"
            size="sm"
            onClick={onClearHistory}
            className="w-full gap-2 text-destructive hover:text-destructive"
          >
            <Trash2 className="h-4 w-4" />
            Clear History
          </Button>
        </div>
      )}
    </div>
  );
}