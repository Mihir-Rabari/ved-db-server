import React, { useRef } from 'react';
import Editor from '@monaco-editor/react';
import { useThemeStore } from '@/store';
import { Badge } from '@/components/ui/badge';
import { AlertCircle, CheckCircle } from 'lucide-react';

interface JsonQueryEditorProps {
  value: string;
  onChange: (value: string) => void;
}

export function JsonQueryEditor({ value, onChange }: JsonQueryEditorProps) {
  const { theme } = useThemeStore();
  const editorRef = useRef<any>(null);
  const [isValidJson, setIsValidJson] = React.useState(true);
  const [jsonError, setJsonError] = React.useState<string>('');

  // Validate JSON
  const validateJson = (jsonString: string) => {
    try {
      JSON.parse(jsonString);
      setIsValidJson(true);
      setJsonError('');
      return true;
    } catch (error) {
      setIsValidJson(false);
      setJsonError(error instanceof Error ? error.message : 'Invalid JSON');
      return false;
    }
  };

  // Handle editor change
  const handleEditorChange = (newValue: string | undefined) => {
    if (newValue !== undefined) {
      validateJson(newValue);
      onChange(newValue);
    }
  };

  // Configure Monaco Editor
  const handleEditorDidMount = (editor: any, monaco: any) => {
    editorRef.current = editor;

    // Configure JSON schema for VedDB queries
    monaco.languages.json.jsonDefaults.setDiagnosticsOptions({
      validate: true,
      schemas: [{
        uri: "http://veddb.com/query-schema.json",
        fileMatch: ["*"],
        schema: {
          type: "object",
          properties: {
            filter: {
              type: "object",
              description: "Filter conditions for documents",
              additionalProperties: true
            },
            projection: {
              type: "object",
              description: "Fields to include/exclude in results",
              additionalProperties: {
                type: "number",
                enum: [0, 1]
              }
            },
            sort: {
              type: "object",
              description: "Sort order for results",
              additionalProperties: {
                type: "number",
                enum: [-1, 1]
              }
            },
            limit: {
              type: "number",
              description: "Maximum number of documents to return",
              minimum: 1,
              maximum: 10000
            },
            skip: {
              type: "number",
              description: "Number of documents to skip",
              minimum: 0
            }
          },
          additionalProperties: false
        }
      }]
    });

    // Add custom completions for VedDB operators
    monaco.languages.registerCompletionItemProvider('json', {
      provideCompletionItems: () => {
        const suggestions = [
          // Filter operators
          {
            label: '$eq',
            kind: monaco.languages.CompletionItemKind.Operator,
            insertText: '"$eq": ',
            documentation: 'Matches values that are equal to a specified value.'
          },
          {
            label: '$ne',
            kind: monaco.languages.CompletionItemKind.Operator,
            insertText: '"$ne": ',
            documentation: 'Matches all values that are not equal to a specified value.'
          },
          {
            label: '$gt',
            kind: monaco.languages.CompletionItemKind.Operator,
            insertText: '"$gt": ',
            documentation: 'Matches values that are greater than a specified value.'
          },
          {
            label: '$gte',
            kind: monaco.languages.CompletionItemKind.Operator,
            insertText: '"$gte": ',
            documentation: 'Matches values that are greater than or equal to a specified value.'
          },
          {
            label: '$lt',
            kind: monaco.languages.CompletionItemKind.Operator,
            insertText: '"$lt": ',
            documentation: 'Matches values that are less than a specified value.'
          },
          {
            label: '$lte',
            kind: monaco.languages.CompletionItemKind.Operator,
            insertText: '"$lte": ',
            documentation: 'Matches values that are less than or equal to a specified value.'
          },
          {
            label: '$in',
            kind: monaco.languages.CompletionItemKind.Operator,
            insertText: '"$in": []',
            documentation: 'Matches any of the values specified in an array.'
          },
          {
            label: '$nin',
            kind: monaco.languages.CompletionItemKind.Operator,
            insertText: '"$nin": []',
            documentation: 'Matches none of the values specified in an array.'
          },
          {
            label: '$exists',
            kind: monaco.languages.CompletionItemKind.Operator,
            insertText: '"$exists": true',
            documentation: 'Matches documents that have the specified field.'
          },
          {
            label: '$regex',
            kind: monaco.languages.CompletionItemKind.Operator,
            insertText: '"$regex": ""',
            documentation: 'Selects documents where values match a specified regular expression.'
          },
          {
            label: '$and',
            kind: monaco.languages.CompletionItemKind.Operator,
            insertText: '"$and": []',
            documentation: 'Joins query clauses with a logical AND.'
          },
          {
            label: '$or',
            kind: monaco.languages.CompletionItemKind.Operator,
            insertText: '"$or": []',
            documentation: 'Joins query clauses with a logical OR.'
          },
          {
            label: '$not',
            kind: monaco.languages.CompletionItemKind.Operator,
            insertText: '"$not": {}',
            documentation: 'Inverts the effect of a query expression.'
          },
          // Common field names
          {
            label: '_id',
            kind: monaco.languages.CompletionItemKind.Field,
            insertText: '"_id"',
            documentation: 'Document unique identifier'
          },
          {
            label: 'name',
            kind: monaco.languages.CompletionItemKind.Field,
            insertText: '"name"',
            documentation: 'Name field'
          },
          {
            label: 'email',
            kind: monaco.languages.CompletionItemKind.Field,
            insertText: '"email"',
            documentation: 'Email field'
          },
          {
            label: 'created_at',
            kind: monaco.languages.CompletionItemKind.Field,
            insertText: '"created_at"',
            documentation: 'Creation timestamp'
          },
          {
            label: 'updated_at',
            kind: monaco.languages.CompletionItemKind.Field,
            insertText: '"updated_at"',
            documentation: 'Last update timestamp'
          }
        ];

        return { suggestions };
      }
    });
  };

  // Get editor theme based on app theme
  const getEditorTheme = () => {
    if (theme === 'dark') return 'vs-dark';
    if (theme === 'light') return 'light';
    
    // System theme
    const systemTheme = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    return systemTheme === 'dark' ? 'vs-dark' : 'light';
  };

  // Format JSON
  const formatJson = () => {
    if (editorRef.current && isValidJson) {
      const formatted = JSON.stringify(JSON.parse(value), null, 2);
      editorRef.current.setValue(formatted);
      onChange(formatted);
    }
  };

  // Example queries for quick insertion
  const insertExample = (example: string) => {
    if (editorRef.current) {
      editorRef.current.setValue(example);
      onChange(example);
    }
  };

  const examples = [
    {
      name: 'Simple Filter',
      query: JSON.stringify({
        filter: { status: "active" },
        limit: 100
      }, null, 2)
    },
    {
      name: 'Range Query',
      query: JSON.stringify({
        filter: {
          age: { $gte: 18, $lt: 65 },
          status: "active"
        },
        sort: { created_at: -1 },
        limit: 50
      }, null, 2)
    },
    {
      name: 'Text Search',
      query: JSON.stringify({
        filter: {
          $or: [
            { name: { $regex: "john" } },
            { email: { $regex: "john" } }
          ]
        },
        projection: { name: 1, email: 1, created_at: 1 },
        limit: 20
      }, null, 2)
    },
    {
      name: 'Complex Query',
      query: JSON.stringify({
        filter: {
          $and: [
            { status: { $in: ["active", "pending"] } },
            { created_at: { $gte: "2024-01-01" } },
            { tags: { $exists: true } }
          ]
        },
        projection: { password: 0, internal_notes: 0 },
        sort: { priority: -1, created_at: 1 },
        limit: 100
      }, null, 2)
    }
  ];

  return (
    <div className="h-full flex flex-col">
      {/* Header with validation status and examples */}
      <div className="flex items-center justify-between mb-3 pb-2 border-b">
        <div className="flex items-center gap-2">
          {isValidJson ? (
            <Badge variant="secondary" className="gap-1 text-xs">
              <CheckCircle className="h-3 w-3" />
              Valid JSON
            </Badge>
          ) : (
            <Badge variant="destructive" className="gap-1 text-xs">
              <AlertCircle className="h-3 w-3" />
              Invalid JSON
            </Badge>
          )}
          
          {!isValidJson && (
            <span className="text-xs text-destructive">{jsonError}</span>
          )}
        </div>

        <div className="flex items-center gap-2">
          <select
            onChange={(e) => {
              if (e.target.value) {
                insertExample(e.target.value);
                e.target.value = '';
              }
            }}
            className="px-2 py-1 text-xs border rounded bg-background"
            defaultValue=""
          >
            <option value="">Insert Example...</option>
            {examples.map((example, index) => (
              <option key={index} value={example.query}>
                {example.name}
              </option>
            ))}
          </select>

          <button
            onClick={formatJson}
            disabled={!isValidJson}
            className="px-2 py-1 text-xs border rounded bg-background hover:bg-muted disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Format
          </button>
        </div>
      </div>

      {/* Monaco Editor */}
      <div className="flex-1 border rounded-md overflow-hidden">
        <Editor
          height="100%"
          defaultLanguage="json"
          value={value}
          onChange={handleEditorChange}
          onMount={handleEditorDidMount}
          theme={getEditorTheme()}
          options={{
            minimap: { enabled: false },
            fontSize: 14,
            lineNumbers: 'on',
            roundedSelection: false,
            scrollBeyondLastLine: false,
            automaticLayout: true,
            tabSize: 2,
            insertSpaces: true,
            wordWrap: 'on',
            folding: true,
            lineDecorationsWidth: 10,
            lineNumbersMinChars: 3,
            glyphMargin: false,
            contextmenu: true,
            selectOnLineNumbers: true,
            matchBrackets: 'always',
            autoIndent: 'full',
            formatOnPaste: true,
            formatOnType: true,
            suggest: {
              showKeywords: true,
              showSnippets: true,
              showFunctions: true
            }
          }}
        />
      </div>

      {/* Query hints */}
      <div className="mt-3 p-2 bg-muted/30 rounded text-xs text-muted-foreground">
        <div className="font-medium mb-1">Query Structure:</div>
        <div>• <code>filter</code>: Conditions to match documents</div>
        <div>• <code>projection</code>: Fields to include (1) or exclude (0)</div>
        <div>• <code>sort</code>: Sort order (1 = ascending, -1 = descending)</div>
        <div>• <code>limit</code>: Maximum number of results</div>
        <div>• <code>skip</code>: Number of documents to skip</div>
      </div>
    </div>
  );
}