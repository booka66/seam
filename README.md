# seam

A change, read by definition rather than by file.

```
seam HEAD~..HEAD            the definitions the change touches, as a table
seam HEAD~..HEAD --json     the same as a graph: definitions, edges, boxes
seam worktree               what is changed against HEAD, staged or not
```

seam draws nothing. It works out what a change did to the named things in it and
hands that to whatever draws: a picker, a graph, a comment on a merge request.

## The model

A **definition** is any self-contained named thing you would want to read a diff
of: a function, a class and each of its members, a type, a const. Every one has
an **interface** and a **body**, and the difference is the whole point:

- when a definition's interface changes, everything that mentions it is worth
  reading again;
- when only its body changes, nothing outside it is.

An **edge** is one definition mentioning another's name. An edge has a facet,
which is where the mention sits: in the user's own signature (`iface`), so what
it names is part of the user's interface too and the consequence carries on
outward, or in its body (`body`), where it stops. An edge is `new` when the base
side of the user did not mention the name: rewiring, as against behaviour.

Every definition has a **box** it lives in — for now its file, and what kind of
file that is (source, tests, generated, config). Each kind keeps to itself in the
graph, because a spec mentions everything it tests and a codegen dump mentions
everything, and neither should pull a source definition off the top level.

None of this comes from a model. ast-grep parses both sides of every changed file
seam has a language for, `git diff -U0` says which lines moved, and a definition
is listed when a changed line falls inside it — the comment block right above it
counting as inside. The text before the body is compared across the two sides,
and that is what "the interface changed" means. Same answer every time.

## Extending it

Three kinds of file, looked for in `./.seam/`, then `$XDG_CONFIG_HOME/seam/`,
then the ones seam ships. A file of the same name nearer the front wins, so a
repo can replace what seam ships without forking it.

**`languages/<name>.yml`** — implemented. ast-grep rules, plus `#seam:` lines
saying which extensions the language uses and which grammars to write the rules
against. Two things every rule must capture, because the rest of seam is built on
them: `$N`, the name, which is how a mention of it is found, and `$B`, the body,
so that everything before it is the interface. Adding a language is a data file
and no code; see `share/languages/typescript.yml`.

**`references/<name>`** — not yet. Inside the change seam finds edges itself. The
references that matter most come from outside it and from things seam should
never know about: a compiler, an HTTP route table, a build graph, a test name.
Those belong to whoever has the repo. The contract will be a command reading the
definition table on stdin and writing `from⇥to⇥facet⇥note` rows — `note` from the
start, because a provider that can say *this caller no longer compiles* is worth
more than one that can only say *this caller exists*.

**`containers/<name>`** — not yet. A path in, a box id out, so a definition can
sit in a package or a bazel target and not only a file.

`SEAM_KINDS` already works this way today: a command reading paths on stdin and
writing each back with its kind (`tests`, `gen`, `cfg`, or nothing for source).
Without it seam uses `share/kinds.awk`. otis passes its own, which also reads
what `.gitattributes` declares generated.

## The table

One row a definition, in reading order, members right after their class, tab
separated:

```
path  kind  name  class  exported  change  side  start  end  open line
added  removed  head  base start  base end  staged  unstaged  uses  root  new uses
```

`change` is `added`, `removed`, `signature` (the interface changed) or `body`.
`uses` and `new uses` are `path#name` lists, in the order they are mentioned, so
reading a definition's edges reads it top to bottom. `root` marks a definition
nothing else changed mentions: an entry point, where a reader should start.
`staged` and `unstaged` are the worktree's alone — what of each definition is in
the index and what is not, which is enough to stage one function out of a file
with three other things going on.

Files seam reads no definitions from get a row after the definitions, as do the
lines of a parsed file that no definition claims (an import, a top-level
statement).

## Speed

It is shell over `git`, `ast-grep` and `jq`. A real change of 95 changed
TypeScript files in a monorepo, 195 definitions, reads in **4.8s**; a synthetic
one of 6400 definitions, which nobody can review, takes 18s.

Two things cost far more than they look:

- **One huge file.** ast-grep hands back the text of every declaration it
  matches, so a single 2MB generated registry in a change became 28MB of JSON
  for a regex to walk, and nine minutes. A file over `SEAM_MAX_BYTES` (256KB)
  is listed as a plain file instead, the way one seam has no language for is.
  Nobody reads a file that size by definition anyway.
- **Comparing every pair.** Finding which definition mentions which used to test
  every definition against every other, which is the square of the change. It is
  now one pass over each definition's text against an index of names.

What is left is ast-grep's JSON and the jq pass that flattens it, which is where
the next win is if one is ever needed.

## What it wants

`git`, [`ast-grep`](https://ast-grep.github.io) (`sg`), `jq`, and a POSIX shell.

```
bin/seam-test
```

## Status

Extracted from [otis](https://github.com/booka66/otis)'s symbol picker, which is the first
thing to read a seam table and still the reason it exists. TypeScript and TSX
today; the language contract has had one implementation, so expect the second one
to find something wrong with it.
