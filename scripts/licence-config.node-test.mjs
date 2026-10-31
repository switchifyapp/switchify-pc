import test from 'node:test';
import assert from 'node:assert/strict';
import { Buffer } from 'node:buffer';
import { readFile } from 'node:fs/promises';
import { validateLicenceConfig } from './validate-licence-config.mjs';
const env={SWITCHIFY_LICENCE_PUBLIC_KEYS:JSON.stringify({purchase:Buffer.alloc(32,31).toString('base64url')}),
  SWITCHIFY_LICENCE_LEASE_PUBLIC_KEYS:JSON.stringify({lease:Buffer.alloc(32,32).toString('base64url')}),
  SWITCHIFY_SUPABASE_URL:'https://example.supabase.co',SWITCHIFY_SUPABASE_PUBLISHABLE_KEY:'public-test-placeholder'};
test('release config requires separate nonempty purchase and lease trust maps and secure endpoint',()=>{
  assert.doesNotThrow(()=>validateLicenceConfig(env));
  for(const patch of [
    {SWITCHIFY_LICENCE_PUBLIC_KEYS:''},{SWITCHIFY_LICENCE_LEASE_PUBLIC_KEYS:'{}'},
    {SWITCHIFY_LICENCE_LEASE_PUBLIC_KEYS:env.SWITCHIFY_LICENCE_PUBLIC_KEYS},
    {SWITCHIFY_LICENCE_LEASE_PUBLIC_KEYS:JSON.stringify({different:Buffer.alloc(32,31).toString('base64url')})},
    {SWITCHIFY_SUPABASE_URL:'http://example.com'},{SWITCHIFY_SUPABASE_PUBLISHABLE_KEY:''},
  ]) assert.throws(()=>validateLicenceConfig({...env,...patch}));
});
test('release config rejects every frozen fixture signer',async()=>{
  for(const path of ['../tools/licence-keygen/test-vector.json','../tools/licence-keygen/activation-test-vector.json']) {
    const vector=JSON.parse(await readFile(new URL(path,import.meta.url)));
    assert.throws(()=>validateLicenceConfig({...env,SWITCHIFY_LICENCE_LEASE_PUBLIC_KEYS:JSON.stringify({test:vector.public_key})}));
  }
});
