const { spawnSync } = require('node:child_process');

const rawArgs = process.argv.slice(2);
const debug = rawArgs.includes('--debug');
const nativeOnly = rawArgs.includes('--native-only');
const packageFlagIndex = rawArgs.indexOf('--package');
const packageName = packageFlagIndex >= 0 ? rawArgs[packageFlagIndex + 1] : null;
const nativeArgs = rawArgs.filter((arg, index) => {
  if (arg === '--debug') return false;
  if (arg === '--native-only') return false;
  if (arg === '--package') return false;
  if (packageFlagIndex >= 0 && index === packageFlagIndex + 1) return false;
  return true;
});

const packages = packageName ? [packageName] : ['napi-blitz', 'wasm-blitz'];

function run(command, commandArgs) {
  const executable = process.platform === 'win32' ? `${command}.cmd` : command;
  const result = spawnSync(executable, commandArgs, {
    stdio: 'inherit',
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

for (const pkg of packages) {
  run('pnpm', [
    '--filter',
    `@ylcc/${pkg}`,
    'run',
    debug ? 'build:native:debug' : 'build:native',
    ...nativeArgs,
  ]);
  if (!nativeOnly) {
    run('pnpm', ['--filter', `@ylcc/${pkg}`, 'run', 'build:ts']);
  }
}
