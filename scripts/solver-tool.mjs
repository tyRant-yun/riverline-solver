import { existsSync, readdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = fileURLToPath(new URL("..", import.meta.url));
const commands = {
  build: ["build", "--release", "--locked"],
  test: ["test", "--workspace", "--locked"],
  fmt: ["fmt", "--all", "--check"],
  format: ["fmt", "--all"],
  clippy: [
    "clippy",
    "--workspace",
    "--all-targets",
    "--locked",
    "--",
    "-D",
    "warnings",
  ],
};
const args = commands[process.argv[2]];
if (!args) throw Error(`Choose: ${Object.keys(commands).join(", ")}`);
const env = { ...process.env };
// Opt-in machine workaround; never overwrite global proxy or Git settings.
if (env.RIVERLINE_SOLVER_DIRECT_NETWORK === "1") {
  for (const key of [
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
  ])
    delete env[key];
}
if (env.RIVERLINE_SOLVER_GIT_SSH === "1") {
  if (env.GIT_CONFIG_COUNT)
    throw Error(
      "Existing per-process Git config: configure SSH explicitly instead",
    );
  env.CARGO_NET_GIT_FETCH_WITH_CLI = "true";
  env.GIT_CONFIG_COUNT = "1";
  env.GIT_CONFIG_KEY_0 = "url.ssh://git@github.com/.insteadOf";
  env.GIT_CONFIG_VALUE_0 = "https://github.com/";
  env.GIT_SSH_COMMAND = "ssh -o BatchMode=yes -o ConnectTimeout=10";
}
const cwd = path.join(root, "engines/postflop");
let command = "cargo";
let commandArgs = args;
if (process.platform === "win32" && !env.VCINSTALLDIR) {
  const installer =
    "C:/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe";
  const registered = existsSync(installer)
    ? spawnSync(
        installer,
        [
          "-latest",
          "-products",
          "*",
          "-requires",
          "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
          "-property",
          "installationPath",
        ],
        { encoding: "utf8", windowsHide: true },
      ).stdout?.trim()
    : "";
  const base = "C:/Program Files/Microsoft Visual Studio";
  const candidates = registered ? [registered] : [];
  if (existsSync(base))
    for (const version of readdirSync(base).sort().reverse()) {
      const folder = path.join(base, version);
      for (const edition of readdirSync(folder, { withFileTypes: true }).filter(
        (e) => e.isDirectory(),
      ))
        candidates.push(path.join(folder, edition.name));
    }
  const vcvars = candidates
    .map((c) => path.join(c, "VC/Auxiliary/Build/vcvars64.bat"))
    .find(existsSync);
  if (!vcvars)
    throw Error(
      "MSVC x64 C++ build tools are required; use a Developer PowerShell or install the tools",
    );
  // Arguments are selected solely from the fixed command table above.
  command = "cmd.exe";
  commandArgs = [
    "/d",
    "/s",
    "/c",
    `""${vcvars}" >nul && cargo ${args.join(" ")}"`,
  ];
}
const result = spawnSync(command, commandArgs, {
  cwd,
  env,
  stdio: "inherit",
  windowsHide: true,
  windowsVerbatimArguments: command === "cmd.exe",
});
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
