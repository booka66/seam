// node web/check.mjs <seam.wasm> <seam> <commit>...: run from a repo, the same change
// through the browser's seam and the command line's. Their graphs must be one graph.
import { execFileSync as run } from "node:child_process";
import fs from "node:fs";

const [wasm, native, ...commits] = process.argv.slice(2);
const module = await WebAssembly.compile(fs.readFileSync(wasm));
const env = { ...process.env, XDG_CONFIG_HOME: "/nonexistent" };
delete env.SEAM_PATH;
delete env.SEAM_KINDS;
const git = (...args) => run("git", args, { maxBuffer: 2 ** 30, stdio: ["ignore", "pipe", "ignore"] });
const READS = /\.(ts|tsx|rs|py|pyi)$/;

function files(commit, parent) {
  const found = new Map();
  const put = (path, fields) => found.set(path, { ...found.get(path), path, ...fields });
  const fields = git("diff", "--raw", "--numstat", "-z", "-M", commit + "^", commit).toString().split("\0");
  for (let i = 0; i < fields.length; i++) {
    const f = fields[i];
    if (f.startsWith(":")) {
      const letter = f.split(" ")[4][0];
      if (letter === "R") {
        put(fields[++i], { status: "removed" });
        put(fields[++i], { status: "added" });
      } else put(fields[++i], { status: letter === "A" ? "added" : letter === "D" ? "removed" : "modified" });
    } else if (f.includes("\t")) {
      const [added, removed, inline] = f.split("\t");
      const path = inline || (i++, fields[++i]);
      const count = (n) => (/^\d+$/.test(n) ? +n : null);
      put(path, { added: count(added), removed: count(removed) });
    }
  }
  return [...found.values()].filter((f) => f.status).sort((a, b) => (a.path < b.path ? -1 : 1)).map((f) => {
    if (READS.test(f.path)) {
      if (f.status !== "added") f.old = git("show", `${parent}:${f.path}`).toString();
      if (f.status !== "removed") f.new = git("show", `${commit}:${f.path}`).toString();
    }
    return f;
  });
}

let differ = 0;
for (const commit of commits) {
  const parent = git("rev-parse", commit + "^").toString().trim();
  const input = Buffer.from(JSON.stringify({ rev: commit, base: parent, head: commit, files: files(commit, parent) }));
  const { exports: core } = await WebAssembly.instantiate(module, {});
  const at = core.seam_reserve(input.length);
  new Uint8Array(core.memory.buffer, at, input.length).set(input);
  const block = core.seam_read(at, input.length);
  const length = new DataView(core.memory.buffer).getUint32(block, true);
  const { score, ...inBrowser } = JSON.parse(Buffer.from(new Uint8Array(core.memory.buffer, block + 4, length)).toString());
  const onDisk = JSON.parse(run(native, [commit, "--json", "--diffs"], { env, maxBuffer: 2 ** 30 }).toString());
  if (JSON.stringify(inBrowser) !== JSON.stringify(onDisk)) {
    differ++;
    console.log("differ:", commit);
  }
}
console.log(`${commits.length} commits, ${differ} differ`);
process.exit(differ ? 1 : 0);
