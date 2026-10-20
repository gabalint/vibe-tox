// Reports non-test source files over the coding-standards budget (about 400 lines).
// Usage: node scripts/check-file-length.mjs [limit]   Exit code 1 when any file exceeds the limit.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const limit = Number(process.argv[2] ?? 400);
const roots = ["crates", "src-tauri/src", "ui/src"];
const skipDirs = new Set(["node_modules", "target", "dist", "generated", "tests", "corpus", "test"]);
const sourceFile = /\.(rs|ts|tsx|css)$/;
const testFile = /(\.test\.tsx?$)|(_tests?\.rs$)|(^tests\.rs$)/;

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      if (!skipDirs.has(name)) yield* walk(path);
    } else if (sourceFile.test(name) && !testFile.test(name)) {
      yield path;
    }
  }
}

// Rust unit tests live in `#[cfg(test)] mod tests` blocks; count only the code above them.
function countLines(path) {
  const lines = readFileSync(path, "utf8").split("\n");
  const testStart = path.endsWith(".rs") ? lines.findIndex((l) => l.trim() === "#[cfg(test)]") : -1;
  return testStart >= 0 ? testStart : lines.length;
}

const offenders = [];
for (const root of roots) {
  try {
    for (const file of walk(root)) {
      const count = countLines(file);
      if (count > limit) offenders.push(`${relative(".", file)}: ${count} lines`);
    }
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
}

if (offenders.length > 0) {
  process.stderr.write(`Files over ${limit} lines (split by responsibility):\n${offenders.join("\n")}\n`);
  process.exit(1);
}
