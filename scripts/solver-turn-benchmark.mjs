import { spawnSync } from "node:child_process";
import { readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
const root = resolve(".artifacts/solver/turn-n06");
mkdirSync(root, { recursive: true });
const manifest = JSON.parse(
  readFileSync("benchmarks/solver/turn-cases.json", "utf8"),
);
const binary = resolve(
  `engines/postflop/target/release/riverline-solver${process.platform === "win32" ? ".exe" : ""}`,
);
if (process.platform === "win32") {
  const result = spawnSync(
    "pwsh",
    [
      "-NoProfile",
      "-File",
      "benchmarks/solver/run-river.ps1",
      "-OutputDirectory",
      ".artifacts/solver/turn-n06",
      "-Manifest",
      "turn-cases.json",
      "-Command",
      "validate-turn",
    ],
    { windowsHide: true, stdio: "inherit" },
  );
  if (result.error || result.status !== 0)
    throw Error(`Turn resource benchmark: ${result.error ?? result.status}`);
} else {
  for (const item of manifest.cases) {
    const input = resolve(root, item.caseId + ".input.json"),
      output = resolve(root, item.caseId + ".json");
    writeFileSync(
      input,
      JSON.stringify({ schemaVersion: 3, ...item, limits: manifest.limits }),
    );
    const result = spawnSync(binary, ["validate-turn", input, output], {
      windowsHide: true,
      encoding: "utf8",
      timeout: 130000,
      maxBuffer: 1024 * 1024,
    });
    if (result.error || result.status !== 0)
      throw Error(`${item.caseId}: ${result.error ?? result.stderr}`);
  }
}
const python = resolve(
  process.platform === "win32"
    ? ".artifacts/solver/oracle-venv/Scripts/python.exe"
    : ".artifacts/solver/oracle-venv/bin/python",
);
for (const args of [
  ["benchmarks/solver/test_turn_oracle.py"],
  ["benchmarks/solver/verify_turn.py", root],
]) {
  const result = spawnSync(python, args, {
    windowsHide: true,
    stdio: "inherit",
  });
  if (result.error || result.status !== 0)
    throw Error(`Turn oracle: ${result.error ?? result.status}`);
}
