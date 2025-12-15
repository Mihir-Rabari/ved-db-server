import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { 
  Plus,
  ChevronUp,
  ChevronDown,
  Trash2,
  Eye,
  EyeOff,
  Filter,
  Columns,
  Group,
  ArrowUpDown,
  Hash
} from 'lucide-react';
import { PipelineStage } from '../AggregationBuilder';
import { StageEditor } from './StageEditor';

interface PipelineStageBuilderProps {
  pipeline: PipelineStage[];
  onAddStage: (type: PipelineStage['type']) => void;
  onUpdateStage: (stageId: string, updates: Partial<PipelineStage>) => void;
  onRemoveStage: (stageId: string) => void;
  onMoveStage: (stageId: string, direction: 'up' | 'down') => void;
}

const stageTypes: Array<{
  type: PipelineStage['type'];
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  description: string;
}> = [
  {
    type: '$match',
    label: 'Match',
    icon: Filter,
    description: 'Filter documents'
  },
  {
    type: '$project',
    label: 'Project',
    icon: Columns,
    description: 'Select/transform fields'
  },
  {
    type: '$group',
    label: 'Group',
    icon: Group,
    description: 'Group and aggregate'
  },
  {
    type: '$sort',
    label: 'Sort',
    icon: ArrowUpDown,
    description: 'Sort documents'
  },
  {
    type: '$limit',
    label: 'Limit',
    icon: Hash,
    description: 'Limit result count'
  }
];

export function PipelineStageBuilder({
  pipeline,
  onAddStage,
  onUpdateStage,
  onRemoveStage,
  onMoveStage
}: PipelineStageBuilderProps) {
  const [showAddMenu, setShowAddMenu] = useState(false);

  const getStageIcon = (type: PipelineStage['type']) => {
    const stageType = stageTypes.find(st => st.type === type);
    return stageType?.icon || Filter;
  };

  const getStageLabel = (type: PipelineStage['type']) => {
    const stageType = stageTypes.find(st => st.type === type);
    return stageType?.label || type;
  };

  return (
    <div className="flex flex-col gap-4 h-full">
      {/* Add Stage Button */}
      <div className="relative">
        <Button
          onClick={() => setShowAddMenu(!showAddMenu)}
          className="w-full gap-2"
          variant="outline"
        >
          <Plus className="h-4 w-4" />
          Add Stage
        </Button>

        {/* Add Stage Menu */}
        {showAddMenu && (
          <div className="absolute top-full left-0 right-0 mt-2 bg-background border rounded-lg shadow-lg z-10">
            <div className="p-2">
              <div className="text-sm font-medium mb-2 px-2">Select Stage Type</div>
              {stageTypes.map((stageType) => {
                const Icon = stageType.icon;
                return (
                  <Button
                    key={stageType.type}
                    variant="ghost"
                    className="w-full justify-start h-auto p-2"
                    onClick={() => {
                      onAddStage(stageType.type);
                      setShowAddMenu(false);
                    }}
                  >
                    <div className="flex items-center gap-3 w-full">
                      <Icon className="h-4 w-4 text-muted-foreground" />
                      <div className="text-left">
                        <div className="font-medium">{stageType.label}</div>
                        <div className="text-xs text-muted-foreground">
                          {stageType.description}
                        </div>
                      </div>
                    </div>
                  </Button>
                );
              })}
            </div>
          </div>
        )}
      </div>

      {/* Pipeline Stages */}
      <div className="flex-1 overflow-auto space-y-3">
        {pipeline.length === 0 ? (
          <div className="flex-1 flex items-center justify-center text-center">
            <div className="text-muted-foreground">
              <Plus className="h-8 w-8 mx-auto mb-2 opacity-50" />
              <p className="text-sm">No stages in pipeline</p>
              <p className="text-xs">Add a stage to get started</p>
            </div>
          </div>
        ) : (
          pipeline.map((stage, index) => {
            const Icon = getStageIcon(stage.type);
            
            return (
              <Card key={stage.id} className={`${!stage.enabled ? 'opacity-60' : ''}`}>
                <CardHeader className="pb-2">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <Badge variant="outline" className="text-xs">
                        {index + 1}
                      </Badge>
                      <Icon className="h-4 w-4" />
                      <span className="font-medium">{getStageLabel(stage.type)}</span>
                      <Badge variant="secondary" className="text-xs">
                        {stage.type}
                      </Badge>
                    </div>
                    
                    <div className="flex items-center gap-1">
                      {/* Toggle enabled/disabled */}
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => onUpdateStage(stage.id, { enabled: !stage.enabled })}
                        className="h-8 w-8 p-0"
                      >
                        {stage.enabled ? (
                          <Eye className="h-4 w-4" />
                        ) : (
                          <EyeOff className="h-4 w-4" />
                        )}
                      </Button>
                      
                      {/* Move up */}
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => onMoveStage(stage.id, 'up')}
                        disabled={index === 0}
                        className="h-8 w-8 p-0"
                      >
                        <ChevronUp className="h-4 w-4" />
                      </Button>
                      
                      {/* Move down */}
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => onMoveStage(stage.id, 'down')}
                        disabled={index === pipeline.length - 1}
                        className="h-8 w-8 p-0"
                      >
                        <ChevronDown className="h-4 w-4" />
                      </Button>
                      
                      {/* Remove */}
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => onRemoveStage(stage.id)}
                        className="h-8 w-8 p-0 text-destructive hover:text-destructive"
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>
                  </div>
                </CardHeader>
                
                <CardContent className="pt-0">
                  <StageEditor
                    stage={stage}
                    onChange={(config) => onUpdateStage(stage.id, { config })}
                  />
                </CardContent>
              </Card>
            );
          })
        )}
      </div>

      {/* Pipeline Summary */}
      {pipeline.length > 0 && (
        <div className="border-t pt-3">
          <div className="text-sm text-muted-foreground">
            Pipeline: {pipeline.filter(s => s.enabled).length} active stages
            {pipeline.some(s => !s.enabled) && (
              <span className="ml-2">
                ({pipeline.filter(s => !s.enabled).length} disabled)
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}