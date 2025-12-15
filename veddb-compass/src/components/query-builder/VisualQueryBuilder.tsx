import React, { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Badge } from '@/components/ui/badge';
import { 
  Plus, 
  Trash2, 
  ChevronDown, 
  ChevronRight,
  Filter,
  Eye,
  ArrowUpDown,
  Hash
} from 'lucide-react';

interface VisualQueryBuilderProps {
  query: any;
  onChange: (query: any) => void;
  collection: string;
}

interface FilterCondition {
  id: string;
  field: string;
  operator: string;
  value: string;
  type: 'string' | 'number' | 'boolean' | 'date';
}

interface SortField {
  id: string;
  field: string;
  direction: 'asc' | 'desc';
}

const OPERATORS = [
  { value: '$eq', label: 'equals (=)', types: ['string', 'number', 'boolean', 'date'] },
  { value: '$ne', label: 'not equals (≠)', types: ['string', 'number', 'boolean', 'date'] },
  { value: '$gt', label: 'greater than (>)', types: ['number', 'date'] },
  { value: '$gte', label: 'greater than or equal (≥)', types: ['number', 'date'] },
  { value: '$lt', label: 'less than (<)', types: ['number', 'date'] },
  { value: '$lte', label: 'less than or equal (≤)', types: ['number', 'date'] },
  { value: '$in', label: 'in array', types: ['string', 'number'] },
  { value: '$nin', label: 'not in array', types: ['string', 'number'] },
  { value: '$exists', label: 'field exists', types: ['string', 'number', 'boolean', 'date'] },
  { value: '$regex', label: 'matches regex', types: ['string'] },
];

const COMMON_FIELDS = [
  { name: '_id', type: 'string' },
  { name: 'name', type: 'string' },
  { name: 'email', type: 'string' },
  { name: 'age', type: 'number' },
  { name: 'created_at', type: 'date' },
  { name: 'updated_at', type: 'date' },
  { name: 'active', type: 'boolean' },
  { name: 'count', type: 'number' },
  { name: 'status', type: 'string' },
  { name: 'tags', type: 'string' },
];

export function VisualQueryBuilder({ query, onChange }: VisualQueryBuilderProps) {
  const [expandedSections, setExpandedSections] = useState({
    filter: true,
    projection: false,
    sort: false,
    limit: false
  });

  // Parse current query into visual components
  const [filterConditions, setFilterConditions] = useState<FilterCondition[]>(() => {
    const conditions: FilterCondition[] = [];
    if (query.filter && typeof query.filter === 'object') {
      Object.entries(query.filter).forEach(([field, value], index) => {
        if (typeof value === 'object' && value !== null) {
          Object.entries(value).forEach(([op, val]) => {
            conditions.push({
              id: `${index}-${op}`,
              field,
              operator: op,
              value: String(val),
              type: inferType(val)
            });
          });
        } else {
          conditions.push({
            id: String(index),
            field,
            operator: '$eq',
            value: String(value),
            type: inferType(value)
          });
        }
      });
    }
    return conditions;
  });

  const [projectionFields, setProjectionFields] = useState<string[]>(() => {
    if (query.projection && typeof query.projection === 'object') {
      return Object.keys(query.projection).filter(key => query.projection[key] === 1);
    }
    return [];
  });

  const [sortFields, setSortFields] = useState<SortField[]>(() => {
    const sorts: SortField[] = [];
    if (query.sort && typeof query.sort === 'object') {
      Object.entries(query.sort).forEach(([field, direction], index) => {
        sorts.push({
          id: String(index),
          field,
          direction: direction === 1 ? 'asc' : 'desc'
        });
      });
    }
    return sorts;
  });

  const [limitValue, setLimitValue] = useState<number>(query.limit || 100);

  function inferType(value: any): 'string' | 'number' | 'boolean' | 'date' {
    if (typeof value === 'number') return 'number';
    if (typeof value === 'boolean') return 'boolean';
    if (typeof value === 'string') {
      // Try to detect date strings
      if (/^\d{4}-\d{2}-\d{2}/.test(value)) return 'date';
      return 'string';
    }
    return 'string';
  }

  function updateQuery() {
    const newQuery: any = { ...query };

    // Build filter
    const filter: any = {};
    filterConditions.forEach(condition => {
      if (condition.field && condition.operator && condition.value) {
        let value: any = condition.value;
        
        // Convert value based on type
        if (condition.type === 'number') {
          value = parseFloat(condition.value);
        } else if (condition.type === 'boolean') {
          value = condition.value.toLowerCase() === 'true';
        } else if (condition.operator === '$in' || condition.operator === '$nin') {
          value = condition.value.split(',').map(v => v.trim());
        } else if (condition.operator === '$exists') {
          value = condition.value.toLowerCase() === 'true';
        }

        if (condition.operator === '$eq') {
          filter[condition.field] = value;
        } else {
          if (!filter[condition.field]) {
            filter[condition.field] = {};
          }
          filter[condition.field][condition.operator] = value;
        }
      }
    });
    newQuery.filter = filter;

    // Build projection
    const projection: any = {};
    projectionFields.forEach(field => {
      if (field) projection[field] = 1;
    });
    newQuery.projection = Object.keys(projection).length > 0 ? projection : {};

    // Build sort
    const sort: any = {};
    sortFields.forEach(sortField => {
      if (sortField.field) {
        sort[sortField.field] = sortField.direction === 'asc' ? 1 : -1;
      }
    });
    newQuery.sort = sort;

    // Set limit
    newQuery.limit = limitValue;

    onChange(newQuery);
  }

  const addFilterCondition = () => {
    const newCondition: FilterCondition = {
      id: crypto.randomUUID(),
      field: '',
      operator: '$eq',
      value: '',
      type: 'string'
    };
    setFilterConditions([...filterConditions, newCondition]);
  };

  const removeFilterCondition = (id: string) => {
    setFilterConditions(filterConditions.filter(c => c.id !== id));
  };

  const updateFilterCondition = (id: string, updates: Partial<FilterCondition>) => {
    setFilterConditions(filterConditions.map(c => 
      c.id === id ? { ...c, ...updates } : c
    ));
  };

  const addProjectionField = () => {
    setProjectionFields([...projectionFields, '']);
  };

  const removeProjectionField = (index: number) => {
    setProjectionFields(projectionFields.filter((_, i) => i !== index));
  };

  const updateProjectionField = (index: number, value: string) => {
    const newFields = [...projectionFields];
    newFields[index] = value;
    setProjectionFields(newFields);
  };

  const addSortField = () => {
    const newSort: SortField = {
      id: crypto.randomUUID(),
      field: '',
      direction: 'asc'
    };
    setSortFields([...sortFields, newSort]);
  };

  const removeSortField = (id: string) => {
    setSortFields(sortFields.filter(s => s.id !== id));
  };

  const updateSortField = (id: string, updates: Partial<SortField>) => {
    setSortFields(sortFields.map(s => 
      s.id === id ? { ...s, ...updates } : s
    ));
  };

  const toggleSection = (section: keyof typeof expandedSections) => {
    setExpandedSections(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };

  // Update query whenever conditions change
  React.useEffect(() => {
    updateQuery();
  }, [filterConditions, projectionFields, sortFields, limitValue]);

  return (
    <ScrollArea className="h-full">
      <div className="space-y-4">
        {/* Filter Section */}
        <Card>
          <CardHeader 
            className="pb-3 cursor-pointer"
            onClick={() => toggleSection('filter')}
          >
            <CardTitle className="text-base flex items-center gap-2">
              {expandedSections.filter ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
              <Filter className="h-4 w-4" />
              Filter Conditions
              {filterConditions.length > 0 && (
                <Badge variant="secondary" className="text-xs">
                  {filterConditions.length}
                </Badge>
              )}
            </CardTitle>
          </CardHeader>
          
          {expandedSections.filter && (
            <CardContent className="space-y-3">
              {filterConditions.map((condition) => (
                <div key={condition.id} className="flex items-center gap-2 p-3 border rounded-lg">
                  <select
                    value={condition.field}
                    onChange={(e) => updateFilterCondition(condition.id, { field: e.target.value })}
                    className="px-2 py-1 text-sm border rounded bg-background min-w-[120px]"
                  >
                    <option value="">Select Field</option>
                    {COMMON_FIELDS.map(field => (
                      <option key={field.name} value={field.name}>
                        {field.name} ({field.type})
                      </option>
                    ))}
                  </select>

                  <select
                    value={condition.operator}
                    onChange={(e) => updateFilterCondition(condition.id, { operator: e.target.value })}
                    className="px-2 py-1 text-sm border rounded bg-background min-w-[140px]"
                  >
                    {OPERATORS
                      .filter(op => op.types.includes(condition.type))
                      .map(op => (
                        <option key={op.value} value={op.value}>
                          {op.label}
                        </option>
                      ))}
                  </select>

                  <Input
                    value={condition.value}
                    onChange={(e) => updateFilterCondition(condition.id, { value: e.target.value })}
                    placeholder="Value"
                    className="text-sm"
                  />

                  <select
                    value={condition.type}
                    onChange={(e) => updateFilterCondition(condition.id, { type: e.target.value as any })}
                    className="px-2 py-1 text-sm border rounded bg-background"
                  >
                    <option value="string">String</option>
                    <option value="number">Number</option>
                    <option value="boolean">Boolean</option>
                    <option value="date">Date</option>
                  </select>

                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => removeFilterCondition(condition.id)}
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              ))}

              <Button
                variant="outline"
                size="sm"
                onClick={addFilterCondition}
                className="gap-2"
              >
                <Plus className="h-4 w-4" />
                Add Condition
              </Button>
            </CardContent>
          )}
        </Card>

        {/* Projection Section */}
        <Card>
          <CardHeader 
            className="pb-3 cursor-pointer"
            onClick={() => toggleSection('projection')}
          >
            <CardTitle className="text-base flex items-center gap-2">
              {expandedSections.projection ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
              <Eye className="h-4 w-4" />
              Field Projection
              {projectionFields.length > 0 && (
                <Badge variant="secondary" className="text-xs">
                  {projectionFields.length}
                </Badge>
              )}
            </CardTitle>
          </CardHeader>
          
          {expandedSections.projection && (
            <CardContent className="space-y-3">
              {projectionFields.map((field, index) => (
                <div key={index} className="flex items-center gap-2">
                  <Input
                    value={field}
                    onChange={(e) => updateProjectionField(index, e.target.value)}
                    placeholder="Field name"
                    className="text-sm"
                  />
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => removeProjectionField(index)}
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              ))}

              <Button
                variant="outline"
                size="sm"
                onClick={addProjectionField}
                className="gap-2"
              >
                <Plus className="h-4 w-4" />
                Add Field
              </Button>
            </CardContent>
          )}
        </Card>

        {/* Sort Section */}
        <Card>
          <CardHeader 
            className="pb-3 cursor-pointer"
            onClick={() => toggleSection('sort')}
          >
            <CardTitle className="text-base flex items-center gap-2">
              {expandedSections.sort ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
              <ArrowUpDown className="h-4 w-4" />
              Sort Order
              {sortFields.length > 0 && (
                <Badge variant="secondary" className="text-xs">
                  {sortFields.length}
                </Badge>
              )}
            </CardTitle>
          </CardHeader>
          
          {expandedSections.sort && (
            <CardContent className="space-y-3">
              {sortFields.map((sortField) => (
                <div key={sortField.id} className="flex items-center gap-2">
                  <select
                    value={sortField.field}
                    onChange={(e) => updateSortField(sortField.id, { field: e.target.value })}
                    className="px-2 py-1 text-sm border rounded bg-background flex-1"
                  >
                    <option value="">Select Field</option>
                    {COMMON_FIELDS.map(field => (
                      <option key={field.name} value={field.name}>
                        {field.name}
                      </option>
                    ))}
                  </select>

                  <select
                    value={sortField.direction}
                    onChange={(e) => updateSortField(sortField.id, { direction: e.target.value as 'asc' | 'desc' })}
                    className="px-2 py-1 text-sm border rounded bg-background"
                  >
                    <option value="asc">Ascending</option>
                    <option value="desc">Descending</option>
                  </select>

                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => removeSortField(sortField.id)}
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              ))}

              <Button
                variant="outline"
                size="sm"
                onClick={addSortField}
                className="gap-2"
              >
                <Plus className="h-4 w-4" />
                Add Sort Field
              </Button>
            </CardContent>
          )}
        </Card>

        {/* Limit Section */}
        <Card>
          <CardHeader 
            className="pb-3 cursor-pointer"
            onClick={() => toggleSection('limit')}
          >
            <CardTitle className="text-base flex items-center gap-2">
              {expandedSections.limit ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
              <Hash className="h-4 w-4" />
              Result Limit
            </CardTitle>
          </CardHeader>
          
          {expandedSections.limit && (
            <CardContent>
              <div className="flex items-center gap-2">
                <Label htmlFor="limit" className="text-sm">
                  Maximum results:
                </Label>
                <Input
                  id="limit"
                  type="number"
                  value={limitValue}
                  onChange={(e) => setLimitValue(parseInt(e.target.value) || 100)}
                  min="1"
                  max="10000"
                  className="w-24 text-sm"
                />
              </div>
            </CardContent>
          )}
        </Card>
      </div>
    </ScrollArea>
  );
}