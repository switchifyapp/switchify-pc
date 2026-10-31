import { pathToFileURL } from 'node:url';
import { Buffer } from 'node:buffer';

// Public fixture anchors, never valid for a release build.
const fixtureKeys = new Set([
  '_RckOFqgx1tk-3jNYC-h2ZH96_drE8WO1wLqyDXp9hg',
  '6kpsY-KcUgq-9VB7Ey7F-ZVHdq6-vnuSQh7qaRRG0iw',
  'E5j2LG0aRXxRumpLXz29L2n8qTIWIY3ImX5Ba9F9k8o',
]);
function keys(raw) {
  const map=JSON.parse(raw || 'null');
  if (!map || typeof map !== 'object' || Array.isArray(map) || !Object.keys(map).length) throw new Error('Missing licence trust anchors');
  for (const [id,key] of Object.entries(map)) {
    if (!/^[A-Za-z0-9_-]{1,32}$/.test(id) || typeof key !== 'string' || !/^[A-Za-z0-9_-]{43}$/.test(key)
      || Buffer.from(key,'base64url').length!==32 || Buffer.from(key,'base64url').toString('base64url')!==key || fixtureKeys.has(key)) throw new Error('Invalid or test-only licence trust anchor');
  }
  return map;
}
export function validateLicenceConfig(env) {
  const purchases=keys(env.SWITCHIFY_LICENCE_PUBLIC_KEYS), leases=keys(env.SWITCHIFY_LICENCE_LEASE_PUBLIC_KEYS);
  for (const [id,key] of Object.entries(leases)) {
    if (Object.hasOwn(purchases,id) || Object.values(purchases).includes(key)) throw new Error('Purchase and lease issuers must be separate');
  }
  const url=new URL(env.SWITCHIFY_SUPABASE_URL || '');
  if (url.protocol!=='https:' || !url.hostname || url.username || url.password || url.pathname!=='/' || url.search || url.hash
    || !env.SWITCHIFY_SUPABASE_PUBLISHABLE_KEY?.trim()) throw new Error('Missing or invalid licence activation endpoint');
}
if (process.argv[1] && import.meta.url===pathToFileURL(process.argv[1]).href) {
  try { validateLicenceConfig(process.env); }
  catch { console.error('Release licence configuration is missing, invalid, test-only or overlaps signer roles.');process.exitCode=1; }
}
