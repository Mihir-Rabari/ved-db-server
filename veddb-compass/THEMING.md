# VedDB Compass Theming

## Overview

VedDB Compass supports three theme modes:
- **Light Mode**: Light background with dark text
- **Dark Mode**: Dark background with light text  
- **System Mode**: Automatically follows the user's system preference

## Implementation

### Theme Store
The theme preference is managed by Zustand store with persistence:
- State is stored in `localStorage` with key `veddb-compass-theme`
- Default theme is `system`
- Theme changes are immediately applied to the UI

### Theme Switcher Component
Located at `src/components/ui/theme-switcher.tsx`:
- Dropdown select with theme options
- Icons for each theme mode (Sun, Moon, Monitor)
- Integrated into the Navigation sidebar footer

### CSS Variables
Themes are implemented using CSS custom properties in `src/styles.css`:
- Light theme: Default CSS variables
- Dark theme: Overridden variables in `.dark` class
- Tailwind CSS configured for class-based dark mode

### System Theme Detection
The App component (`src/App.tsx`) handles:
- Applying theme classes to document root
- Listening for system theme changes when in system mode
- Automatic cleanup of event listeners

## Usage

### For Users
1. Open VedDB Compass
2. Look for the theme selector in the bottom of the sidebar
3. Choose between Light, Dark, or System theme
4. Theme preference is automatically saved

### For Developers
```typescript
import { useThemeStore } from '@/store'

function MyComponent() {
  const { theme, setTheme } = useThemeStore()
  
  // Get current theme
  console.log(theme) // 'light' | 'dark' | 'system'
  
  // Change theme
  setTheme('dark')
}
```

## Testing

Theme switcher tests are located at `src/components/ui/__tests__/theme-switcher.test.tsx`:
- Tests theme display for each mode
- Tests component rendering
- Mocks store for isolated testing

## Browser Support

- Modern browsers with CSS custom properties support
- `prefers-color-scheme` media query support for system theme detection
- localStorage support for theme persistence