import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { UserManagement } from '../UserManagement';
import { useConnectionStore } from '@/store';

// Mock Tauri API
vi.mock('@tauri-apps/api/tauri', () => ({
  invoke: vi.fn(),
}));

// Mock the store
vi.mock('@/store', () => ({
  useConnectionStore: vi.fn(),
}));

// Mock the toast hook
vi.mock('@/hooks/use-toast', () => ({
  useToast: () => ({
    toast: vi.fn(),
  }),
}));

const mockConnection = {
  id: 'test-connection',
  name: 'Test Server',
  host: 'localhost',
  port: 50051,
  isConnected: true,
};

const mockUsers = [
  {
    username: 'admin',
    role: 'admin',
    created_at: '2024-01-15T10:30:00Z',
    last_login: '2024-11-24T09:15:00Z',
    enabled: true,
  },
  {
    username: 'readonly_user',
    role: 'read-only',
    created_at: '2024-02-01T14:20:00Z',
    last_login: '2024-11-23T16:45:00Z',
    enabled: true,
  },
];

describe('UserManagement', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should show no connection message when not connected', () => {
    (useConnectionStore as any).mockReturnValue({
      activeConnection: null,
    });

    render(<UserManagement />);

    expect(screen.getByText('No Connection')).toBeInTheDocument();
    expect(screen.getByText('Connect to a VedDB server to manage users.')).toBeInTheDocument();
  });

  it('should show user management interface when connected', async () => {
    const { invoke } = await import('@tauri-apps/api/tauri');
    (invoke as any).mockResolvedValue(mockUsers);
    
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection,
    });

    render(<UserManagement />);

    expect(screen.getByText('User Management')).toBeInTheDocument();
    expect(screen.getByText('Create User')).toBeInTheDocument();
    expect(screen.getByText('Refresh')).toBeInTheDocument();
  });

  it('should load and display users', async () => {
    const { invoke } = await import('@tauri-apps/api/tauri');
    (invoke as any).mockResolvedValue(mockUsers);
    
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection,
    });

    render(<UserManagement />);

    await waitFor(() => {
      expect(screen.getAllByText('admin')).toHaveLength(2); // Title and role badge
      expect(screen.getByText('readonly_user')).toBeInTheDocument();
    });

    // Check that user cards are displayed
    expect(screen.getAllByText('Edit Role')).toHaveLength(2);
    expect(screen.getAllByText('Delete')).toHaveLength(2);
  });

  it('should open create user dialog when create button is clicked', async () => {
    const { invoke } = await import('@tauri-apps/api/tauri');
    (invoke as any).mockResolvedValue(mockUsers);
    
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection,
    });

    render(<UserManagement />);

    const createButton = screen.getByText('Create User');
    fireEvent.click(createButton);

    await waitFor(() => {
      expect(screen.getByText('Create New User')).toBeInTheDocument();
      expect(screen.getByPlaceholderText('Enter username')).toBeInTheDocument();
      expect(screen.getByPlaceholderText('Enter password')).toBeInTheDocument();
    });
  });

  it('should show user action buttons', async () => {
    const { invoke } = await import('@tauri-apps/api/tauri');
    (invoke as any).mockResolvedValue(mockUsers);
    
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection,
    });

    render(<UserManagement />);

    await waitFor(() => {
      // Should show action buttons for each user
      expect(screen.getAllByText('Edit Role')).toHaveLength(2);
      expect(screen.getAllByText('Change Password')).toHaveLength(2);
      expect(screen.getAllByText('Delete')).toHaveLength(2);
    });
  });

  it('should show empty state when no users exist', async () => {
    const { invoke } = await import('@tauri-apps/api/tauri');
    (invoke as any).mockResolvedValue([]);
    
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection,
    });

    render(<UserManagement />);

    await waitFor(() => {
      expect(screen.getByText('No Users Found')).toBeInTheDocument();
      expect(screen.getByText('No users are currently configured on this server.')).toBeInTheDocument();
      expect(screen.getByText('Create First User')).toBeInTheDocument();
    });
  });

  it('should handle refresh button click', async () => {
    const { invoke } = await import('@tauri-apps/api/tauri');
    (invoke as any).mockResolvedValue(mockUsers);
    
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection,
    });

    render(<UserManagement />);

    const refreshButton = screen.getByText('Refresh');
    fireEvent.click(refreshButton);

    // Should call invoke again for refresh
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('get_users', {
        connectionId: mockConnection.id,
      });
    });
  });

  it('should format dates correctly', async () => {
    const { invoke } = await import('@tauri-apps/api/tauri');
    (invoke as any).mockResolvedValue(mockUsers);
    
    (useConnectionStore as any).mockReturnValue({
      activeConnection: mockConnection,
    });

    render(<UserManagement />);

    await waitFor(() => {
      // Check that dates are formatted (exact format may vary by locale)
      expect(screen.getAllByText(/Created.*2024/)).toHaveLength(2); // Two users
      expect(screen.getAllByText(/Last login:.*2024/)).toHaveLength(2); // Two users
    });
  });
});