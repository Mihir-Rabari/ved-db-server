import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ConnectionForm } from '../ConnectionForm';
import { useConnectionStore } from '@/store';
import { invoke } from '@tauri-apps/api/tauri';

// Mock the store
vi.mock('@/store', () => ({
  useConnectionStore: vi.fn(),
}));

// Mock Tauri API
vi.mock('@tauri-apps/api/tauri', () => ({
  invoke: vi.fn(),
}));

// Mock toast hook
const mockToast = vi.fn();
vi.mock('@/hooks/use-toast', () => ({
  useToast: () => ({
    toast: mockToast,
  }),
}));

describe('ConnectionForm - Error Handling Integration Tests', () => {
  const mockUseConnectionStore = vi.mocked(useConnectionStore);
  const mockInvoke = vi.mocked(invoke);
  const mockOnClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockUseConnectionStore.mockReturnValue({
      connections: [],
      addConnection: vi.fn(),
      updateConnection: vi.fn(),
      removeConnection: vi.fn(),
      activeConnection: null,
      setActiveConnection: vi.fn(),
    });
  });

  describe('Form Input Preservation on Error', () => {
    it('should preserve all form inputs when DNS resolution fails', async () => {
      mockInvoke.mockRejectedValueOnce('Failed to resolve hostname "invalid.example.com": Name or service not known');

      render(<ConnectionForm onClose={mockOnClose} />);

      // Fill in form
      const nameInput = screen.getByLabelText(/Connection Name/i);
      const hostInput = screen.getByLabelText(/Host/i);
      const portInput = screen.getByLabelText(/Port/i);
      const usernameInput = screen.getByLabelText(/Username/i);
      const passwordInput = screen.getByLabelText(/Password/i);

      fireEvent.change(nameInput, { target: { value: 'Test Connection' } });
      fireEvent.change(hostInput, { target: { value: 'invalid.example.com' } });
      fireEvent.change(portInput, { target: { value: '50051' } });
      fireEvent.change(usernameInput, { target: { value: 'testuser' } });
      fireEvent.change(passwordInput, { target: { value: 'testpass' } });

      // Test connection
      const testButton = screen.getByRole('button', { name: /Test Connection/i });
      fireEvent.click(testButton);

      // Wait for error
      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalled();
      });

      // Verify all inputs are preserved
      expect(nameInput).toHaveValue('Test Connection');
      expect(hostInput).toHaveValue('invalid.example.com');
      expect(portInput).toHaveValue(50051);
      expect(usernameInput).toHaveValue('testuser');
      expect(passwordInput).toHaveValue('testpass');
    });

    it('should preserve form inputs when connection is refused', async () => {
      mockInvoke.mockRejectedValueOnce('Connection refused');

      render(<ConnectionForm onClose={mockOnClose} />);

      const nameInput = screen.getByLabelText(/Connection Name/i);
      const hostInput = screen.getByLabelText(/Host/i);

      fireEvent.change(nameInput, { target: { value: 'Local Server' } });
      fireEvent.change(hostInput, { target: { value: '127.0.0.1' } });

      const testButton = screen.getByRole('button', { name: /Test Connection/i });
      fireEvent.click(testButton);

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalled();
      });

      expect(nameInput).toHaveValue('Local Server');
      expect(hostInput).toHaveValue('127.0.0.1');
    });

    it('should preserve form inputs when authentication fails', async () => {
      mockInvoke.mockRejectedValueOnce('Authentication failed: Invalid credentials');

      render(<ConnectionForm onClose={mockOnClose} />);

      const nameInput = screen.getByLabelText(/Connection Name/i);
      const usernameInput = screen.getByLabelText(/Username/i);
      const passwordInput = screen.getByLabelText(/Password/i);

      fireEvent.change(nameInput, { target: { value: 'Secure Server' } });
      fireEvent.change(usernameInput, { target: { value: 'admin' } });
      fireEvent.change(passwordInput, { target: { value: 'wrongpass' } });

      const testButton = screen.getByRole('button', { name: /Test Connection/i });
      fireEvent.click(testButton);

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalled();
      });

      expect(nameInput).toHaveValue('Secure Server');
      expect(usernameInput).toHaveValue('admin');
      expect(passwordInput).toHaveValue('wrongpass');
    });
  });

  describe('Latest Error Message Display', () => {
    it('should display the most recent error message when multiple errors occur', async () => {
      render(<ConnectionForm onClose={mockOnClose} />);

      const nameInput = screen.getByLabelText(/Connection Name/i);
      const hostInput = screen.getByLabelText(/Host/i);
      const testButton = screen.getByRole('button', { name: /Test Connection/i });

      fireEvent.change(nameInput, { target: { value: 'Test' } });
      fireEvent.change(hostInput, { target: { value: 'first.example.com' } });

      // First error
      mockInvoke.mockRejectedValueOnce('Failed to resolve hostname "first.example.com"');
      fireEvent.click(testButton);

      await waitFor(() => {
        expect(screen.getByText(/Could not resolve hostname/i)).toBeInTheDocument();
      });

      // Change host and trigger second error
      fireEvent.change(hostInput, { target: { value: '127.0.0.1' } });
      mockInvoke.mockRejectedValueOnce('Connection refused');
      fireEvent.click(testButton);

      await waitFor(() => {
        expect(screen.getByText(/refused the connection/i)).toBeInTheDocument();
      });

      // Verify only the latest error is shown (not the first one)
      expect(screen.queryByText(/Could not resolve hostname "first.example.com"/i)).not.toBeInTheDocument();
    });

    it('should clear error message when form input changes', async () => {
      mockInvoke.mockRejectedValueOnce('Connection refused');

      render(<ConnectionForm onClose={mockOnClose} />);

      const nameInput = screen.getByLabelText(/Connection Name/i);
      const hostInput = screen.getByLabelText(/Host/i);
      const testButton = screen.getByRole('button', { name: /Test Connection/i });

      fireEvent.change(nameInput, { target: { value: 'Test' } });
      fireEvent.change(hostInput, { target: { value: '127.0.0.1' } });
      fireEvent.click(testButton);

      await waitFor(() => {
        expect(screen.getByText(/refused the connection/i)).toBeInTheDocument();
      });

      // Change input
      fireEvent.change(hostInput, { target: { value: '192.168.1.1' } });

      // Error should be cleared
      await waitFor(() => {
        expect(screen.queryByText(/refused the connection/i)).not.toBeInTheDocument();
      });
    });

    it('should show success message after previous error', async () => {
      render(<ConnectionForm onClose={mockOnClose} />);

      const nameInput = screen.getByLabelText(/Connection Name/i);
      const hostInput = screen.getByLabelText(/Host/i);
      const testButton = screen.getByRole('button', { name: /Test Connection/i });

      fireEvent.change(nameInput, { target: { value: 'Test' } });
      fireEvent.change(hostInput, { target: { value: 'bad.host' } });

      // First attempt fails
      mockInvoke.mockRejectedValueOnce('Failed to resolve hostname');
      fireEvent.click(testButton);

      await waitFor(() => {
        expect(screen.getByText(/Could not resolve hostname/i)).toBeInTheDocument();
      });

      // Fix the host and try again
      fireEvent.change(hostInput, { target: { value: 'localhost' } });
      mockInvoke.mockResolvedValueOnce(true);
      fireEvent.click(testButton);

      await waitFor(() => {
        expect(screen.getByText(/Connection successful/i)).toBeInTheDocument();
      });

      // Error should not be visible anymore
      expect(screen.queryByText(/Could not resolve hostname/i)).not.toBeInTheDocument();
    });
  });

  describe('Connection Status Updates', () => {
    it('should show "Resolving hostname..." status during DNS resolution', async () => {
      let resolveTest: () => void;
      const testPromise = new Promise<boolean>((resolve) => {
        resolveTest = () => resolve(true);
      });
      mockInvoke.mockReturnValueOnce(testPromise);

      render(<ConnectionForm onClose={mockOnClose} />);

      const nameInput = screen.getByLabelText(/Connection Name/i);
      const hostInput = screen.getByLabelText(/Host/i);
      const testButton = screen.getByRole('button', { name: /Test Connection/i });

      fireEvent.change(nameInput, { target: { value: 'Test' } });
      fireEvent.change(hostInput, { target: { value: 'example.com' } });
      fireEvent.click(testButton);

      // Should show either resolving or connecting status (transitions happen quickly)
      await waitFor(() => {
        const text = screen.queryByText(/Resolving hostname/i) || screen.queryByText(/Connecting to server/i);
        expect(text).toBeInTheDocument();
      });

      resolveTest!();
    });

    it('should show "Connecting to server..." status after resolution', async () => {
      let resolveTest: () => void;
      const testPromise = new Promise<boolean>((resolve) => {
        resolveTest = () => {
          setTimeout(() => resolve(true), 100);
        };
      });
      mockInvoke.mockReturnValueOnce(testPromise);

      render(<ConnectionForm onClose={mockOnClose} />);

      const nameInput = screen.getByLabelText(/Connection Name/i);
      const testButton = screen.getByRole('button', { name: /Test Connection/i });

      fireEvent.change(nameInput, { target: { value: 'Test' } });
      fireEvent.click(testButton);

      // Should show connecting status (may skip resolving due to fast transition)
      await waitFor(() => {
        expect(screen.getByText(/Connecting to server/i)).toBeInTheDocument();
      });

      resolveTest!();

      // Should eventually show success
      await waitFor(() => {
        expect(screen.getByText(/Connection successful/i)).toBeInTheDocument();
      });
    });

    it('should clear status indicators on successful connection', async () => {
      mockInvoke.mockResolvedValueOnce(true);

      render(<ConnectionForm onClose={mockOnClose} />);

      const nameInput = screen.getByLabelText(/Connection Name/i);
      const testButton = screen.getByRole('button', { name: /Test Connection/i });

      fireEvent.change(nameInput, { target: { value: 'Test' } });
      fireEvent.click(testButton);

      await waitFor(() => {
        expect(screen.getByText(/Connection successful/i)).toBeInTheDocument();
      });

      // Status indicators should not be visible
      expect(screen.queryByText(/Resolving hostname/i)).not.toBeInTheDocument();
      expect(screen.queryByText(/Connecting to server/i)).not.toBeInTheDocument();
    });

    it('should reset status to idle when form input changes', async () => {
      mockInvoke.mockRejectedValueOnce('Connection refused');

      render(<ConnectionForm onClose={mockOnClose} />);

      const nameInput = screen.getByLabelText(/Connection Name/i);
      const hostInput = screen.getByLabelText(/Host/i);
      const testButton = screen.getByRole('button', { name: /Test Connection/i });

      fireEvent.change(nameInput, { target: { value: 'Test' } });
      fireEvent.click(testButton);

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalled();
      });

      // Change input should reset status
      fireEvent.change(hostInput, { target: { value: 'newhost' } });

      // No status indicators should be visible
      expect(screen.queryByText(/Resolving hostname/i)).not.toBeInTheDocument();
      expect(screen.queryByText(/Connecting to server/i)).not.toBeInTheDocument();
    });
  });

  describe('Toast Notification Messages', () => {
    it('should show DNS resolution error toast with appropriate message', async () => {
      mockInvoke.mockRejectedValueOnce('Failed to resolve hostname "bad.host"');

      render(<ConnectionForm onClose={mockOnClose} />);

      const nameInput = screen.getByLabelText(/Connection Name/i);
      const hostInput = screen.getByLabelText(/Host/i);
      const testButton = screen.getByRole('button', { name: /Test Connection/i });

      fireEvent.change(nameInput, { target: { value: 'Test' } });
      fireEvent.change(hostInput, { target: { value: 'bad.host' } });
      fireEvent.click(testButton);

      await waitFor(() => {
        expect(mockToast).toHaveBeenCalledWith(
          expect.objectContaining({
            title: 'DNS Resolution Failed',
            description: expect.stringContaining('Could not resolve hostname'),
            variant: 'destructive',
          })
        );
      });
    });

    it('should show connection refused error toast', async () => {
      mockInvoke.mockRejectedValueOnce('Connection refused');

      render(<ConnectionForm onClose={mockOnClose} />);

      const nameInput = screen.getByLabelText(/Connection Name/i);
      const testButton = screen.getByRole('button', { name: /Test Connection/i });

      fireEvent.change(nameInput, { target: { value: 'Test' } });
      fireEvent.click(testButton);

      await waitFor(() => {
        expect(mockToast).toHaveBeenCalledWith(
          expect.objectContaining({
            title: 'Connection Refused',
            description: expect.stringContaining('refused the connection'),
            variant: 'destructive',
          })
        );
      });
    });

    it('should show timeout error toast', async () => {
      mockInvoke.mockRejectedValueOnce('Connection timeout');

      render(<ConnectionForm onClose={mockOnClose} />);

      const nameInput = screen.getByLabelText(/Connection Name/i);
      const testButton = screen.getByRole('button', { name: /Test Connection/i });

      fireEvent.change(nameInput, { target: { value: 'Test' } });
      fireEvent.click(testButton);

      await waitFor(() => {
        expect(mockToast).toHaveBeenCalledWith(
          expect.objectContaining({
            title: 'Connection Timeout',
            description: expect.stringContaining('timed out'),
            variant: 'destructive',
          })
        );
      });
    });

    it('should show authentication error toast', async () => {
      mockInvoke.mockRejectedValueOnce('Authentication failed: Invalid credentials');

      render(<ConnectionForm onClose={mockOnClose} />);

      const nameInput = screen.getByLabelText(/Connection Name/i);
      const testButton = screen.getByRole('button', { name: /Test Connection/i });

      fireEvent.change(nameInput, { target: { value: 'Test' } });
      fireEvent.click(testButton);

      await waitFor(() => {
        expect(mockToast).toHaveBeenCalledWith(
          expect.objectContaining({
            title: 'Authentication Failed',
            description: expect.stringContaining('Invalid username or password'),
            variant: 'destructive',
          })
        );
      });
    });

    it('should show success toast on successful connection', async () => {
      mockInvoke.mockResolvedValueOnce(true);

      render(<ConnectionForm onClose={mockOnClose} />);

      const nameInput = screen.getByLabelText(/Connection Name/i);
      const hostInput = screen.getByLabelText(/Host/i);
      const testButton = screen.getByRole('button', { name: /Test Connection/i });

      fireEvent.change(nameInput, { target: { value: 'Test' } });
      fireEvent.change(hostInput, { target: { value: 'localhost' } });
      fireEvent.click(testButton);

      await waitFor(() => {
        expect(mockToast).toHaveBeenCalledWith(
          expect.objectContaining({
            title: 'Connection Test Successful',
            description: expect.stringContaining('Successfully connected to localhost:50051'),
            variant: 'success',
          })
        );
      });
    });
  });
});
