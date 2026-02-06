import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../src/", import.meta.url));

function collectFiles(dir) {
  const entries = readdirSync(dir);
  return entries.flatMap((entry) => {
    const fullPath = join(dir, entry);
    const info = statSync(fullPath);
    if (info.isDirectory()) {
      return collectFiles(fullPath);
    }
    if (fullPath.endsWith(".js") || fullPath.endsWith(".css") || fullPath.endsWith(".html")) {
      return [fullPath];
    }
    return [];
  });
}

function lintFile(path) {
  const content = readFileSync(path, "utf8");
  const lines = content.split(/\r?\n/);
  lines.forEach((line, index) => {
    if (line.includes("\t")) {
      throw new Error(`${path}:${index + 1} contains a tab character`);
    }
    if (line.match(/\s+$/)) {
      throw new Error(`${path}:${index + 1} has trailing whitespace`);
    }
  });
}

const files = collectFiles(root);

files.forEach(lintFile);

console.log(`Linted ${files.length} files.`);
