// Runs `npm audit` and fails on any high or critical advisory, except the
// ones listed in npm-audit-exceptions.json (see docs/dependency-audit.md).
// An exception that no longer matches an advisory also fails, so it is
// removed as soon as a fixed release is in use.
import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";

export const BLOCKING_SEVERITIES = new Set(["high", "critical"]);

const advisoryId = (url) => String(url ?? "").match(/GHSA(?:-[0-9a-z]{4}){3}/i)?.[0]?.toUpperCase() ?? null;

/** Every advisory in an `npm audit --json` report, once each. */
export function advisories(report) {
  if (!report || typeof report !== "object" || report.error || typeof report.vulnerabilities !== "object") {
    throw new Error(`npm audit did not produce a report: ${JSON.stringify(report?.error ?? report).slice(0, 500)}`);
  }
  const found = new Map();
  for (const vulnerability of Object.values(report.vulnerabilities)) {
    for (const via of vulnerability.via ?? []) {
      // A string names another vulnerable package; its own entry lists the advisory.
      if (typeof via !== "object" || via === null) continue;
      const id = advisoryId(via.url) ?? `npm-${via.source}`;
      if (!found.has(id)) found.set(id, { id, package: via.name, severity: via.severity, title: via.title, url: via.url });
    }
  }
  return [...found.values()];
}

/** The advisories that block, and the exceptions that no longer match anything. */
export function evaluate(report, exceptions) {
  const found = advisories(report);
  const excepted = new Set(exceptions.map((exception) => exception.id.toUpperCase()));
  const present = new Set(found.map((advisory) => advisory.id));
  return {
    blocking: found.filter((advisory) => BLOCKING_SEVERITIES.has(advisory.severity) && !excepted.has(advisory.id)),
    excepted: found.filter((advisory) => excepted.has(advisory.id)),
    stale: exceptions.filter((exception) => !present.has(exception.id.toUpperCase())),
  };
}

function runAudit() {
  try {
    return execSync("npm audit --json", { encoding: "utf8", maxBuffer: 64 * 1024 * 1024, stdio: ["ignore", "pipe", "inherit"] });
  } catch (error) {
    // npm audit exits non-zero whenever it finds anything; the report is still on stdout.
    if (typeof error.stdout === "string" && error.stdout.trim()) return error.stdout;
    throw error;
  }
}

if (process.argv[1] && (await import("node:url")).pathToFileURL(process.argv[1]).href === import.meta.url) {
  const exceptions = JSON.parse(readFileSync(new URL("./npm-audit-exceptions.json", import.meta.url), "utf8"));
  const { blocking, excepted, stale } = evaluate(JSON.parse(runAudit()), exceptions);
  for (const advisory of excepted) {
    console.log(`Excepted ${advisory.severity} advisory ${advisory.id} in ${advisory.package}: ${advisory.title}`);
  }
  for (const advisory of blocking) {
    console.error(`Blocking ${advisory.severity} advisory ${advisory.id} in ${advisory.package}: ${advisory.title} ${advisory.url ?? ""}`);
  }
  for (const exception of stale) {
    console.error(`Stale exception ${exception.id} (${exception.package}) no longer matches an advisory; remove it from scripts/npm-audit-exceptions.json and docs/dependency-audit.md.`);
  }
  if (blocking.length || stale.length) process.exit(1);
  console.log("npm audit: no blocking advisories.");
}
