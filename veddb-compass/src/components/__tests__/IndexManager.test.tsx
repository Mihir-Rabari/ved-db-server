import { render, screen } from '@testing-library/react'
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { IndexManager } from '../IndexManager'

// Mock the modules
vi.mock('@/store', () => ({
  useConnectionStore: () => ({
    activeConnection: null,
  }),
}))

vi.mock('@/hooks/use-toast', () => ({
  useToast: () => ({
    toast: vi.fn(),
  }),
}))

vi.mock('@tauri-apps/api/tauri', () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}))

describe('IndexManager', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders no connection state when not connected', () => {
    render(<IndexManager />)
    
    expect(screen.getByText('Index Manager')).toBeInTheDocument()
    expect(screen.getByText('Connect to a VedDB server to manage database indexes.')).toBeInTheDocument()
  })
})