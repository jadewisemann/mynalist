import { configDefaults, defineConfig } from 'vitest/config';

// .claude/ holds agent worktrees (full copies of the repo): never collect their tests
export default defineConfig({ test: { exclude: [...configDefaults.exclude, '.claude/**'] } });
