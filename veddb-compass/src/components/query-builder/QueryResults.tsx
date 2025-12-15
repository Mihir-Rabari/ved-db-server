import { useState } from 'react';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { 
  ChevronDown, 
  ChevronRight, 
  Copy, 
  Download,
  AlertCircle,
  Clock,
  Database,
  Loader2,
  FileText,
  Eye,
  EyeOff
} from 'lucide-react';
import { QueryResult } from '../QueryBuilder';

interface QueryResultsProps {
  results: QueryResult | null;
  isLoading: boolean;
}

interface DocumentViewProps {
  document: any;
  index: number;
}

function DocumentView({ document, index }: DocumentViewProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const [showRaw, setShowRaw] = useState(false);

  const copyDocument = async () => {
    try {
      await navigator.clipboard.writeText(JSON.stringify(document, null, 2));
    } catch (error) {
      console.error('Failed to copy document:', error);
    }
  };

  const renderValue = (value: any, key?: string): React.ReactNode => {
    if (value === null) {
      return <span className="text-muted-foreground italic">null</span>;
    }
    
    if (value === undefined) {
      return <span className="text-muted-foreground italic">undefined</span>;
    }
    
    if (typeof value === 'boolean') {
      return <span className="text-blue-600 dark:text-blue-400">{value.toString()}</span>;
    }
    
    if (typeof value === 'number') {
      return <span className="text-green-600 dark:text-green-400">{value}</span>;
    }
    
    if (typeof value === 'string') {
      // Detect special string types
      if (key === '_id' || key?.endsWith('_id')) {
        return <span className="text-purple-600 dark:text-purple-400 font-mono text-sm">"{value}"</span>;
      }
      if (key?.includes('date') || key?.includes('time') || /^\d{4}-\d{2}-\d{2}/.test(value)) {
        return <span className="text-orange-600 dark:text-orange-400">"{value}"</span>;
      }
      if (key?.includes('email') && value.includes('@')) {
        return <span className="text-cyan-600 dark:text-cyan-400">"{value}"</span>;
      }
      return <span className="text-gray-700 dark:text-gray-300">"{value}"</span>;
    }
    
    if (Array.isArray(value)) {
      if (value.length === 0) {
        return <span className="text-muted-foreground">[]</span>;
      }
      return (
        <div className="ml-4">
          <span className="text-muted-foreground">[</span>
          {value.map((item, i) => (
            <div key={i} className="ml-4">
              <span className="text-muted-foreground">{i}:</span> {renderValue(item)}
              {i < value.length - 1 && <span className="text-muted-foreground">,</span>}
            </div>
          ))}
          <span className="text-muted-foreground">]</span>
        </div>
      );
    }
    
    if (typeof value === 'object') {
      const keys = Object.keys(value);
      if (keys.length === 0) {
        return <span className="text-muted-foreground">{'{}'}</span>;
      }
      return (
        <div className="ml-4">
          <span className="text-muted-foreground">{'{'}</span>
          {keys.map((k, i) => (
            <div key={k} className="ml-4">
              <span className="text-blue-700 dark:text-blue-300">"{k}"</span>
              <span className="text-muted-foreground">: </span>
              {renderValue(value[k], k)}
              {i < keys.length - 1 && <span className="text-muted-foreground">,</span>}
            </div>
          ))}
          <span className="text-muted-foreground">{'}'}</span>
        </div>
      );
    }
    
    return <span>{String(value)}</span>;
  };

  const renderDocumentPreview = (doc: any) => {
    const previewFields = ['_id', 'name', 'title', 'email', 'status', 'created_at'];
    const preview: any = {};
    
    previewFields.forEach(field => {
      if (doc[field] !== undefined) {
        preview[field] = doc[field];
      }
    });
    
    // If no preview fields found, show first 3 fields
    if (Object.keys(preview).length === 0) {
      const keys = Object.keys(doc).slice(0, 3);
      keys.forEach(key => {
        preview[key] = doc[key];
      });
    }
    
    return preview;
  };

  return (
    <div className="border rounded-lg">
      {/* Document Header */}
      <div className="flex items-center justify-between p-3 bg-muted/30 border-b">
        <div className="flex items-center gap-2">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setIsExpanded(!isExpanded)}
            className="h-6 w-6 p-0"
          >
            {isExpanded ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
          </Button>
          
          <span className="font-mono text-sm text-muted-foreground">
            Document #{index + 1}
          </span>
          
          {document._id && (
            <Badge variant="outline" className="text-xs font-mono">
              {document._id}
            </Badge>
          )}
        </div>
        
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setShowRaw(!showRaw)}
            className="h-6 px-2 text-xs"
          >
            {showRaw ? <EyeOff className="h-3 w-3" /> : <Eye className="h-3 w-3" />}
            {showRaw ? 'Pretty' : 'Raw'}
          </Button>
          
          <Button
            variant="ghost"
            size="sm"
            onClick={copyDocument}
            className="h-6 px-2 text-xs"
          >
            <Copy className="h-3 w-3" />
          </Button>
        </div>
      </div>
      
      {/* Document Content */}
      <div className="p-3">
        {!isExpanded ? (
          // Preview mode
          <div className="font-mono text-sm">
            {renderValue(renderDocumentPreview(document))}
          </div>
        ) : (
          // Full document view
          <div className="font-mono text-sm">
            {showRaw ? (
              <pre className="whitespace-pre-wrap text-xs bg-muted/50 p-3 rounded border overflow-x-auto">
                {JSON.stringify(document, null, 2)}
              </pre>
            ) : (
              renderValue(document)
            )}
          </div>
        )}
      </div>
    </div>
  );
}

export function QueryResults({ results, isLoading }: QueryResultsProps) {
  const [expandAll, setExpandAll] = useState(false);

  if (isLoading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <Loader2 className="h-8 w-8 animate-spin mx-auto mb-4 text-muted-foreground" />
          <p className="text-muted-foreground">Executing query...</p>
        </div>
      </div>
    );
  }

  if (!results) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <Database className="h-12 w-12 mx-auto mb-4 text-muted-foreground" />
          <h3 className="text-lg font-medium mb-2">No Query Executed</h3>
          <p className="text-muted-foreground">
            Build a query and click "Execute Query" to see results here.
          </p>
        </div>
      </div>
    );
  }

  if (results.error) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center max-w-md">
          <AlertCircle className="h-12 w-12 mx-auto mb-4 text-destructive" />
          <h3 className="text-lg font-medium mb-2">Query Error</h3>
          <p className="text-sm text-muted-foreground mb-4">
            {results.error}
          </p>
          <div className="text-xs text-muted-foreground">
            Execution time: {results.executionTime}ms
          </div>
        </div>
      </div>
    );
  }

  if (results.documents.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <FileText className="h-12 w-12 mx-auto mb-4 text-muted-foreground" />
          <h3 className="text-lg font-medium mb-2">No Results Found</h3>
          <p className="text-muted-foreground mb-4">
            Your query didn't match any documents.
          </p>
          <div className="text-xs text-muted-foreground">
            Execution time: {results.executionTime}ms
          </div>
        </div>
      </div>
    );
  }

  const exportResults = () => {
    const dataStr = JSON.stringify(results.documents, null, 2);
    const dataBlob = new Blob([dataStr], { type: 'application/json' });
    const url = URL.createObjectURL(dataBlob);
    
    const link = document.createElement('a');
    link.href = url;
    link.download = `veddb-results-${new Date().toISOString().split('T')[0]}.json`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
  };

  return (
    <div className="flex-1 flex flex-col">
      {/* Results Header */}
      <div className="flex items-center justify-between mb-4 pb-2 border-b">
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <Badge variant="secondary" className="gap-1">
              <Database className="h-3 w-3" />
              {results.documents.length} documents
            </Badge>
            
            <Badge variant="outline" className="gap-1 text-xs">
              <Clock className="h-3 w-3" />
              {results.executionTime}ms
            </Badge>
            
            {results.totalCount > results.documents.length && (
              <Badge variant="outline" className="text-xs">
                {results.totalCount} total
              </Badge>
            )}
          </div>
        </div>
        
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => setExpandAll(!expandAll)}
            className="text-xs"
          >
            {expandAll ? 'Collapse All' : 'Expand All'}
          </Button>
          
          <Button
            variant="outline"
            size="sm"
            onClick={exportResults}
            className="gap-1 text-xs"
          >
            <Download className="h-3 w-3" />
            Export
          </Button>
        </div>
      </div>

      {/* Results List */}
      <ScrollArea className="flex-1">
        <div className="space-y-2">
          {results.documents.map((document, index) => (
            <DocumentView
              key={document._id || index}
              document={document}
              index={index}
            />
          ))}
        </div>
      </ScrollArea>
    </div>
  );
}