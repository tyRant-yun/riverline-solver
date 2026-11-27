import { readFileSync, writeFileSync, statSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { resolve } from "node:path";
const root = ".artifacts/solver/m1-capacity",
  binary = resolve(
    `engines/postflop/target/release/riverline-solver${process.platform === "win32" ? ".exe" : ""}`,
  ),
  digest = (p) => createHash("sha256").update(readFileSync(p)).digest("hex"),
  runs = [];
for (const name of ["m1-rainbow-128", "m1-two-tone-128"]) {
  const input = `${root}/${name}.input.json`,
    native = `${root}/${name}.json.native.bin`,
    query = `${root}/${name}.query.json`,
    output = `${root}/${name}.page.json`,
    times = [];
  let maxBytes = 0;
  for (let i = 0; i < 50; i++) {
    writeFileSync(
      query,
      JSON.stringify({
        schemaVersion: 4,
        nativeSha256: digest(native),
        inputSha256: digest(input),
        path:
          i % 2
            ? [
                { kind: "action", index: 0 },
                { kind: "action", index: 0 },
              ]
            : [],
        maxMemoryBytes: 268435456,
        player: i % 2,
        indices: Array.from({ length: 100 }, (_, i) => i),
      }),
    );
    const t = performance.now(),
      r = spawnSync(binary, ["query-turn-page", native, query, output], {
        windowsHide: true,
        encoding: "utf8",
        timeout: 5000,
        maxBuffer: 65536,
      });
    if (r.status !== 0) throw Error(r.stderr);
    times.push(performance.now() - t);
    maxBytes = Math.max(maxBytes, statSync(output).size);
  }
  times.sort((a, b) => a - b);
  runs.push({
    caseId: name,
    samples: 50,
    p50: times[25],
    p95: times[47],
    max: times.at(-1),
    maxOutputBytes: maxBytes,
  });
  const good = JSON.parse(readFileSync(query, "utf8"));
  for (const patch of [
    { schemaVersion: 3 },
    { nativeSha256: "0".repeat(64) },
    { inputSha256: "0".repeat(64) },
    { indices: Array(101).fill(0) },
    { indices: [128] },
    { path: [{ kind: "deal", card: "Ac" }] },
  ]) {
    writeFileSync(query, JSON.stringify({ ...good, ...patch }));
    const r = spawnSync(binary, ["query-turn-page", native, query, output], {
      windowsHide: true,
      encoding: "utf8",
      timeout: 5000,
    });
    if (r.status === 0) throw Error("reader accepted invalid identity/page");
  }
}
const report = {
  method:
    "fresh reader process per sample, native checksum and reload, alternating root/chance, 100 selected rows; no Node cache; 6 invalid identity/version/page/path probes per solution rejected",
  runs,
};
writeFileSync(`${root}/reader-timings.json`, JSON.stringify(report, null, 2));
console.log(JSON.stringify(report, null, 2));
