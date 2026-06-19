const { spawnSync } = require('node:child_process');

const args = process.argv.slice(2);
const debug = args.includes('--debug');
const nativeArgs = args.filter((arg) => arg !== '--debug');

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

run('pnpm', ['run', debug ? 'build:native:debug' : 'build:native', ...nativeArgs]);
run('pnpm', ['run', 'build:ts']);
