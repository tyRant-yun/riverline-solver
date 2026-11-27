import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
const definition = JSON.parse(
  readFileSync("benchmarks/solver/m1-capacity-definition.json", "utf8"),
);
const ranks = [..."AKQJT98765432"],
  suits = [..."cdhs"];
function ranges(board, size) {
  const deck = ranks
    .flatMap((r) => suits.map((s) => r + s))
    .filter((c) => !board.includes(c));
  const combos = deck.flatMap((a, i) => deck.slice(i + 1).map((b) => a + b));
  return [17, 191].map((offset) =>
    Object.fromEntries(
      Array.from({ length: size }, (_, i) => [
        combos[(i * 337 + offset) % combos.length],
        i % 7 === 0 ? 0.25 : i % 3 === 0 ? 0.6 : 1,
      ]),
    ),
  );
}
const cases = definition.boards.flatMap((b) =>
  definition.sizes.map((size) => ({
    caseId: `m1-${b.id}-${size}`,
    board: b.board,
    pot: b.pot,
    stack: b.stack,
    turnBets: b.turnBets,
    riverBets: b.riverBets,
    ranges: ranges(b.board, size),
  })),
);
const b = definition.checkdown;
cases.push({
  caseId: "m1-checkdown-64",
  board: b.board,
  pot: b.pot,
  stack: b.stack,
  turnBets: b.turnBets,
  riverBets: b.riverBets,
  ranges: ranges(b.board, b.size),
});
const manifest = {
  schemaVersion: 4,
  definitionSha256: createHash("sha256")
    .update(readFileSync("benchmarks/solver/m1-capacity-definition.json"))
    .digest("hex"),
  limits: definition.limits,
  cases,
};
mkdirSync(".artifacts/solver/m1-capacity", { recursive: true });
writeFileSync(
  ".artifacts/solver/m1-capacity/frozen-manifest.json",
  JSON.stringify(manifest, null, 2) + "\n",
);
const r = spawnSync(
  "pwsh",
  ["-NoProfile", "-File", "benchmarks/solver/run-m1-capacity.ps1"],
  { stdio: "inherit", windowsHide: true },
);
if (r.status !== 0) throw Error(`M1 matrix failed: ${r.status}`);
