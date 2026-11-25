import { configDefaults, defineConfig, mergeConfig } from 'vitest/config'

import viteConfig from './vite.config.ts'

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'jsdom',
      setupFiles: ['./src/test/setup.ts'],
      // las pruebas en navegador real (`e2e/`) las corre Playwright: `npm run test:e2e` (#661)
      exclude: [...configDefaults.exclude, 'e2e/**'],
    },
  }),
)
