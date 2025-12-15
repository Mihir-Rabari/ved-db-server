import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { 
  Loader2, 
  AlertCircle, 
  FileText, 
  Table,
  ChevronRight,
  ChevronDown,
  Copy
} from 'lucide-react';
import { PipelineResult } from '../AggregationBuilder';

interface PipelineResultsProps {
  results: PipelineResult | null;
  isLoading: boolean;
}

export function PipelineResults({ results, isLoading }: PipelineResultsProps) {
  const [viewMode, setViewMode] = useState<'json' | 'table'>('json');
  const [expandedItems, setExpandedItems] = useState<Set<number>>(new Set());

  const toggleExpanded = (index: number) => {
    const newExpanded = new Set(expandedItems);
    if (newExpanded.has(index)) {
      newExpanded.delete(index);
    } else {
      newExpanded.add(index);
    }
    setExpandedItems(newExpanded);
  };

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
    } catch (error) {
      console.error('Failed to copy to clipboard:', error);
    }
  };

  const renderJsonView = () => {
    if (!results || results.documents.length === 0) {
      return (
        <div className="flex-1 flex items-center justify-center text-center">
          <div className="text-muted-foreground">
            <FileText className="h-8 w-8 mx-auto mb-2 opacity-50" />
            <p className="text-sm">No results to display</p>
          </div>
        </div>
      );
    }

    return (
      <ScrollArea className="flex-1">
        <div className="space-y-2 p-2">
          {results.documents.map((doc, index) => {
            const isExpanded = expandedItems.has(index);
            const jsonString = JSON.stringify(doc, null, 2);
            
            return (
              <div key={index} className="border rounded-lg">
                <div className="flex items-center justify-between p-3 bg-muted/30">
                  <div className="flex items-center gap-2">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => toggleExpanded(index)}
                      className="h-6 w-6 p-0"
                    >
                      {isExpanded ? (
                        <ChevronDown className="h-3 w-3" />
                      ) : (
                        <ChevronRight className="h-3 w-3" />
                      )}
                    </Button>
                    <span className="text-sm font-medium">Document {index + 1}</span>
                    <Badge variant="outline" className="text-xs">
                      {doc._id || 'No ID'}
                    </Badge>
                  </div>
                  
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => copyToClipboard(jsonString)}
                    className="h-6 w-6 p-0"
                  >
                    <Copy className="h-3 w-3" />
                  </Button>
                </div>
                
                {isExpanded && (
                  <div className="p-3 border-t">
                    <pre className="text-xs font-mono bg-muted/50 p-3 rounded overflow-x-auto">
                      {jsonString}
                    </pre>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </ScrollArea>
    );
  };

  const renderTableView = () => {
    if (!results || results.documents.length === 0) {
      return (
        <div className="flex-1 flex items-center justify-center text-center">
          <div className="text-muted-foreground">
            <Table className="h-8 w-8 mx-auto mb-2 opacity-50" />
            <p className="text-sm">No results to display</p>
          </div>
        </div>
      );
    }

    // Get all unique keys from all documents
    const allKeys = new Set<string>();
    results.documents.forEach(doc => {
      Object.keys(doc).forEach(key => allKeys.add(key));
    });
    const columns = Array.from(allKeys);

    return (
      <ScrollArea className="flex-1">
        <div className="min-w-full">
          <table className="w-full text-sm">
            <thead className="border-b bg-muted/30">
              <tr>
                {columns.map(column => (
                  <th key={column} className="text-left p-2 font-medium">
                    {column}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {results.documents.map((doc, index) => (
                <tr key={index} className="border-b hover:bg-muted/20">
                  {columns.map(column => {
                    const value = doc[column];
                    const displayValue = value === null || value === undefined 
                      ? <span className="text-muted-foreground italic">null</span>
                      : typeof value === 'object'
                      ? JSON.stringify(value)
                      : String(value);
                    
                    return (
                      <td key={column} className="p-2 max-w-xs truncate">
                        {displayValue}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </ScrollArea>
    );
  };

  if (isLoading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <Loader2 className="h-8 w-8 animate-spin mx-auto mb-2" />
          <p className="text-sm text-muted-foreground">Executing pipeline...</p>
        </div>
      </div>
    );
  }

  if (!results) {
    return (
      <div className="flex-1 flex items-center justify-center text-center">
        <div className="text-muted-foreground">
          <FileText className="h-8 w-8 mx-auto mb-2 opacity-50" />
          <p className="text-sm">Execute a pipeline to see results</p>
        </div>
      </div>
    );
  }

  if (results.error) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <AlertCircle className="h-8 w-8 text-destructive mx-auto mb-2" />
          <p className="text-sm font-medium text-destructive mb-1">Pipeline Error</p>
          <p className="text-xs text-muted-foreground max-w-md">
            {results.error}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Results Header */}
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium">Results</span>
          <Badge variant="secondary" className="text-xs">
            {results.documents.length} documents
          </Badge>
          <Badge variant="outline" className="text-xs">
            {results.executionTime}ms
          </Badge>
        </div>
        
        <Tabs value={viewMode} onValueChange={(value) => setViewMode(value as 'json' | 'table')}>
          <TabsList className="h-8">
            <TabsTrigger value="json" className="text-xs">JSON</TabsTrigger>
            <TabsTrigger value="table" className="text-xs">Table</TabsTrigger>
          </TabsList>
        </Tabs>
      </div>

      {/* Results Content */}
      <div className="flex-1 border rounded-lg overflow-hidden">
        {viewMode === 'json' ? renderJsonView() : renderTableView()}
      </div>

      {/* Results Footer */}
      {results.documents.length > 0 && (
        <div className="mt-2 text-xs text-muted-foreground">
          Showing {results.documents.length} of {results.totalCount} total documents
        </div>
      )}
    </div>
  );
}