import { describe, it, expect, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ThemeSwitcher } from '../theme-switcher'
import { useThemeStore } from '@/store'

// Mock the store
const mockSetTheme = vi.fn()
vi.mock('@/store', () => ({
  useThemeStore: vi.fn()
}))

describe('ThemeSwitcher', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    ;(useThemeStore as any).mockReturnValue({
      theme: 'system',
      setTheme: mockSetTheme
    })
  })

  it('should render theme switcher with system theme selected', () => {
    render(<ThemeSwitcher />)
    
    expect(screen.getByRole('combobox')).toBeInTheDocument()
    expect(screen.getByText('System')).toBeInTheDocument()
  })

  it('should display light theme when selected', () => {
    ;(useThemeStore as any).mockReturnValue({
      theme: 'light',
      setTheme: mockSetTheme
    })

    render(<ThemeSwitcher />)
    
    expect(screen.getByText('Light')).toBeInTheDocument()
  })

  it('should display dark theme when selected', () => {
    ;(useThemeStore as any).mockReturnValue({
      theme: 'dark',
      setTheme: mockSetTheme
    })

    render(<ThemeSwitcher />)
    
    expect(screen.getByText('Dark')).toBeInTheDocument()
  })
})