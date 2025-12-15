import { create } from 'zustand'
import { devtools, persist } from 'zustand/middleware'

// Connection state
export interface Connection {
  id: string
  name: string
  host: string
  port: number
  username?: string
  password?: string
  tls: boolean
  isConnected: boolean
}

interface ConnectionState {
  connections: Connection[]
  activeConnection: Connection | null
  addConnection: (connection: Omit<Connection, 'id' | 'isConnected'>) => void
  updateConnection: (id: string, updates: Partial<Connection>) => void
  removeConnection: (id: string) => void
  setActiveConnection: (connection: Connection | null) => void
}

export const useConnectionStore = create<ConnectionState>()(
  devtools(
    persist(
      (set) => ({
        connections: [],
        activeConnection: null,
        addConnection: (connection) => {
          const newConnection: Connection = {
            ...connection,
            id: crypto.randomUUID(),
            isConnected: false,
          }
          set((state) => ({
            connections: [...state.connections, newConnection],
          }))
        },
        updateConnection: (id, updates) => {
          set((state) => ({
            connections: state.connections.map((conn) =>
              conn.id === id ? { ...conn, ...updates } : conn
            ),
            activeConnection:
              state.activeConnection?.id === id
                ? { ...state.activeConnection, ...updates }
                : state.activeConnection,
          }))
        },
        removeConnection: (id) => {
          set((state) => ({
            connections: state.connections.filter((conn) => conn.id !== id),
            activeConnection:
              state.activeConnection?.id === id ? null : state.activeConnection,
          }))
        },
        setActiveConnection: (connection) => {
          set({ activeConnection: connection })
        },
      }),
      {
        name: 'veddb-compass-connections',
        partialize: (state) => ({
          connections: state.connections.map((conn) => ({
            ...conn,
            isConnected: false, // Don't persist connection status
          })),
        }),
      }
    ),
    {
      name: 'connection-store',
    }
  )
)

// Theme state
interface ThemeState {
  theme: 'light' | 'dark' | 'system'
  setTheme: (theme: 'light' | 'dark' | 'system') => void
}

export const useThemeStore = create<ThemeState>()(
  devtools(
    persist(
      (set) => ({
        theme: 'system',
        setTheme: (theme) => set({ theme }),
      }),
      {
        name: 'veddb-compass-theme',
      }
    ),
    {
      name: 'theme-store',
    }
  )
)

// Application state
interface AppState {
  sidebarCollapsed: boolean
  setSidebarCollapsed: (collapsed: boolean) => void
  currentView: 'connections' | 'explorer' | 'query' | 'aggregation' | 'metrics' | 'users' | 'import-export' | 'indexes'
  setCurrentView: (view: AppState['currentView']) => void
}

export const useAppStore = create<AppState>()(
  devtools(
    (set) => ({
      sidebarCollapsed: false,
      setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),
      currentView: 'connections',
      setCurrentView: (view) => set({ currentView: view }),
    }),
    {
      name: 'app-store',
    }
  )
)