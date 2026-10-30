import { createPrivateKey, randomUUID, sign } from 'node:crypto';

export function issueLicence(privatePem, { keyId, licenceId = randomUUID(), issuedAt = Math.floor(Date.now() / 1000), expiresAt = null, seats = 1 }) {
  if (!/^[A-Za-z0-9_-]{1,32}$/.test(keyId ?? '')
      || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(licenceId)
      || !Number.isSafeInteger(issuedAt) || issuedAt < 0 || issuedAt > 253402300799
      || !Number.isSafeInteger(seats) || seats < 1 || seats > 1000000
      || (expiresAt !== null && (!Number.isSafeInteger(expiresAt) || expiresAt <= issuedAt || expiresAt > 253402300799))) {
    throw new Error('Invalid licence claims');
  }
  const privateKey = createPrivateKey(privatePem);
  if (privateKey.asymmetricKeyType !== 'ed25519') throw new Error('An Ed25519 signing key is required');
  const claims = { version: 1, licence_id: licenceId, product: 'switchify-pc', issued_at: issuedAt, expires_at: expiresAt, seats };
  const message = `SWPC1.${keyId}.${Buffer.from(JSON.stringify(claims)).toString('base64url')}`;
  return { licenceId, key: `${message}.${sign(null, Buffer.from(message), privateKey).toString('base64url')}` };
}
