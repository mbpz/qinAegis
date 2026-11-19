import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';

// Public marketing build: deliberately independent of the workbench API and assets.
export default defineConfig({
  root: path.resolve(__dirname, '../../../docs/landing'),
  plugins: [react(), tailwindcss()],
  base: './',
  define: {
    __SOURCE_REF__: JSON.stringify(process.env.QINAEGIS_PAGES_SOURCE_REF || 'codex/qinaegis-2.0'),
  },
  resolve: {
    alias: Object.fromEntries([
      ['@', path.resolve(__dirname, 'src')],
      ...['react', 'react-dom', 'motion', 'lucide-react', 'clsx', 'tailwind-merge'].map(
        (name) => [name, path.resolve(__dirname, 'node_modules', name)],
      ),
    ]),
  },
  build: { outDir: path.resolve(__dirname, '../../../build/pages'), emptyOutDir: true },
});
