import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { AggregationBuilder } from '../../AggregationBuilder';
import { useConnectionStore } from '@/store';

// Mock the store
vi.mock('@/store', () => ({
  useConnectionStore: vi.fn()
}));

// Mock the toast hook
vi.mock('@/hooks/use-toast', () => ({
  useToast: () => ({
    toast: vi.fn()
  })
}));

// Mock localStorage
const localStorageMock = {
  getItem: vi.fn(),
  setItem: vi.fn(),
  removeItem: vi.fn(),
  clear: vi.fn(),
};
Object.defineProperty(window, 'localStorage', {
  value: localStorageMock
});

describe('AggregationBuilder', () => {
  const mockConnection = {
    id: 'test-connection',
    name: 'Test Connection',
    host: 'localhost',
    port: 50051,
    tls: false,
    isConnected: true
  };

  beforeEach(() => {
    vi.clearAllMocks();
    localStorageMock.getItem.mockReturnValue(null);
  });

  it('renders no connection message when not connected', () => {
    (useConnectionStore as any).mockReturnValue({
      activeConnection: null
    });

    render(<AggregationBuilder />);
    
    expect(screen.getByText('No Connection')).toBeInTheDocument();
    expect(screen.getByText('Connect to a VedDB server to use the aggregation pipeline builder.')).toBeInTheDocument();
  });

  it('renders aggregation builder when connected', () => {
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection
    });

    render(<AggregationBuilder />);
    
    expect(screen.getByText('Aggregation Pipeline')).toBeInTheDocument();
    expect(screen.getByText('Pipeline Stages')).toBeInTheDocument();
    expect(screen.getByText('Results')).toBeInTheDocument();
    expect(screen.getByText('Add Stage')).toBeInTheDocument();
  });

  it('allows adding pipeline stages', async () => {
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection
    });

    render(<AggregationBuilder />);
    
    // Click Add Stage button
    const addStageButton = screen.getByText('Add Stage');
    fireEvent.click(addStageButton);
    
    // Should show stage type menu
    await waitFor(() => {
      expect(screen.getByText('Select Stage Type')).toBeInTheDocument();
    });
    
    // Click on Match stage
    const matchStage = screen.getByText('Match');
    fireEvent.click(matchStage);
    
    // Should add a match stage to the pipeline
    await waitFor(() => {
      expect(screen.getByText('$match')).toBeInTheDocument();
    });
  });

  it('shows execute button disabled when no collection selected', () => {
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection
    });

    render(<AggregationBuilder />);
    
    const executeButton = screen.getByText('Execute Pipeline');
    expect(executeButton).toBeDisabled();
  });

  it('enables execute button when collection is selected and pipeline has stages', async () => {
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection
    });

    render(<AggregationBuilder />);
    
    // Select a collection
    const collectionSelect = screen.getByDisplayValue('Select Collection');
    fireEvent.change(collectionSelect, { target: { value: 'users' } });
    
    // Add a stage
    const addStageButton = screen.getByText('Add Stage');
    fireEvent.click(addStageButton);
    
    await waitFor(() => {
      const matchStage = screen.getByText('Match');
      fireEvent.click(matchStage);
    });
    
    // Execute button should now be enabled
    await waitFor(() => {
      const executeButton = screen.getByText('Execute Pipeline');
      expect(executeButton).not.toBeDisabled();
    });
  });

  it('displays pipeline history', () => {
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection
    });

    render(<AggregationBuilder />);
    
    // Click history button
    const historyButton = screen.getByText(/History \(0\)/);
    fireEvent.click(historyButton);
    
    // Should show history sidebar
    expect(screen.getByText('Pipeline History')).toBeInTheDocument();
  });

  it('supports copying pipeline to clipboard', async () => {
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection
    });

    // Mock clipboard API
    const mockWriteText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, {
      clipboard: {
        writeText: mockWriteText
      }
    });

    render(<AggregationBuilder />);
    
    // Add a stage first
    const addStageButton = screen.getByText('Add Stage');
    fireEvent.click(addStageButton);
    
    await waitFor(() => {
      const matchStage = screen.getByText('Match');
      fireEvent.click(matchStage);
    });
    
    // Click copy pipeline button
    await waitFor(() => {
      const copyButton = screen.getByText('Copy Pipeline');
      fireEvent.click(copyButton);
    });
    
    // Should have called clipboard API
    expect(mockWriteText).toHaveBeenCalled();
  });
});