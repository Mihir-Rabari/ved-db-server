import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryBuilder } from '../../QueryBuilder';
import { useConnectionStore } from '@/store';

// Mock the store
vi.mock('@/store', () => ({
  useConnectionStore: vi.fn(),
}));

// Mock Monaco Editor
vi.mock('@monaco-editor/react', () => ({
  default: ({ value, onChange }: { value: string; onChange: (value: string) => void }) => (
    <textarea
      data-testid="monaco-editor"
      value={value}
      onChange={(e) => onChange(e.target.value)}
    />
  ),
}));

// Mock toast hook
vi.mock('@/hooks/use-toast', () => ({
  useToast: () => ({
    toast: vi.fn(),
  }),
}));

describe('QueryBuilder', () => {
  const mockConnection = {
    id: 'test-connection',
    name: 'Test Connection',
    host: 'localhost',
    port: 50051,
    tls: false,
    isConnected: true,
  };

  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  it('should show no connection message when not connected', () => {
    (useConnectionStore as any).mockReturnValue({
      activeConnection: null,
    });

    render(<QueryBuilder />);
    
    expect(screen.getByText('No Connection')).toBeInTheDocument();
    expect(screen.getByText('Connect to a VedDB server to use the query builder.')).toBeInTheDocument();
  });

  it('should render query builder when connected', () => {
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection,
    });

    render(<QueryBuilder />);
    
    expect(screen.getByText('Query Builder')).toBeInTheDocument();
    expect(screen.getByText('Test Connection')).toBeInTheDocument();
    expect(screen.getByText('Execute Query')).toBeInTheDocument();
  });

  it('should have visual and JSON tabs', () => {
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection,
    });

    render(<QueryBuilder />);
    
    // Should have both tabs
    expect(screen.getByRole('tab', { name: 'Visual Builder' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'JSON Editor' })).toBeInTheDocument();
    
    // Should start with Visual Builder tab active
    expect(screen.getByRole('tab', { name: 'Visual Builder' })).toHaveAttribute('data-state', 'active');
  });

  it('should require collection selection before executing query', async () => {
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection,
    });

    render(<QueryBuilder />);
    
    const executeButton = screen.getByText('Execute Query');
    expect(executeButton).toBeDisabled();
    
    // Select a collection
    const collectionSelect = screen.getByDisplayValue('Select Collection');
    fireEvent.change(collectionSelect, { target: { value: 'users' } });
    
    expect(executeButton).not.toBeDisabled();
  });

  it('should show query history', () => {
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection,
    });

    render(<QueryBuilder />);
    
    const historyButton = screen.getByText(/History \(0\)/);
    fireEvent.click(historyButton);
    
    expect(screen.getByText('Query History')).toBeInTheDocument();
    expect(screen.getByText('No Query History')).toBeInTheDocument();
  });

  it('should execute query and show results', async () => {
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection,
    });

    render(<QueryBuilder />);
    
    // Select collection
    const collectionSelect = screen.getByDisplayValue('Select Collection');
    fireEvent.change(collectionSelect, { target: { value: 'users' } });
    
    // Execute query
    const executeButton = screen.getByText('Execute Query');
    fireEvent.click(executeButton);
    
    // Should show loading state
    expect(screen.getByText('Executing...')).toBeInTheDocument();
    
    // Wait for results
    await waitFor(() => {
      expect(screen.getByText('Results')).toBeInTheDocument();
    }, { timeout: 2000 });
  });

  it('should save and load query history', async () => {
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection,
    });

    render(<QueryBuilder />);
    
    // Select collection and execute query
    const collectionSelect = screen.getByDisplayValue('Select Collection');
    fireEvent.change(collectionSelect, { target: { value: 'users' } });
    
    const executeButton = screen.getByText('Execute Query');
    fireEvent.click(executeButton);
    
    // Wait for query to complete
    await waitFor(() => {
      expect(screen.getByText(/History \(1\)/)).toBeInTheDocument();
    }, { timeout: 2000 });
    
    // Open history
    const historyButton = screen.getByText(/History \(1\)/);
    fireEvent.click(historyButton);
    
    // Should show the executed query in history - look for the badge specifically
    expect(screen.getAllByText('users')).toHaveLength(3); // Collection select, badge, and history
  });

  it('should copy query to clipboard', async () => {
    // Mock clipboard API
    const mockWriteText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, {
      clipboard: {
        writeText: mockWriteText,
      },
    });

    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection,
    });

    render(<QueryBuilder />);
    
    const copyButton = screen.getByText('Copy Query');
    fireEvent.click(copyButton);
    
    expect(mockWriteText).toHaveBeenCalledWith(
      expect.stringContaining('"filter": {}')
    );
  });
});