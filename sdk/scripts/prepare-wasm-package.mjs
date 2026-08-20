import { unlink } from 'node:fs/promises';

const generatedIgnore = new URL('../pkg/.gitignore', import.meta.url);

try {
  await unlink(generatedIgnore);
} catch (error) {
  if (error.code !== 'ENOENT') throw error;
}
