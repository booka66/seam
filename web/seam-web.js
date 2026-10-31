// seam in a browser: a GitHub pull request, commit or compare URL in, the page out.
// The files come from GitHub and the reading is seam.wasm, the same core the
// command line runs, so nothing leaves the browser. Public repos only: GitHub's API
// allows sixty asks an hour without a token. A link can carry the URL after a #.
(function () {
  // What is worth fetching; which of these seam reads is the core's to say.
  var READS = /\.(ts|tsx|mts|cts|rs|py|pyi)$/;

  function parse(url) {
    var m = String(url).trim().match(/github\.com\/([^/]+)\/([^/]+)\/(pull|commit|compare)\/([^?#]+)/);
    if (!m) return null;
    return { owner: m[1], repo: m[2], what: m[3], ref: m[4].replace(/\/(files|commits|changes)(\/.*)?$/, "").replace(/\/$/, "") };
  }

  function api(where, path) {
    return fetch("https://api.github.com/repos/" + where.owner + "/" + where.repo + path, { headers: { Accept: "application/vnd.github+json" } }).then(function (r) {
      if (r.status === 403 || r.status === 429) throw new Error("GitHub says no more asks this hour without a token.");
      if (!r.ok) throw new Error("GitHub said " + r.status + " for " + path + ". Is the repo public?");
      return r.json();
    });
  }

  // The two ends and the files, whichever kind of URL it was.
  function change(where) {
    if (where.what === "commit") {
      return api(where, "/commits/" + where.ref).then(function (c) {
        return { rev: c.sha.slice(0, 11), base: c.parents.length ? c.parents[0].sha : null, head: c.sha, files: c.files };
      });
    }
    var range = where.what === "compare" ? Promise.resolve(where.ref) : api(where, "/pulls/" + where.ref).then(function (p) { return p.base.sha + "..." + p.head.sha; });
    return range.then(function (r) {
      return api(where, "/compare/" + r).then(function (c) {
        return { rev: where.what === "pull" ? "#" + where.ref : r, base: c.merge_base_commit.sha, head: c.commits.length ? c.commits[c.commits.length - 1].sha : c.merge_base_commit.sha, files: c.files };
      });
    });
  }

  function text(where, sha, path) {
    var encoded = path.split("/").map(encodeURIComponent).join("/");
    return fetch("https://raw.githubusercontent.com/" + where.owner + "/" + where.repo + "/" + sha + "/" + encoded).then(function (r) {
      if (!r.ok) throw new Error("GitHub said " + r.status + " for " + path + ".");
      return r.text();
    });
  }

  // A rename is the file that went and the file that came, since a definition has
  // no idea it moved.
  function input(where, c) {
    var entries = [];
    c.files.forEach(function (f) {
      if (f.status === "renamed") {
        entries.push({ path: f.previous_filename, status: "removed" });
        entries.push({ path: f.filename, status: "added", added: f.additions, removed: f.deletions });
      } else {
        entries.push({ path: f.filename, status: f.status === "added" || f.status === "removed" ? f.status : "modified", added: f.additions, removed: f.deletions });
      }
    });
    return Promise.all(entries.map(function (e) {
      if (!READS.test(e.path)) return e;
      return Promise.all([
        e.status === "added" || !c.base ? null : text(where, c.base, e.path),
        e.status === "removed" ? null : text(where, c.head, e.path),
      ]).then(function (both) { e.old = both[0]; e.new = both[1]; return e; });
    })).then(function (files) {
      return { rev: c.rev, base: c.base, head: c.head, files: files };
    });
  }

  var core = null, here = document.currentScript.src;
  function wasm() {
    core = core || fetch(new URL("seam.wasm", here)).then(function (r) { return r.arrayBuffer(); }).then(function (bytes) {
      return WebAssembly.instantiate(bytes, {});
    }).then(function (m) { return m.instance.exports; });
    return core;
  }

  function read(exports, change) {
    var bytes = new TextEncoder().encode(JSON.stringify(change));
    var p = exports.seam_reserve(bytes.length);
    new Uint8Array(exports.memory.buffer, p, bytes.length).set(bytes);
    var block = exports.seam_read(p, bytes.length);
    exports.seam_free(p, bytes.length);
    var n = new DataView(exports.memory.buffer).getUint32(block, true);
    var answer = new TextDecoder().decode(new Uint8Array(exports.memory.buffer, block + 4, n));
    exports.seam_free(block, n + 4);
    return answer;
  }

  function open(url, say) {
    var where = parse(url);
    if (!where) { say("That is not a GitHub pull request, commit or compare URL."); return; }
    say("Reading " + where.owner + "/" + where.repo + "…");
    Promise.all([wasm(), change(where).then(function (c) { return input(where, c); })]).then(function (both) {
      var answer = read(both[0], both[1]);
      var parsed = JSON.parse(answer);
      if (parsed.error) throw new Error(parsed.error);
      window.seamLoad(answer);
    }).catch(function (err) { say(String(err.message || err)); });
  }

  window.seamRead = function (change) { return wasm().then(function (e) { return read(e, change); }); };

  var drop = document.getElementById("drop");
  if (!drop) return;
  var form = document.createElement("form");
  var field = document.createElement("input");
  field.placeholder = "https://github.com/owner/repo/pull/123";
  field.style.cssText = "width:100%;box-sizing:border-box;padding:10px;font:inherit;color:inherit;background:transparent;border:1px solid var(--line)";
  var note = document.createElement("p");
  note.className = "hint";
  note.textContent = "or a public GitHub pull request, commit or compare URL, read right here:";
  var said = document.createElement("p");
  said.className = "hint";
  form.appendChild(note); form.appendChild(field); form.appendChild(said);
  drop.appendChild(form);
  function say(t) { said.textContent = t; }
  form.addEventListener("submit", function (e) { e.preventDefault(); open(field.value, say); });
  if (location.hash.length > 1) open(decodeURIComponent(location.hash.slice(1)), say);
})();
