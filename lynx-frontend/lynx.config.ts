import { defineConfig } from '@lynx-js/rspeedy';
import { fileURLToPath } from 'node:url';

const seedDesignDir = fileURLToPath(new URL('./seed-design', import.meta.url));

export default defineConfig({
  resolve: {
    alias: {
      'seed-design': seedDesignDir,
    },
  },
});
