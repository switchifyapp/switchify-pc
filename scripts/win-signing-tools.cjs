const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

function isWindows() {
  return process.platform === 'win32';
}

function resolveProjectPath(...segments) {
  return path.resolve(__dirname, '..', ...segments);
}

function findWindowsSdkTool(toolName) {
  const envOverride = toolName.toLowerCase() === 'mt.exe' ? process.env.SWITCHIFY_MT_EXE : process.env.SWITCHIFY_SIGNTOOL_EXE;
  if (envOverride) {
    if (!fs.existsSync(envOverride)) {
      throw new Error(`${path.basename(envOverride)} was configured but does not exist: ${envOverride}`);
    }
    return envOverride;
  }

  const sdkBinRoot = 'C:\\Program Files (x86)\\Windows Kits\\10\\bin';
  const sdkTool = findNewestSdkTool(sdkBinRoot, toolName);
  if (sdkTool) return sdkTool;

  const whereResult = spawnSync('where.exe', [toolName], { encoding: 'utf8' });
  if (whereResult.status === 0) {
    const [firstMatch] = whereResult.stdout.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
    if (firstMatch) return firstMatch;
  }

  throw new Error(`Unable to find ${toolName}. Install the Windows SDK or set ${toolName.toLowerCase() === 'mt.exe' ? 'SWITCHIFY_MT_EXE' : 'SWITCHIFY_SIGNTOOL_EXE'}.`);
}

function runTool(command, args, options = {}) {
  const displayCommand = [command, ...redactSensitiveArgs(args)].join(' ');
  const result = spawnSync(command, args, {
    stdio: options.stdio ?? 'inherit',
    encoding: 'utf8',
    ...options
  });

  if (result.status !== 0) {
    throw new Error(`${displayCommand} failed with exit code ${result.status ?? 'unknown'}.`);
  }

  return result;
}

function findNewestSdkTool(sdkBinRoot, toolName) {
  if (!fs.existsSync(sdkBinRoot)) return null;

  const versions = fs.readdirSync(sdkBinRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort(compareVersionDescending);

  for (const version of versions) {
    const candidate = path.join(sdkBinRoot, version, 'x64', toolName);
    if (fs.existsSync(candidate)) return candidate;
  }

  return null;
}

function compareVersionDescending(left, right) {
  const leftParts = left.split('.').map((part) => Number.parseInt(part, 10));
  const rightParts = right.split('.').map((part) => Number.parseInt(part, 10));
  const length = Math.max(leftParts.length, rightParts.length);

  for (let index = 0; index < length; index += 1) {
    const delta = (rightParts[index] || 0) - (leftParts[index] || 0);
    if (delta !== 0) return delta;
  }

  return right.localeCompare(left);
}

function redactSensitiveArgs(args) {
  return args.map((arg, index) => {
    const previous = args[index - 1]?.toLowerCase();
    if (previous === '/p' || previous === '-p') return '<redacted>';
    return String(arg);
  });
}

module.exports = {
  findWindowsSdkTool,
  isWindows,
  resolveProjectPath,
  runTool
};
