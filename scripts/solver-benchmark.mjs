import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = fileURLToPath(new URL("..", import.meta.url));
const tree = process.argv.includes("--tree");
if (process.platform !== "win32")
  throw Error("R1 resource runner currently verified on Windows only");
const python = path.join(
  root,
  ".artifacts/solver/oracle-venv/Scripts/python.exe",
);
if (!existsSync(python))
  throw Error("Prepare the oracle venv first; see benchmarks/solver/README.md");
const commands = [
  [
    "pwsh",
    [
      "-NoProfile",
      "-File",
      path.join(root, "benchmarks/solver/run-river.ps1"),
      ...(tree
        ? [
            "-OutputDirectory",
            ".artifacts/solver/river-tree",
            "-Manifest",
            "river-tree-cases.json",
            "-Command",
            "validate-river-tree",
          ]
        : []),
    ],
  ],
  [
    python,
    [
      path.join(
        root,
        tree
          ? "benchmarks/solver/verify_river_tree.py"
          : "benchmarks/solver/verify_river.py",
      ),
      path.join(
        root,
        tree ? ".artifacts/solver/river-tree" : ".artifacts/solver/r1",
      ),
    ],
  ],
  [
    python,
    [
      path.join(
        root,
        tree
          ? "benchmarks/solver/test_tree_oracle.py"
          : "benchmarks/solver/test_oracle.py",
      ),
    ],
  ],
];
for (const [command, args] of commands) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: "inherit",
    windowsHide: true,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
