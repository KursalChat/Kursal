import { readFileSync, writeFileSync } from "fs";

const version = process.argv[2];

if (!version) {
  console.error("usage: bun run bin/bump-version.ts <version>");
  process.exit(1);
}

if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?$/.test(version)) {
  console.error(`invalid semver: ${version} (expected e.g. 0.1.0 or 0.1.0-beta.2)`);
  process.exit(1);
}

const cargoPath = "Cargo.toml";
const cargo = readFileSync(cargoPath, "utf8");
const bumped = cargo.replace(
  /(\[workspace\.package\][\s\S]*?\nversion\s*=\s*")[^"]*(")/,
  `$1${version}$2`,
);
if (bumped === cargo) {
  console.error("could not find [workspace.package] version in Cargo.toml");
  process.exit(1);
}
writeFileSync(cargoPath, bumped);

const pkgPath = "kursal-tauri/package.json";
const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
pkg.version = version;
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

console.log(`bumped Cargo.toml + package.json to ${version}`);
