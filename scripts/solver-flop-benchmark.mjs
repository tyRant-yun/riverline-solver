import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
const directory = resolve(".artifacts/af/flop");
mkdirSync(directory, { recursive: true });
const manifest = JSON.parse(
  readFileSync("benchmarks/solver/flop-cases.json", "utf8"),
);
const binary = resolve(
  `engines/postflop/target/release/riverline-solver${process.platform === "win32" ? ".exe" : ""}`,
);
for (const item of manifest.cases) {
  const input = resolve(directory, `${item.caseId}.input.json`);
  const output = resolve(directory, `${item.caseId}.output.json`);
  writeFileSync(input, JSON.stringify({ ...item, limits: manifest.limits }));
  const r = spawnSync(binary, ["measure-flop", input, output], {
    windowsHide: true,
    timeout: 70000,
    encoding: "utf8",
  });
  if (r.status !== 0 || r.error)
    throw Error(`${item.caseId}: ${r.error ?? r.stderr}`);
  console.log(`${item.caseId}: real native output saved`);
}
const python = resolve(
  process.platform === "win32"
    ? ".artifacts/solver/oracle-venv/Scripts/python.exe"
    : ".artifacts/solver/oracle-venv/bin/python",
);
const check = spawnSync(
  python,
  ["benchmarks/solver/verify_flop.py", directory],
  { windowsHide: true, stdio: "inherit", timeout: 240000 },
);
if (check.status !== 0 || check.error)
  throw Error(`Flop independent oracle: ${check.error ?? check.status}`);
