import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Card } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Plus, Minus, Code } from 'lucide-react';
import { PipelineStage } from '../AggregationBuilder';

interface StageEditorProps {
  stage: PipelineStage;
  onChange: (config: any) => void;
}

export function StageEditor({ stage, onChange }: StageEditorProps) {
  const [jsonMode, setJsonMode] = useState(false);
  const [jsonValue, setJsonValue] = useState(() => JSON.stringify(stage.config, null, 2));

  const handleJsonChange = (value: string) => {
    setJsonValue(value);
    try {
      const parsed = JSON.parse(value);
      onChange(parsed);
    } catch (error) {
      // Invalid JSON, don't update config
    }
  };

  const renderMatchEditor = () => {
    const config = stage.config || {};
    
    const addCondition = () => {
      const newConfig = { ...config };
      const fieldName = `field${Object.keys(newConfig).length + 1}`;
      newConfig[fieldName] = { $eq: '' };
      onChange(newConfig);
    };

    const removeCondition = (field: string) => {
      const newConfig = { ...config };
      delete newConfig[field];
      onChange(newConfig);
    };

    const updateCondition = (field: string, operator: string, value: any) => {
      const newConfig = { ...config };
      newConfig[field] = { [operator]: value };
      onChange(newConfig);
    };

    return (
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <Label className="text-sm font-medium">Match Conditions</Label>
          <Button
            variant="outline"
            size="sm"
            onClick={addCondition}
            className="gap-2"
          >
            <Plus className="h-3 w-3" />
            Add Condition
          </Button>
        </div>

        {Object.keys(config).length === 0 ? (
          <div className="text-sm text-muted-foreground text-center py-4">
            No conditions. Click "Add Condition" to filter documents.
          </div>
        ) : (
          <div className="space-y-2">
            {Object.entries(config).map(([field, condition]: [string, any]) => {
              const operator = Object.keys(condition)[0] || '$eq';
              const value = condition[operator];
              
              return (
                <Card key={field} className="p-3">
                  <div className="flex items-center gap-2">
                    <Input
                      placeholder="Field name"
                      value={field}
                      onChange={(e) => {
                        const newConfig = { ...config };
                        delete newConfig[field];
                        newConfig[e.target.value] = condition;
                        onChange(newConfig);
                      }}
                      className="flex-1"
                    />
                    
                    <select
                      value={operator}
                      onChange={(e) => updateCondition(field, e.target.value, value)}
                      className="px-2 py-1 text-sm border rounded"
                    >
                      <option value="$eq">equals</option>
                      <option value="$ne">not equals</option>
                      <option value="$gt">greater than</option>
                      <option value="$gte">greater than or equal</option>
                      <option value="$lt">less than</option>
                      <option value="$lte">less than or equal</option>
                      <option value="$in">in array</option>
                      <option value="$nin">not in array</option>
                      <option value="$exists">exists</option>
                      <option value="$regex">regex</option>
                    </select>
                    
                    <Input
                      placeholder="Value"
                      value={typeof value === 'string' ? value : JSON.stringify(value)}
                      onChange={(e) => {
                        let newValue = e.target.value;
                        // Try to parse as JSON for complex values
                        try {
                          if (newValue.startsWith('[') || newValue.startsWith('{')) {
                            newValue = JSON.parse(newValue);
                          }
                        } catch {
                          // Keep as string
                        }
                        updateCondition(field, operator, newValue);
                      }}
                      className="flex-1"
                    />
                    
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => removeCondition(field)}
                      className="text-destructive hover:text-destructive"
                    >
                      <Minus className="h-3 w-3" />
                    </Button>
                  </div>
                </Card>
              );
            })}
          </div>
        )}
      </div>
    );
  };

  const renderProjectEditor = () => {
    const config = stage.config || { _id: 1 };
    
    const addField = () => {
      const newConfig = { ...config };
      const fieldName = `field${Object.keys(newConfig).length + 1}`;
      newConfig[fieldName] = 1;
      onChange(newConfig);
    };

    const removeField = (field: string) => {
      const newConfig = { ...config };
      delete newConfig[field];
      onChange(newConfig);
    };

    const updateField = (oldField: string, newField: string, value: any) => {
      const newConfig = { ...config };
      delete newConfig[oldField];
      newConfig[newField] = value;
      onChange(newConfig);
    };

    return (
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <Label className="text-sm font-medium">Project Fields</Label>
          <Button
            variant="outline"
            size="sm"
            onClick={addField}
            className="gap-2"
          >
            <Plus className="h-3 w-3" />
            Add Field
          </Button>
        </div>

        <div className="space-y-2">
          {Object.entries(config).map(([field, value]: [string, any]) => (
            <Card key={field} className="p-3">
              <div className="flex items-center gap-2">
                <Input
                  placeholder="Field name"
                  value={field}
                  onChange={(e) => updateField(field, e.target.value, value)}
                  className="flex-1"
                />
                
                <select
                  value={typeof value === 'number' ? value : 'expression'}
                  onChange={(e) => {
                    const newValue = e.target.value === 'expression' ? '$field' : parseInt(e.target.value);
                    updateField(field, field, newValue);
                  }}
                  className="px-2 py-1 text-sm border rounded"
                >
                  <option value={1}>Include (1)</option>
                  <option value={0}>Exclude (0)</option>
                  <option value="expression">Expression</option>
                </select>
                
                {typeof value !== 'number' && (
                  <Input
                    placeholder="Expression (e.g., $field, $sum, etc.)"
                    value={typeof value === 'string' ? value : JSON.stringify(value)}
                    onChange={(e) => {
                      let newValue = e.target.value;
                      try {
                        if (newValue.startsWith('{')) {
                          newValue = JSON.parse(newValue);
                        }
                      } catch {
                        // Keep as string
                      }
                      updateField(field, field, newValue);
                    }}
                    className="flex-1"
                  />
                )}
                
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => removeField(field)}
                  className="text-destructive hover:text-destructive"
                >
                  <Minus className="h-3 w-3" />
                </Button>
              </div>
            </Card>
          ))}
        </div>
      </div>
    );
  };

  const renderGroupEditor = () => {
    const config = stage.config || { _id: null, count: { $sum: 1 } };
    
    const updateGroupBy = (value: string) => {
      const newConfig = { ...config };
      newConfig._id = value === 'null' ? null : value;
      onChange(newConfig);
    };

    const addAccumulator = () => {
      const newConfig = { ...config };
      const fieldName = `field${Object.keys(newConfig).length}`;
      newConfig[fieldName] = { $sum: 1 };
      onChange(newConfig);
    };

    const removeAccumulator = (field: string) => {
      if (field === '_id') return; // Don't allow removing _id
      const newConfig = { ...config };
      delete newConfig[field];
      onChange(newConfig);
    };

    const updateAccumulator = (field: string, operator: string, expression: string) => {
      const newConfig = { ...config };
      newConfig[field] = { [operator]: expression };
      onChange(newConfig);
    };

    return (
      <div className="space-y-4">
        {/* Group By */}
        <div>
          <Label className="text-sm font-medium">Group By</Label>
          <Input
            placeholder="Field to group by (e.g., $category, null for all)"
            value={config._id === null ? 'null' : config._id || ''}
            onChange={(e) => updateGroupBy(e.target.value)}
            className="mt-1"
          />
        </div>

        {/* Accumulators */}
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <Label className="text-sm font-medium">Accumulators</Label>
            <Button
              variant="outline"
              size="sm"
              onClick={addAccumulator}
              className="gap-2"
            >
              <Plus className="h-3 w-3" />
              Add Accumulator
            </Button>
          </div>

          <div className="space-y-2">
            {Object.entries(config).filter(([key]) => key !== '_id').map(([field, value]: [string, any]) => {
              const operator = Object.keys(value)[0] || '$sum';
              const expression = value[operator];
              
              return (
                <Card key={field} className="p-3">
                  <div className="flex items-center gap-2">
                    <Input
                      placeholder="Field name"
                      value={field}
                      onChange={(e) => {
                        const newConfig = { ...config };
                        delete newConfig[field];
                        newConfig[e.target.value] = value;
                        onChange(newConfig);
                      }}
                      className="flex-1"
                    />
                    
                    <select
                      value={operator}
                      onChange={(e) => updateAccumulator(field, e.target.value, expression)}
                      className="px-2 py-1 text-sm border rounded"
                    >
                      <option value="$sum">Sum</option>
                      <option value="$avg">Average</option>
                      <option value="$min">Minimum</option>
                      <option value="$max">Maximum</option>
                      <option value="$count">Count</option>
                      <option value="$push">Push</option>
                      <option value="$addToSet">Add to Set</option>
                    </select>
                    
                    <Input
                      placeholder="Expression (e.g., 1, $field)"
                      value={typeof expression === 'string' ? expression : JSON.stringify(expression)}
                      onChange={(e) => {
                        let newValue = e.target.value;
                        try {
                          if (newValue === '1' || newValue === '0') {
                            newValue = parseInt(newValue) as any;
                          }
                        } catch {
                          // Keep as string
                        }
                        updateAccumulator(field, operator, newValue);
                      }}
                      className="flex-1"
                    />
                    
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => removeAccumulator(field)}
                      className="text-destructive hover:text-destructive"
                    >
                      <Minus className="h-3 w-3" />
                    </Button>
                  </div>
                </Card>
              );
            })}
          </div>
        </div>
      </div>
    );
  };

  const renderSortEditor = () => {
    const config = stage.config || {};
    
    const addSortField = () => {
      const newConfig = { ...config };
      const fieldName = `field${Object.keys(newConfig).length + 1}`;
      newConfig[fieldName] = 1;
      onChange(newConfig);
    };

    const removeSortField = (field: string) => {
      const newConfig = { ...config };
      delete newConfig[field];
      onChange(newConfig);
    };

    const updateSortField = (oldField: string, newField: string, direction: number) => {
      const newConfig = { ...config };
      delete newConfig[oldField];
      newConfig[newField] = direction;
      onChange(newConfig);
    };

    return (
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <Label className="text-sm font-medium">Sort Fields</Label>
          <Button
            variant="outline"
            size="sm"
            onClick={addSortField}
            className="gap-2"
          >
            <Plus className="h-3 w-3" />
            Add Field
          </Button>
        </div>

        {Object.keys(config).length === 0 ? (
          <div className="text-sm text-muted-foreground text-center py-4">
            No sort fields. Click "Add Field" to sort results.
          </div>
        ) : (
          <div className="space-y-2">
            {Object.entries(config).map(([field, direction]: [string, any]) => (
              <Card key={field} className="p-3">
                <div className="flex items-center gap-2">
                  <Input
                    placeholder="Field name"
                    value={field}
                    onChange={(e) => updateSortField(field, e.target.value, direction)}
                    className="flex-1"
                  />
                  
                  <select
                    value={direction}
                    onChange={(e) => updateSortField(field, field, parseInt(e.target.value))}
                    className="px-2 py-1 text-sm border rounded"
                  >
                    <option value={1}>Ascending (1)</option>
                    <option value={-1}>Descending (-1)</option>
                  </select>
                  
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => removeSortField(field)}
                    className="text-destructive hover:text-destructive"
                  >
                    <Minus className="h-3 w-3" />
                  </Button>
                </div>
              </Card>
            ))}
          </div>
        )}
      </div>
    );
  };

  const renderLimitEditor = () => {
    const config = stage.config || 100;
    
    return (
      <div className="space-y-3">
        <Label className="text-sm font-medium">Limit Count</Label>
        <Input
          type="number"
          placeholder="Number of documents to return"
          value={config}
          onChange={(e) => onChange(parseInt(e.target.value) || 0)}
          min="0"
        />
        <div className="text-xs text-muted-foreground">
          Limits the number of documents returned by the pipeline.
        </div>
      </div>
    );
  };

  const renderVisualEditor = () => {
    switch (stage.type) {
      case '$match':
        return renderMatchEditor();
      case '$project':
        return renderProjectEditor();
      case '$group':
        return renderGroupEditor();
      case '$sort':
        return renderSortEditor();
      case '$limit':
        return renderLimitEditor();
      default:
        return <div>Unsupported stage type: {stage.type}</div>;
    }
  };

  return (
    <div className="space-y-3">
      <Tabs value={jsonMode ? 'json' : 'visual'} onValueChange={(value) => setJsonMode(value === 'json')}>
        <TabsList className="grid w-full grid-cols-2">
          <TabsTrigger value="visual">Visual Editor</TabsTrigger>
          <TabsTrigger value="json">JSON Editor</TabsTrigger>
        </TabsList>
        
        <TabsContent value="visual" className="mt-3">
          {renderVisualEditor()}
        </TabsContent>
        
        <TabsContent value="json" className="mt-3">
          <div className="space-y-2">
            <Label className="text-sm font-medium flex items-center gap-2">
              <Code className="h-4 w-4" />
              JSON Configuration
            </Label>
            <Textarea
              value={jsonValue}
              onChange={(e) => handleJsonChange(e.target.value)}
              placeholder="Enter JSON configuration..."
              className="font-mono text-sm min-h-[120px]"
            />
            <div className="text-xs text-muted-foreground">
              Edit the stage configuration as JSON. Changes are applied automatically.
            </div>
          </div>
        </TabsContent>
      </Tabs>
    </div>
  );
}