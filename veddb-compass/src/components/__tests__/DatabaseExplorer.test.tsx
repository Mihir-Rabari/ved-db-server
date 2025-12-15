import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { DatabaseExplorer } from '../DatabaseExplorer';
import { useConnectionStore } from '@/store';

// Mock the store
vi.mock('@/store', () => ({
  useConnectionStore: vi.fn(),
}));

// Mock Tauri API
vi.mock('@tauri-apps/api/tauri', () => ({
  invoke: vi.fn().mockResolvedValue([]),
}));

// Mock toast hook
vi.mock('@/hooks/use-toast', () => ({
  useToast: () => ({
    toast: vi.fn(),
  }),
}));

describe('DatabaseExplorer', () => {
  const mockUseConnectionStore = vi.mocked(useConnectionStore);

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should show no connection message when not connected', () => {
    mockUseConnectionStore.mockReturnValue({
      activeConnection: null,
    });

    render(<DatabaseExplorer />);

    expect(screen.getByText('Database Explorer')).toBeInTheDocument();
    expect(screen.getByText('Connect to a VedDB server to explore your databases and collections.')).toBeInTheDocument();
  });

  it('should show database overview when connected but no collection selected', async () => {
    mockUseConnectionStore.mockReturnValue({
      activeConnection: {
        id: 'test-connection',
        isConnected: true,
        name: 'Test Connection',
      },
    });

    render(<DatabaseExplorer />);

    await waitFor(() => {
      expect(screen.getByText('Database Overview')).toBeInTheDocument();
    });
    
    expect(screen.getByText('VedDB v0.2.0 Hybrid Storage Architecture - Select a collection to explore its schema and performance')).toBeInTheDocument();
    expect(screen.getAllByText('Collections')[0]).toBeInTheDocument(); // Use getAllByText since there are multiple
    expect(screen.getByText('Documents')).toBeInTheDocument();
    expect(screen.getByText('Storage')).toBeInTheDocument();
  });

  it('should show hybrid storage architecture information', async () => {
    mockUseConnectionStore.mockReturnValue({
      activeConnection: {
        id: 'test-connection',
        isConnected: true,
        name: 'Test Connection',
      },
    });

    render(<DatabaseExplorer />);

    await waitFor(() => {
      expect(screen.getByText('Hybrid Storage Architecture')).toBeInTheDocument();
    });
    
    expect(screen.getByText('VedDB v0.2.0 combines MongoDB-like document storage with Redis-like caching')).toBeInTheDocument();
    expect(screen.getByText('Persistent Layer')).toBeInTheDocument();
    expect(screen.getByText('Cache Layer')).toBeInTheDocument();
    expect(screen.getByText(/RocksDB-backed document storage/)).toBeInTheDocument();
    expect(screen.getByText(/In-memory Redis-like data structures/)).toBeInTheDocument();
  });

  it('should show quick actions', async () => {
    mockUseConnectionStore.mockReturnValue({
      activeConnection: {
        id: 'test-connection',
        isConnected: true,
        name: 'Test Connection',
      },
    });

    render(<DatabaseExplorer />);

    await waitFor(() => {
      expect(screen.getByText('Quick Actions')).toBeInTheDocument();
    });
    
    expect(screen.getByText('Create Collection')).toBeInTheDocument();
    expect(screen.getByText('Manage Indexes')).toBeInTheDocument();
    expect(screen.getByText('User Management')).toBeInTheDocument();
    expect(screen.getByText('View Metrics')).toBeInTheDocument();
  });

  it('should have search functionality for collections', async () => {
    mockUseConnectionStore.mockReturnValue({
      activeConnection: {
        id: 'test-connection',
        isConnected: true,
        name: 'Test Connection',
      },
    });

    render(<DatabaseExplorer />);

    await waitFor(() => {
      const searchInput = screen.getByPlaceholderText('Search collections...');
      expect(searchInput).toBeInTheDocument();

      // Test search functionality
      fireEvent.change(searchInput, { target: { value: 'users' } });
      expect(searchInput).toHaveValue('users');
    });
  });

  it('should show refresh button for collections', async () => {
    mockUseConnectionStore.mockReturnValue({
      activeConnection: {
        id: 'test-connection',
        isConnected: true,
        name: 'Test Connection',
      },
    });

    render(<DatabaseExplorer />);

    await waitFor(() => {
      const refreshButtons = screen.getAllByRole('button');
      expect(refreshButtons.length).toBeGreaterThan(0);
    });
  });

  it('should display collection tree structure', async () => {
    mockUseConnectionStore.mockReturnValue({
      activeConnection: {
        id: 'test-connection',
        isConnected: true,
        name: 'Test Connection',
      },
    });

    render(<DatabaseExplorer />);

    // The component should show the collections section
    expect(screen.getAllByText('Collections')[0]).toBeInTheDocument();
    
    // Should show loading state initially
    expect(screen.getByText('Loading collections...')).toBeInTheDocument();
  });
});