const fs = require('node:fs');
const { resolveProjectPath } = require('./win-signing-tools.cjs');

const latestPath = resolveProjectPath('dist', 'latest.yml');
if (!fs.existsSync(latestPath)) {
  throw new Error(`latest.yml was not found: ${latestPath}`);
}

const content = fs.readFileSync(latestPath, 'utf8');
if (!/^isAdminRightsRequired:\s*true\s*$/m.test(content)) {
  throw new Error('latest.yml is missing top-level isAdminRightsRequired: true.');
}

if (!/^    isAdminRightsRequired:\s*true\s*$/m.test(content)) {
  throw new Error('latest.yml is missing installer file entry isAdminRightsRequired: true.');
}

console.log('latest.yml admin-rights metadata: present');
