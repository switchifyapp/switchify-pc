import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { advisories, evaluate } from "./npm-audit.mjs";

const advisory = (name, id, severity) => ({
  source: 1, name, dependency: name, title: `${name} problem`, severity,
  url: `https://github.com/advisories/${id}`,
});

const report = (...entries) => ({
  auditReportVersion: 2,
  vulnerabilities: Object.fromEntries(entries),
  metadata: {},
});

// The shape npm reported for picomatch: one advisory and the packages it reaches.
const picomatch = report(
  ["picomatch", { name: "picomatch", severity: "high", via: [advisory("picomatch", "GHSA-fc5q-39xc-vjpf", "high")] }],
  ["tinyglobby", { name: "tinyglobby", severity: "high", via: ["fdir", "picomatch"] }],
  ["vite", { name: "vite", severity: "high", via: ["picomatch", "tinyglobby"] }],
);
const exceptions = [{ id: "GHSA-fc5q-39xc-vjpf", package: "picomatch" }];

describe("npm audit gate", () => {
  it("lists each advisory once, ignoring the packages that only depend on it", () => {
    assert.deepEqual(advisories(picomatch).map((a) => a.id), ["GHSA-FC5Q-39XC-VJPF"]);
  });

  it("passes when every high advisory is excepted", () => {
    const result = evaluate(picomatch, exceptions);
    assert.deepEqual(result.blocking, []);
    assert.deepEqual(result.stale, []);
    assert.equal(result.excepted.length, 1);
  });

  it("blocks a new high or critical advisory alongside an excepted one", () => {
    const withNew = report(
      ...Object.entries(picomatch.vulnerabilities),
      ["left-pad", { name: "left-pad", severity: "critical", via: [advisory("left-pad", "GHSA-aaaa-bbbb-cccc", "critical")] }],
      ["right-pad", { name: "right-pad", severity: "high", via: [advisory("right-pad", "GHSA-dddd-eeee-ffff", "high")] }],
    );
    assert.deepEqual(evaluate(withNew, exceptions).blocking.map((a) => a.id), ["GHSA-AAAA-BBBB-CCCC", "GHSA-DDDD-EEEE-FFFF"]);
  });

  it("excepts an advisory only in the package the exception names", () => {
    const shared = report(
      ...Object.entries(picomatch.vulnerabilities),
      ["micromatch", { name: "micromatch", severity: "high", via: [advisory("micromatch", "GHSA-fc5q-39xc-vjpf", "high")] }],
    );
    const result = evaluate(shared, exceptions);
    assert.deepEqual(result.blocking.map((a) => a.package), ["micromatch"]);
    assert.deepEqual(result.excepted.map((a) => a.package), ["picomatch"]);
    assert.deepEqual(evaluate(picomatch, [{ id: "GHSA-fc5q-39xc-vjpf", package: "micromatch" }]).blocking.map((a) => a.package), ["picomatch"]);
  });

  it("does not block moderate or low advisories", () => {
    const moderate = report(["qs", { name: "qs", severity: "moderate", via: [advisory("qs", "GHSA-gggg-hhhh-jjjj", "moderate")] }]);
    assert.deepEqual(evaluate(moderate, []).blocking, []);
  });

  it("reports an exception that no longer matches any advisory as stale", () => {
    assert.deepEqual(evaluate(report(), exceptions).stale, exceptions);
  });

  it("fails when npm audit returns an error instead of a report", () => {
    assert.throws(() => evaluate({ error: { code: "ENOAUDIT", summary: "registry unavailable" } }, exceptions), /did not produce a report/);
    assert.throws(() => evaluate(null, exceptions), /did not produce a report/);
  });

  it("keeps every checked-in exception documented with its reason and issue", () => {
    const checkedIn = JSON.parse(readFileSync(new URL("./npm-audit-exceptions.json", import.meta.url), "utf8"));
    const docs = readFileSync(new URL("../docs/dependency-audit.md", import.meta.url), "utf8");
    for (const exception of checkedIn) {
      assert.match(exception.id, /^GHSA(-[0-9a-z]{4}){3}$/);
      assert.ok(exception.package && exception.reason && exception.issue, `${exception.id} needs package, reason and issue`);
      assert.ok(docs.includes(exception.id), `${exception.id} must be listed in docs/dependency-audit.md`);
    }
  });
});
