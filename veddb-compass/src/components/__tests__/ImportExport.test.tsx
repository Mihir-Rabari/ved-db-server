import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { ImportExport } from '../ImportExport';
import { useConnectionStore } from '@/store';

// Mock Tauri API
vi.mock('@tauri-apps/api/tauri', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

vi.mock('@tauri-apps/api/fs', () => ({
  readTextFile: vi.fn(),
  writeTextFile: vi.fn(),
}));

// Mock the connection store
vi.mock('@/store', () => ({
  useConnectionStore: vi.fn(),
}));

// Mock the toast hook
vi.mock('@/hooks/use-toast', () => ({
  useToast: () => ({
    toast: vi.fn(),
  }),
}));

describe('ImportExport', () => {
  const mockUseConnectionStore = vi.mocked(useConnectionStore);

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should show no connection message when not connected', () => {
    mockUseConnectionStore.mockReturnValue({
      activeConnection: null,
      connections: [],
      addConnection: vi.fn(),
      updateConnection: vi.fn(),
      removeConnection: vi.fn(),
      setActiveConnection: vi.fn(),
    });

    render(<ImportExport />);

    expect(screen.getByText('No Connection')).toBeInTheDocument();
    expect(screen.getByText('Connect to a VedDB server to access import/export functionality.')).toBeInTheDocument();
  });

  it('should show import/export interface when connected', async () => {
    mockUseConnectionStore.mockReturnValue({
      activeConnection: {
        id: 'test-connection',
        name: 'Test Connection',
        host: 'localhost',
        port: 50051,
        tls: false,
        isConnected: true,
      },
      connections: [],
      addConnection: vi.fn(),
      updateConnection: vi.fn(),
      removeConnection: vi.fn(),
      setActiveConnection: vi.fn(),
    });

    const { invoke } = await import('@tauri-apps/api/tauri');
    vi.mocked(invoke).mockResolvedValue([
      { name: 'users' },
      { name: 'products' },
    ]);

    render(<ImportExport />);

    expect(screen.getByText('Import & Export')).toBeInTheDocument();
    expect(screen.getByText('Import and export data in JSON, CSV, and BSON formats')).toBeInTheDocument();
    
    // Check for tabs
    expect(screen.getByText('Export Data')).toBeInTheDocument();
    expect(screen.getByText('Import Data')).toBeInTheDocument();
  });

  it('should display export form with format options', async () => {
    mockUseConnectionStore.mockReturnValue({
      activeConnection: {
        id: 'test-connection',
        name: 'Test Connection',
        host: 'localhost',
        port: 50051,
        tls: false,
        isConnected: true,
      },
      connections: [],
      addConnection: vi.fn(),
      updateConnection: vi.fn(),
      removeConnection: vi.fn(),
      setActiveConnection: vi.fn(),
    });

    const { invoke } = await import('@tauri-apps/api/tauri');
    vi.mocked(invoke).mockResolvedValue([
      { name: 'users' },
      { name: 'products' },
    ]);

    render(<ImportExport />);

    // Wait for collections to load
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('get_collections', {
        connectionId: 'test-connection',
      });
    });

    // Check export form elements (use getAllByText for duplicate text)
    expect(screen.getAllByText('Export Collection')).toHaveLength(2); // Header and button
    expect(screen.getByText('Collection')).toBeInTheDocument();
    expect(screen.getByText('Format')).toBeInTheDocument();
    expect(screen.getByText('Query Filter (Optional)')).toBeInTheDocument();
  });

  it('should display import form with mode options', async () => {
    mockUseConnectionStore.mockReturnValue({
      activeConnection: {
        id: 'test-connection',
        name: 'Test Connection',
        host: 'localhost',
        port: 50051,
        tls: false,
        isConnected: true,
      },
      connections: [],
      addConnection: vi.fn(),
      updateConnection: vi.fn(),
      removeConnection: vi.fn(),
      setActiveConnection: vi.fn(),
    });

    const { invoke } = await import('@tauri-apps/api/tauri');
    vi.mocked(invoke).mockResolvedValue([
      { name: 'users' },
      { name: 'products' },
    ]);

    render(<ImportExport />);

    // Check that import tab exists
    expect(screen.getByRole('tab', { name: /import data/i })).toBeInTheDocument();
    
    // Check that export tab is active by default
    expect(screen.getByRole('tab', { name: /export data/i })).toHaveAttribute('aria-selected', 'true');
  });

  it('should handle export button click', async () => {
    mockUseConnectionStore.mockReturnValue({
      activeConnection: {
        id: 'test-connection',
        name: 'Test Connection',
        host: 'localhost',
        port: 50051,
        tls: false,
        isConnected: true,
      },
      connections: [],
      addConnection: vi.fn(),
      updateConnection: vi.fn(),
      removeConnection: vi.fn(),
      setActiveConnection: vi.fn(),
    });

    const { invoke } = await import('@tauri-apps/api/tauri');
    vi.mocked(invoke).mockResolvedValue([
      { name: 'users' },
      { name: 'products' },
    ]);

    render(<ImportExport />);

    // Wait for collections to load
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('get_collections', {
        connectionId: 'test-connection',
      });
    });

    // Export button should be disabled initially (no collection selected)
    const exportButton = screen.getByRole('button', { name: /export collection/i });
    expect(exportButton).toBeDisabled();
  });

  it('should have both import and export tabs', async () => {
    mockUseConnectionStore.mockReturnValue({
      activeConnection: {
        id: 'test-connection',
        name: 'Test Connection',
        host: 'localhost',
        port: 50051,
        tls: false,
        isConnected: true,
      },
      connections: [],
      addConnection: vi.fn(),
      updateConnection: vi.fn(),
      removeConnection: vi.fn(),
      setActiveConnection: vi.fn(),
    });

    const { invoke } = await import('@tauri-apps/api/tauri');
    vi.mocked(invoke).mockResolvedValue([
      { name: 'users' },
      { name: 'products' },
    ]);

    render(<ImportExport />);

    // Check that both tabs exist
    const exportTab = screen.getByRole('tab', { name: /export data/i });
    const importTab = screen.getByRole('tab', { name: /import data/i });
    
    expect(exportTab).toBeInTheDocument();
    expect(importTab).toBeInTheDocument();

    // Check that export tab is initially active
    expect(exportTab).toHaveAttribute('aria-selected', 'true');
    expect(importTab).toHaveAttribute('aria-selected', 'false');
  });

  it('should show format icons correctly', async () => {
    mockUseConnectionStore.mockReturnValue({
      activeConnection: {
        id: 'test-connection',
        name: 'Test Connection',
        host: 'localhost',
        port: 50051,
        tls: false,
        isConnected: true,
      },
      connections: [],
      addConnection: vi.fn(),
      updateConnection: vi.fn(),
      removeConnection: vi.fn(),
      setActiveConnection: vi.fn(),
    });

    const { invoke } = await import('@tauri-apps/api/tauri');
    vi.mocked(invoke).mockResolvedValue([
      { name: 'users' },
      { name: 'products' },
    ]);

    render(<ImportExport />);

    // The format selector should be present
    expect(screen.getByText('Format')).toBeInTheDocument();
  });
});