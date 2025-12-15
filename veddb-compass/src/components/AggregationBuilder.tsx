import { useState, useEffect } from 'react';
import { useConnectionStore } from '@/store';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { useToast } from '@/hooks/use-toast';
import { 
  Play, 
  History, 
  Download, 
  Copy,
  Database,
  FileText,
  Trash2
} from 'lucide-react';
import { PipelineStageBuilder } from './aggregation/PipelineStageBuilder';
import { PipelineResults } from './aggregation/PipelineResults';
import { PipelineHistory } from './aggregation/PipelineHistory';

export interface PipelineStage {
  id: string;
  type: '$match' | '$project' | '$group' | '$sort' | '$limit';
  config: any;
  enabled: boolean;
}

export interface PipelineHistoryItem {
  id: string;
  pipeline: PipelineStage[];
  collection: string;
  timestamp: Date;
  executionTime?: number;
  resultCount?: number;
  error?: string;
}

export interface PipelineResult {
  documents: any[];
  totalCount: number;
  executionTime: number;
  error?: string;
}

export function AggregationBuilder() {
  const { activeConnection } = useConnectionStore();
  const { toast } = useToast();
  
  // Pipeline state
  const [selectedCollection, setSelectedCollection] = useState<string>('');
  const [pipeline, setPipeline] = useState<PipelineStage[]>([]);
  
  // Results and history
  const [pipelineResults, setPipelineResults] = useState<PipelineResult | null>(null);
  const [isExecuting, setIsExecuting] = useState(false);
  const [pipelineHistory, setPipelineHistory] = useState<PipelineHistoryItem[]>([]);
  const [showHistory, setShowHistory] = useState(false);
  
  // Collections list (mock data for now)
  const collections = ['users', 'products', 'orders', 'sessions', 'logs'];

  // Load pipeline history from localStorage
  useEffect(() => {
    const savedHistory = localStorage.getItem('veddb-pipeline-history');
    if (savedHistory) {
      try {
        const parsed = JSON.parse(savedHistory);
        setPipelineHistory(parsed.map((item: any) => ({
          ...item,
          timestamp: new Date(item.timestamp)
        })));
      } catch (error) {
        console.error('Failed to load pipeline history:', error);
      }
    }
  }, []);

  // Save pipeline history to localStorage
  const savePipelineHistory = (newHistory: PipelineHistoryItem[]) => {
    setPipelineHistory(newHistory);
    localStorage.setItem('veddb-pipeline-history', JSON.stringify(newHistory));
  };

  // Add new pipeline stage
  const addStage = (type: PipelineStage['type']) => {
    const newStage: PipelineStage = {
      id: crypto.randomUUID(),
      type,
      config: getDefaultConfig(type),
      enabled: true
    };
    setPipeline([...pipeline, newStage]);
  };

  // Get default configuration for stage type
  const getDefaultConfig = (type: PipelineStage['type']) => {
    switch (type) {
      case '$match':
        return {};
      case '$project':
        return { _id: 1 };
      case '$group':
        return { _id: null, count: { $sum: 1 } };
      case '$sort':
        return { _id: 1 };
      case '$limit':
        return 100;
      default:
        return {};
    }
  };

  // Update pipeline stage
  const updateStage = (stageId: string, updates: Partial<PipelineStage>) => {
    setPipeline(pipeline.map(stage => 
      stage.id === stageId ? { ...stage, ...updates } : stage
    ));
  };

  // Remove pipeline stage
  const removeStage = (stageId: string) => {
    setPipeline(pipeline.filter(stage => stage.id !== stageId));
  };

  // Move stage up/down
  const moveStage = (stageId: string, direction: 'up' | 'down') => {
    const currentIndex = pipeline.findIndex(stage => stage.id === stageId);
    if (currentIndex === -1) return;

    const newIndex = direction === 'up' ? currentIndex - 1 : currentIndex + 1;
    if (newIndex < 0 || newIndex >= pipeline.length) return;

    const newPipeline = [...pipeline];
    [newPipeline[currentIndex], newPipeline[newIndex]] = [newPipeline[newIndex], newPipeline[currentIndex]];
    setPipeline(newPipeline);
  };

  // Execute pipeline
  const executePipeline = async () => {
    if (!selectedCollection) {
      toast({
        title: "No Collection Selected",
        description: "Please select a collection to run the aggregation pipeline.",
        variant: "destructive",
      });
      return;
    }

    if (pipeline.length === 0) {
      toast({
        title: "Empty Pipeline",
        description: "Please add at least one stage to the pipeline.",
        variant: "destructive",
      });
      return;
    }

    setIsExecuting(true);
    const startTime = Date.now();
    
    try {
      // Filter enabled stages
      const enabledStages = pipeline.filter(stage => stage.enabled);
      
      // Mock pipeline execution (replace with actual VedDB API call)
      await new Promise(resolve => setTimeout(resolve, 800 + Math.random() * 1200));
      
      // Mock results based on pipeline stages
      const mockResults = generateMockResults(enabledStages);
      const executionTime = Date.now() - startTime;

      const results = {
        ...mockResults,
        executionTime
      };

      setPipelineResults(results);

      // Add to history
      const historyItem: PipelineHistoryItem = {
        id: crypto.randomUUID(),
        pipeline: enabledStages,
        collection: selectedCollection,
        timestamp: new Date(),
        executionTime,
        resultCount: results.documents.length
      };

      const newHistory = [historyItem, ...pipelineHistory].slice(0, 50); // Keep last 50 pipelines
      savePipelineHistory(newHistory);

      toast({
        title: "Pipeline Executed",
        description: `Processed ${results.documents.length} documents in ${executionTime}ms`,
      });

    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error occurred';
      
      setPipelineResults({
        documents: [],
        totalCount: 0,
        executionTime: Date.now() - startTime,
        error: errorMessage
      });

      // Add error to history
      const historyItem: PipelineHistoryItem = {
        id: crypto.randomUUID(),
        pipeline: pipeline.filter(stage => stage.enabled),
        collection: selectedCollection,
        timestamp: new Date(),
        executionTime: Date.now() - startTime,
        error: errorMessage
      };

      const newHistory = [historyItem, ...pipelineHistory].slice(0, 50);
      savePipelineHistory(newHistory);

      toast({
        title: "Pipeline Failed",
        description: errorMessage,
        variant: "destructive",
      });
    } finally {
      setIsExecuting(false);
    }
  };

  // Generate mock results based on pipeline stages
  const generateMockResults = (stages: PipelineStage[]) => {
    let resultCount = Math.floor(Math.random() * 100) + 10;
    
    // Adjust result count based on stages
    const hasLimit = stages.find(s => s.type === '$limit');
    if (hasLimit && typeof hasLimit.config === 'number') {
      resultCount = Math.min(resultCount, hasLimit.config);
    }

    const hasGroup = stages.find(s => s.type === '$group');
    if (hasGroup) {
      resultCount = Math.floor(resultCount / 5); // Grouping typically reduces results
    }

    const documents = Array.from({ length: resultCount }, (_, i) => {
      if (hasGroup) {
        return {
          _id: `group_${i + 1}`,
          count: Math.floor(Math.random() * 50) + 1,
          total: Math.floor(Math.random() * 10000) + 100
        };
      }
      
      return {
        _id: `doc_${i + 1}`,
        name: `Document ${i + 1}`,
        value: Math.floor(Math.random() * 1000),
        category: ['A', 'B', 'C'][Math.floor(Math.random() * 3)],
        created_at: new Date(Date.now() - Math.random() * 86400000 * 30).toISOString()
      };
    });

    return {
      documents,
      totalCount: Math.floor(Math.random() * 1000) + resultCount
    };
  };

  // Load pipeline from history
  const loadPipelineFromHistory = (historyItem: PipelineHistoryItem) => {
    setPipeline(historyItem.pipeline.map(stage => ({
      ...stage,
      id: crypto.randomUUID() // Generate new IDs
    })));
    setSelectedCollection(historyItem.collection);
    setShowHistory(false);
    
    toast({
      title: "Pipeline Loaded",
      description: "Pipeline loaded from history",
    });
  };

  // Clear pipeline history
  const clearHistory = () => {
    setPipelineHistory([]);
    localStorage.removeItem('veddb-pipeline-history');
    toast({
      title: "History Cleared",
      description: "Pipeline history has been cleared",
    });
  };

  // Export results
  const exportResults = () => {
    if (!pipelineResults || pipelineResults.documents.length === 0) {
      toast({
        title: "No Results",
        description: "No pipeline results to export",
        variant: "destructive",
      });
      return;
    }

    const dataStr = JSON.stringify(pipelineResults.documents, null, 2);
    const dataBlob = new Blob([dataStr], { type: 'application/json' });
    const url = URL.createObjectURL(dataBlob);
    
    const link = document.createElement('a');
    link.href = url;
    link.download = `veddb-aggregation-results-${selectedCollection}-${new Date().toISOString().split('T')[0]}.json`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);

    toast({
      title: "Results Exported",
      description: "Aggregation results exported to JSON file",
    });
  };

  // Copy pipeline to clipboard
  const copyPipeline = async () => {
    const pipelineJson = JSON.stringify(
      pipeline.filter(stage => stage.enabled).map(stage => ({
        [`${stage.type}`]: stage.config
      })),
      null,
      2
    );
    
    try {
      await navigator.clipboard.writeText(pipelineJson);
      toast({
        title: "Pipeline Copied",
        description: "Pipeline copied to clipboard",
      });
    } catch (error) {
      toast({
        title: "Copy Failed",
        description: "Failed to copy pipeline to clipboard",
        variant: "destructive",
      });
    }
  };

  // Clear pipeline
  const clearPipeline = () => {
    setPipeline([]);
    setPipelineResults(null);
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
              Connect to a VedDB server to use the aggregation pipeline builder.
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
            <h1 className="text-2xl font-semibold">Aggregation Pipeline</h1>
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
              History ({pipelineHistory.length})
            </Button>
            
            <Button
              variant="outline"
              size="sm"
              onClick={copyPipeline}
              className="gap-2"
              disabled={pipeline.length === 0}
            >
              <Copy className="h-4 w-4" />
              Copy Pipeline
            </Button>

            <Button
              variant="outline"
              size="sm"
              onClick={clearPipeline}
              className="gap-2"
              disabled={pipeline.length === 0}
            >
              <Trash2 className="h-4 w-4" />
              Clear
            </Button>
            
            {pipelineResults && (
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
              onClick={executePipeline}
              disabled={isExecuting || !selectedCollection || pipeline.length === 0}
              className="gap-2"
            >
              <Play className="h-4 w-4" />
              {isExecuting ? 'Executing...' : 'Execute Pipeline'}
            </Button>
          </div>
        </div>
      </div>

      <div className="flex-1 flex">
        {/* Pipeline History Sidebar */}
        {showHistory && (
          <div className="w-80 border-r bg-muted/30">
            <PipelineHistory
              history={pipelineHistory}
              onLoadPipeline={loadPipelineFromHistory}
              onClearHistory={clearHistory}
            />
          </div>
        )}

        {/* Main Content */}
        <div className="flex-1 flex flex-col">
          {/* Pipeline Builder */}
          <div className="flex-1 p-4">
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 h-full">
              {/* Pipeline Stages */}
              <Card className="flex flex-col">
                <CardHeader className="pb-3">
                  <div className="flex items-center justify-between">
                    <CardTitle className="text-lg">Pipeline Stages</CardTitle>
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
                  <PipelineStageBuilder
                    pipeline={pipeline}
                    onAddStage={addStage}
                    onUpdateStage={updateStage}
                    onRemoveStage={removeStage}
                    onMoveStage={moveStage}
                  />
                </CardContent>
              </Card>

              {/* Pipeline Results */}
              <Card className="flex flex-col">
                <CardHeader className="pb-3">
                  <CardTitle className="text-lg flex items-center gap-2">
                    <FileText className="h-5 w-5" />
                    Results
                    {pipelineResults && (
                      <Badge variant="secondary" className="text-xs">
                        {pipelineResults.documents.length} docs
                      </Badge>
                    )}
                  </CardTitle>
                </CardHeader>
                
                <CardContent className="flex-1 flex flex-col">
                  <PipelineResults
                    results={pipelineResults}
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