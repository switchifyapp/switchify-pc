export type ParsedVersion = {
  major: number;
  minor: number;
  patch: number;
};

const VERSION_PATTERN = /^v?(\d+)\.(\d+)\.(\d+)$/;

export function parseVersion(value: string): ParsedVersion | null {
  const match = VERSION_PATTERN.exec(value.trim());
  if (!match) return null;

  const [, major, minor, patch] = match;
  return {
    major: Number(major),
    minor: Number(minor),
    patch: Number(patch)
  };
}

export function normalizeVersion(value: string): string | null {
  const parsed = parseVersion(value);
  if (!parsed) return null;
  return `${parsed.major}.${parsed.minor}.${parsed.patch}`;
}

export function compareVersions(left: string, right: string): number | null {
  const parsedLeft = parseVersion(left);
  const parsedRight = parseVersion(right);
  if (!parsedLeft || !parsedRight) return null;

  for (const key of ['major', 'minor', 'patch'] as const) {
    if (parsedLeft[key] > parsedRight[key]) return 1;
    if (parsedLeft[key] < parsedRight[key]) return -1;
  }

  return 0;
}
