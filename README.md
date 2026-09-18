# seam

A change, read by definition rather than by file.

```
brew install booka66/seam/seam
```

That brings ast-grep and jq with it. Anywhere else, put `bin/` on your PATH
with `share/` beside it, and have git, ast-grep and jq installed.

```
seam HEAD~..HEAD            the definitions the change touches, as a table
seam HEAD~..HEAD --json     the same as a graph: definitions, edges, boxes
seam HEAD~..HEAD --html     that graph in a page you can click through
seam HEAD~..HEAD --score    what makes the change hard to read, and how hard
seam worktree               what is changed against HEAD, staged or not
seam --mr 1234              a GitLab merge request (or its URL), via glab
```

seam works out what a change did to the named things in it and hands that to
whatever draws: a picker, a graph, a comment on a merge request. It ships one
renderer of its own and no more.

`--html` writes a page and opens it, with two views of the same change.

The **graph**: definitions as boxes laid out left to right, each column headed
(entry points, what they use, and what that uses), each file drawn as a frame
behind the definitions in it, and an arrow for every mention. A solid arrow is a
mention in the user's own signature, so a change to what it points at carries on
outward; a dashed one is in the body, where it stops; a red dashed one is a
cycle, drawn where the layout had to cut it, so a knot shows rather than hiding.
A box is as wide as its name, since a truncated name is not a name. Click one and
everything but its neighbours dims.

The **reading order** (`g`): the same change as numbered threads, each read top
to bottom, the connector between two rows saying whether the mention was in a
signature or in a body. Every definition has a tick, every thread a progress bar,
and `space` marks one read and moves on — read state is per definition, not per
file, and lives in that browser.

Either way the panel holds the definition: what its change means in a sentence,
then its own diff, then what it uses and what uses it. `s` opens the score,
drag to pan, wheel to zoom, `/` to filter.

It is one self-contained file with the graph inside it, so it works offline and
can be sent to someone. The same page (`share/view.html`) opened on its own takes
a graph dropped or pasted onto it.

One warning worth knowing about: `main..` is the two trees, which is only your
branch while `main` is behind you. Once main has moved on, that diff is your
change *and the undoing of every commit main has that you do not* — 21 files read
as 1399. seam says so when it happens and points at `main...`, which is the
branch alone.

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

## Reading the score

`--score` is not a quality grade, and there is no single number: one number hides
which of eight different problems you have, and rewards gaming whichever one is
cheapest to move. Each line is a count, with a coloured bar for its rating and,
under anything that is not ok, what to do about it:

```
  seam  main...
  26 definitions · 36 edges · 21 files

  ▌ stories   7 to follow              entry points with something under them
      7 threads that nothing makes you read together. Split the branch at them
      and each half reads on its own.

  ▌ loose     1 of 26 on their own     changed definitions nothing else changed touches
  ▌ knots     none                     definitions that depend on themselves, in a circle
  ▌ chain     7 deep                   definitions followed from one entry point
      The change threads through 7 layers. Expect a reviewer to lose the thread
      before the end.

  ▌ carry     2 of 7 changed or gone   interfaces something else names in its own signature
  ▌ heaviest  64 lines in one          the most changed lines in a single definition
  ▌ spread    21 files, 13 directories files the change touches at all

  ▌ worth reading in an order
```

The bar is green, amber or red. Colour comes on for a terminal, and `SEAM_COLOR=1`
turns it on for a pager. `--brief` says the same in one line, for a footer or a
CI comment, and `--table <file>` scores a table already built instead of reading
the trees again — which is how otis's picker shows this under `S` without paying
for it twice.

The verdict is the worst line, not an average, because a change with one terrible
dimension is hard to review however tidy the rest is. What each line means, and
why it is worth counting:

- **stories** — entry points with something under them. Four stories in one
  branch is four reviews. This is the line that says *split the PR*.
- **loose** — definitions nothing else in the change touches. A change made
  mostly of these is a sweep (a rename, a lint fix) rather than a change; that is
  fine, but it wants reviewing as one.
- **knots** — a cycle. seam gives every definition an entry point to be read
  from, so an entry point that something else mentions is one it had to cut a
  cycle at. Not a style opinion: you cannot read either of those definitions
  first.
- **chain** — the longest run a reader follows from one entry point.
- **carry** — changed interfaces that something else names in *its own*
  signature, so the change does not stop there. The widest one is the blast
  radius, and the best guess at what a reviewer will miss.
- **heaviest** — the most changed lines in a single definition.
- **spread** — files and directories, the ones seam read no definitions from
  included.
- **safe** — body-only changes, which nothing outside them can break. A change
  that is mostly this is the cheap kind, and it is never rated.

The thresholds are what a reviewer can carry, not measurements of anything, and
they sit at the top of `share/score.jq` to be argued with.

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

`git`, [`ast-grep`](https://ast-grep.github.io), `jq`, and a POSIX shell.

```
bin/seam-test
```

## Status

Extracted from [otis](https://github.com/booka66/otis)'s symbol picker, which is the first
thing to read a seam table and still the reason it exists. TypeScript and TSX
today; the language contract has had one implementation, so expect the second one
to find something wrong with it.
