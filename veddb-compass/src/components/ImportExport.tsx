import React, { useState } from 'react';
import { useConnectionStore } from '@/store';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Badge } from '@/components/ui/badge';
import { useToast } from '@/hooks/use-toast';
import { 
  Upload, 
  Download, 
  FileText, 
  Database, 
  AlertCircle, 
  CheckCircle,
  Loader2,
  FileJson,
  FileSpreadsheet
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/tauri';
import { open, save } from '@tauri-apps/api/dialog';
import { readTextFile, writeTextFile } from '@tauri-apps/api/fs';

interface ExportOptions {
  collection: string;
  format: 'json' | 'csv' | 'bson';
  query?: string;
  includeMetadata: boolean;
  prettyPrint: boolean;
}

interface ImportOptions {
  collection: string;
  format: 'json' | 'csv' | 'bson';
  file: string;
  mode: 'insert' | 'upsert' | 'replace';
  batchSize: number;
}

interface ImportProgress {
  processed: number;
  total: number;
  errors: string[];
  isComplete: boolean;
}

export function ImportExport() {
  const { activeConnection } = useConnectionStore();
  const { toast } = useToast();
  
  const [collections, setCollections] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [exportOptions, setExportOptions] = useState<ExportOptions>({
    collection: '',
    format: 'json',
    includeMetadata: true,
    prettyPrint: true,
  });
  
  const [importOptions, setImportOptions] = useState<ImportOptions>({
    collection: '',
    format: 'json',
    file: '',
    mode: 'insert',
    batchSize: 1000,
  });
  
  const [importProgress, setImportProgress] = useState<ImportProgress | null>(null);

  // Load collections on component mount
  React.useEffect(() => {
    if (activeConnection?.isConnected) {
      loadCollections();
    }
  }, [activeConnection]);

  const loadCollections = async () => {
    if (!activeConnection) return;
    
    try {
      const result = await invoke<Array<{ name: string }>>('get_collections', {
        connectionId: activeConnection.id,
      });
      setCollections(result.map(c => c.name));
    } catch (error) {
      toast({
        title: 'Error',
        description: `Failed to load collections: ${error}`,
        variant: 'destructive',
      });
    }
  };

  const handleExport = async () => {
    if (!activeConnection || !exportOptions.collection) {
      toast({
        title: 'Error',
        description: 'Please select a collection to export',
        variant: 'destructive',
      });
      return;
    }

    setIsLoading(true);
    
    try {
      // Choose save location
      const filePath = await save({
        defaultPath: `${exportOptions.collection}.${exportOptions.format}`,
        filters: [
          {
            name: `${exportOptions.format.toUpperCase()} Files`,
            extensions: [exportOptions.format],
          },
        ],
      });

      if (!filePath) {
        setIsLoading(false);
        return;
      }

      // Export data
      const result = await invoke<string>('export_collection', {
        connectionId: activeConnection.id,
        collection: exportOptions.collection,
        format: exportOptions.format,
        query: exportOptions.query || null,
        includeMetadata: exportOptions.includeMetadata,
        prettyPrint: exportOptions.prettyPrint,
      });

      // Write to file
      await writeTextFile(filePath, result);

      toast({
        title: 'Export Complete',
        description: `Successfully exported ${exportOptions.collection} to ${filePath}`,
      });
    } catch (error) {
      toast({
        title: 'Export Failed',
        description: `Failed to export collection: ${error}`,
        variant: 'destructive',
      });
    } finally {
      setIsLoading(false);
    }
  };

  const handleFileSelect = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: 'Data Files',
            extensions: ['json', 'csv', 'bson'],
          },
        ],
      });

      if (selected && typeof selected === 'string') {
        setImportOptions(prev => ({ ...prev, file: selected }));
        
        // Auto-detect format from file extension
        const extension = selected.split('.').pop()?.toLowerCase();
        if (extension && ['json', 'csv', 'bson'].includes(extension)) {
          setImportOptions(prev => ({ 
            ...prev, 
            format: extension as 'json' | 'csv' | 'bson' 
          }));
        }
      }
    } catch (error) {
      toast({
        title: 'Error',
        description: `Failed to select file: ${error}`,
        variant: 'destructive',
      });
    }
  };

  const handleImport = async () => {
    if (!activeConnection || !importOptions.collection || !importOptions.file) {
      toast({
        title: 'Error',
        description: 'Please select a collection and file to import',
        variant: 'destructive',
      });
      return;
    }

    setIsLoading(true);
    setImportProgress({
      processed: 0,
      total: 0,
      errors: [],
      isComplete: false,
    });

    try {
      // Read file content
      const fileContent = await readTextFile(importOptions.file);
      
      // Start import process
      const result = await invoke<ImportProgress>('import_collection', {
        connectionId: activeConnection.id,
        collection: importOptions.collection,
        format: importOptions.format,
        data: fileContent,
        mode: importOptions.mode,
        batchSize: importOptions.batchSize,
      });

      setImportProgress(result);

      if (result.errors.length === 0) {
        toast({
          title: 'Import Complete',
          description: `Successfully imported ${result.processed} documents`,
        });
      } else {
        toast({
          title: 'Import Complete with Errors',
          description: `Imported ${result.processed} documents with ${result.errors.length} errors`,
          variant: 'destructive',
        });
      }
    } catch (error) {
      toast({
        title: 'Import Failed',
        description: `Failed to import data: ${error}`,
        variant: 'destructive',
      });
      setImportProgress(null);
    } finally {
      setIsLoading(false);
    }
  };

  const getFormatIcon = (format: string) => {
    switch (format) {
      case 'json':
        return <FileJson className="h-4 w-4" />;
      case 'csv':
        return <FileSpreadsheet className="h-4 w-4" />;
      case 'bson':
        return <Database className="h-4 w-4" />;
      default:
        return <FileText className="h-4 w-4" />;
    }
  };

  if (!activeConnection?.isConnected) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <AlertCircle className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h3 className="text-lg font-semibold mb-2">No Connection</h3>
          <p className="text-muted-foreground">
            Connect to a VedDB server to access import/export functionality.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col">
      {/* Header */}
      <div className="border-b p-6">
        <div className="flex items-center gap-3">
          <div className="p-2 bg-primary/10 rounded-lg">
            <Upload className="h-6 w-6 text-primary" />
          </div>
          <div>
            <h1 className="text-2xl font-bold">Import & Export</h1>
            <p className="text-muted-foreground">
              Import and export data in JSON, CSV, and BSON formats
            </p>
          </div>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 p-6">
        <Tabs defaultValue="export" className="space-y-6">
          <TabsList className="grid w-full grid-cols-2">
            <TabsTrigger value="export" className="flex items-center gap-2">
              <Download className="h-4 w-4" />
              Export Data
            </TabsTrigger>
            <TabsTrigger value="import" className="flex items-center gap-2">
              <Upload className="h-4 w-4" />
              Import Data
            </TabsTrigger>
          </TabsList>

          {/* Export Tab */}
          <TabsContent value="export" className="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle>Export Collection</CardTitle>
                <CardDescription>
                  Export data from a collection to a file in your chosen format
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="grid grid-cols-2 gap-4">
                  <div className="space-y-2">
                    <Label htmlFor="export-collection">Collection</Label>
                    <Select
                      value={exportOptions.collection}
                      onValueChange={(value) =>
                        setExportOptions(prev => ({ ...prev, collection: value }))
                      }
                    >
                      <SelectTrigger>
                        <SelectValue placeholder="Select collection" />
                      </SelectTrigger>
                      <SelectContent>
                        {collections.map((collection) => (
                          <SelectItem key={collection} value={collection}>
                            {collection}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>

                  <div className="space-y-2">
                    <Label htmlFor="export-format">Format</Label>
                    <Select
                      value={exportOptions.format}
                      onValueChange={(value: 'json' | 'csv' | 'bson') =>
                        setExportOptions(prev => ({ ...prev, format: value }))
                      }
                    >
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="json">
                          <div className="flex items-center gap-2">
                            {getFormatIcon('json')}
                            JSON
                          </div>
                        </SelectItem>
                        <SelectItem value="csv">
                          <div className="flex items-center gap-2">
                            {getFormatIcon('csv')}
                            CSV
                          </div>
                        </SelectItem>
                        <SelectItem value="bson">
                          <div className="flex items-center gap-2">
                            {getFormatIcon('bson')}
                            BSON
                          </div>
                        </SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="export-query">Query Filter (Optional)</Label>
                  <Input
                    id="export-query"
                    placeholder='{"status": "active"}'
                    value={exportOptions.query || ''}
                    onChange={(e) =>
                      setExportOptions(prev => ({ ...prev, query: e.target.value }))
                    }
                  />
                  <p className="text-sm text-muted-foreground">
                    JSON query to filter documents (leave empty to export all)
                  </p>
                </div>

                <div className="flex items-center space-x-4">
                  <label className="flex items-center space-x-2">
                    <input
                      type="checkbox"
                      checked={exportOptions.includeMetadata}
                      onChange={(e) =>
                        setExportOptions(prev => ({ 
                          ...prev, 
                          includeMetadata: e.target.checked 
                        }))
                      }
                      className="rounded"
                    />
                    <span className="text-sm">Include metadata</span>
                  </label>

                  <label className="flex items-center space-x-2">
                    <input
                      type="checkbox"
                      checked={exportOptions.prettyPrint}
                      onChange={(e) =>
                        setExportOptions(prev => ({ 
                          ...prev, 
                          prettyPrint: e.target.checked 
                        }))
                      }
                      className="rounded"
                    />
                    <span className="text-sm">Pretty print JSON</span>
                  </label>
                </div>

                <Button
                  onClick={handleExport}
                  disabled={!exportOptions.collection || isLoading}
                  className="w-full"
                >
                  {isLoading ? (
                    <>
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                      Exporting...
                    </>
                  ) : (
                    <>
                      <Download className="mr-2 h-4 w-4" />
                      Export Collection
                    </>
                  )}
                </Button>
              </CardContent>
            </Card>
          </TabsContent>

          {/* Import Tab */}
          <TabsContent value="import" className="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle>Import Data</CardTitle>
                <CardDescription>
                  Import data from a file into a collection
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="grid grid-cols-2 gap-4">
                  <div className="space-y-2">
                    <Label htmlFor="import-collection">Target Collection</Label>
                    <Select
                      value={importOptions.collection}
                      onValueChange={(value) =>
                        setImportOptions(prev => ({ ...prev, collection: value }))
                      }
                    >
                      <SelectTrigger>
                        <SelectValue placeholder="Select collection" />
                      </SelectTrigger>
                      <SelectContent>
                        {collections.map((collection) => (
                          <SelectItem key={collection} value={collection}>
                            {collection}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>

                  <div className="space-y-2">
                    <Label htmlFor="import-mode">Import Mode</Label>
                    <Select
                      value={importOptions.mode}
                      onValueChange={(value: 'insert' | 'upsert' | 'replace') =>
                        setImportOptions(prev => ({ ...prev, mode: value }))
                      }
                    >
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="insert">Insert (fail on duplicates)</SelectItem>
                        <SelectItem value="upsert">Upsert (update or insert)</SelectItem>
                        <SelectItem value="replace">Replace (clear collection first)</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </div>

                <div className="space-y-2">
                  <Label>Select File</Label>
                  <div className="flex gap-2">
                    <Button
                      variant="outline"
                      onClick={handleFileSelect}
                      className="flex-1"
                    >
                      <Upload className="mr-2 h-4 w-4" />
                      Choose File
                    </Button>
                    {importOptions.format && (
                      <Badge variant="secondary" className="flex items-center gap-1">
                        {getFormatIcon(importOptions.format)}
                        {importOptions.format.toUpperCase()}
                      </Badge>
                    )}
                  </div>
                  {importOptions.file && (
                    <p className="text-sm text-muted-foreground">
                      Selected: {importOptions.file.split('/').pop()}
                    </p>
                  )}
                </div>

                <div className="space-y-2">
                  <Label htmlFor="batch-size">Batch Size</Label>
                  <Input
                    id="batch-size"
                    type="number"
                    min="1"
                    max="10000"
                    value={importOptions.batchSize}
                    onChange={(e) =>
                      setImportOptions(prev => ({ 
                        ...prev, 
                        batchSize: parseInt(e.target.value) || 1000 
                      }))
                    }
                  />
                  <p className="text-sm text-muted-foreground">
                    Number of documents to process in each batch
                  </p>
                </div>

                <Button
                  onClick={handleImport}
                  disabled={!importOptions.collection || !importOptions.file || isLoading}
                  className="w-full"
                >
                  {isLoading ? (
                    <>
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                      Importing...
                    </>
                  ) : (
                    <>
                      <Upload className="mr-2 h-4 w-4" />
                      Import Data
                    </>
                  )}
                </Button>

                {/* Import Progress */}
                {importProgress && (
                  <Card>
                    <CardHeader>
                      <CardTitle className="flex items-center gap-2">
                        {importProgress.isComplete ? (
                          <CheckCircle className="h-5 w-5 text-green-500" />
                        ) : (
                          <Loader2 className="h-5 w-5 animate-spin" />
                        )}
                        Import Progress
                      </CardTitle>
                    </CardHeader>
                    <CardContent>
                      <div className="space-y-2">
                        <div className="flex justify-between text-sm">
                          <span>Processed:</span>
                          <span>{importProgress.processed} / {importProgress.total}</span>
                        </div>
                        <div className="w-full bg-gray-200 rounded-full h-2">
                          <div
                            className="bg-primary h-2 rounded-full transition-all"
                            style={{
                              width: `${(importProgress.processed / importProgress.total) * 100}%`,
                            }}
                          />
                        </div>
                        {importProgress.errors.length > 0 && (
                          <div className="mt-4">
                            <h4 className="text-sm font-medium text-red-600 mb-2">
                              Errors ({importProgress.errors.length}):
                            </h4>
                            <div className="max-h-32 overflow-y-auto space-y-1">
                              {importProgress.errors.slice(0, 10).map((error, index) => (
                                <p key={index} className="text-xs text-red-600 bg-red-50 p-2 rounded">
                                  {error}
                                </p>
                              ))}
                              {importProgress.errors.length > 10 && (
                                <p className="text-xs text-muted-foreground">
                                  ... and {importProgress.errors.length - 10} more errors
                                </p>
                              )}
                            </div>
                          </div>
                        )}
                      </div>
                    </CardContent>
                  </Card>
                )}
              </CardContent>
            </Card>
          </TabsContent>
        </Tabs>
      </div>
    </div>
  );
}