# seam

A change, read by definition rather than by file.

```
curl -fsSL https://raw.githubusercontent.com/booka66/seam/main/install.sh | sh
```

That puts the newest release in `~/.local/share/seam` and a `seam` that runs it
in `~/.local/bin`, which it puts on your PATH; run it again to update. seam is one
program and needs only git; `--split` also wants jq.

```
seam HEAD~..HEAD            the definitions the change touches, as a table
seam HEAD~..HEAD --json     the same as a graph: definitions, edges, boxes
seam HEAD~..HEAD --html     that graph in a page you can click through
seam HEAD~..HEAD --score    what makes the change hard to read, and how hard
seam main...     --split    how to split it: one slice a story, each file placed
seam main...     --split --branch cut   those slices as commits on a new branch
seam worktree               what is changed against HEAD, staged or not
```

Or read a public GitHub pull request in a browser, with nothing installed: see
[In a browser](#in-a-browser).

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
signature or in a body. A mention of a definition not read yet is a loose end,
something to keep in mind, and each step takes whatever leaves the fewest
loose; on a tie, what the last definition read mentions before what an earlier
one does, then a signature mention before a body one. The page says how many that is at the worst point, and
`--json` carries the order as `reading` (`src/order.rs` has the rules). Every
definition has a tick, every thread a progress bar,
and `space` marks one read and moves on — read state is per definition, not per
file, and lives in that browser.

Either way the panel holds the definition: what its change means in a sentence,
then its own diff, then what it uses and what uses it. The diff reads the way
delta does in a terminal: gruvbox, syntax coloured, both line numbers, the words
that changed on a darker ground than their line, and unchanged runs folded. Every
mention of another changed definition is underlined in the colour of how that one
changed (solid where the change added the mention), and hovering one, a row under
Uses, or a box in the graph opens a card: its signature, how it changed, and
whether the mention is in a signature or a body. Click to go to it. With nothing picked it
holds the change itself — what it is made of, and which definitions carry
furthest, which is the best guess at what a reviewer will miss. `s` opens the
score, drag to pan, wheel to zoom, `/` to filter, and the rest is behind the
one button at the end of the bar.

**What else is known.** seam asks nobody anything, but `--with <file>` merges a JSON
object into the graph before anything draws it, so what a model or a compiler knows
reaches the page, `--json` and `--score`. `callers` lists what outside the change
names a changed definition, and the score gains a *breaks* line. `gloss` puts a few
words on each definition. otis's `otis-seam` fills both, Haiku's notes and tsgo's
callers, and what is not ready yet it names as `glossFrom` or `callersFrom`, files
beside the page, which fill in when they land. `--json --diffs` carries each
definition's own diff, for whatever writes a prompt from it.

`--html --at <id>` opens the page with that definition picked, for a renderer
that wants to hand you from its own view to this one: the id is the one `--json`
gives it, `path#name`, or `path#Class.name` for a member.

It is one self-contained file with the graph inside it, so it works offline and
can be sent to someone. The same page (`share/view.html`) opened on its own takes
a graph dropped or pasted onto it.

One warning worth knowing about: `main..` is the two trees, which is only your
branch while `main` is behind you. Once main has moved on, that diff is your
change *and the undoing of every commit main has that you do not* — 21 files read
as 1399. seam says so when it happens and points at `main...`, which is the
branch alone.

## In a browser

`web/build.sh` makes `web/dist`, a folder any static host can serve: the same page,
and `seam.wasm`, the same core the command line runs. Give it a public GitHub pull
request, commit or compare URL, in its box or after a `#` in its link, and it fetches
the files and reads them where you are. Nothing leaves the browser and nothing is
installed. GitHub allows sixty asks an hour without a token, and private repos are
not read. `node web/check.mjs` holds the two to one graph.

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

None of this comes from a model. tree-sitter parses both sides of every changed
file seam has a language for, ast-grep's rules pick the definitions out, a line diff
says which lines moved, and a definition is listed when a changed line falls inside it — the comment block right above it
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
they sit in `src/score.rs` to be argued with.

## Splitting a branch

`seam --split` is `bin/seam-split`, a program of its own because it is the part of
seam that writes commits and runs what a repo asks it to: it reads the change
through seam, and wants jq. It is the score's *split the PR* line taken seriously: not how many
stories there are but which definition and which file goes in which one, and
`--branch <name>` lays them down as commits.

`--check '<cmd>'` runs a command on every commit as it is made and writes the
branch only if every one of them passes — and when one does not, it fuses that
slice with the slices it was waiting for and builds the whole thing again, up
to six times. The check is the only thing here that knows whether a stack
stands up, so it is the thing allowed to correct the plan: seam's edges are
names it found inside changed definitions, and a type that ripples through
files which never name it is exactly what it cannot see. What a fuse breaks
the next pass catches, and a fuse only ever makes the stack shorter, so it
ends. A change that ends up one commit was one commit, and that is an answer
nobody had before — a proved one.
Without it only the last commit is checked, against the head tree, so a stack
whose middle did not build looked exactly like one that did. The command is
told which slice it is on (`SEAM_SLICE`, `SEAM_SLICES`, `SEAM_COMMIT`), and
where the two ends, the original worktree and the table this plan was made
from are (`SEAM_BASE`, `SEAM_HEAD`, `SEAM_ROOT`, `SEAM_TABLE`). One worktree serves the whole stack and moves from commit to
commit, so an install done for the first slice is still there for the second;
what a check needs before it can run is its own business. A check that would
rather not stand in a worktree at all — asking a language server what the tree
at that commit would say, from where the dependencies already are — has
`SEAM_ROOT` and `SEAM_COMMIT` and needs nothing else.

A repo names its check once, in `.seam/check` (a command, the first line that
is not a comment), and `--branch` runs it without being told; `check.local` is
read first and this user's after, the way `first` is. `--check ''` runs none.

A file the change has no definitions in goes with the package it sits over: a
BUILD file, a `package.json`, a `tsconfig` describes the code in its own
directory, and the code is right there under it. Its own directory only, or a
file at the root would claim the whole change — and the earliest slice under
it, since a dependency declared before it is used builds and one declared
after does not. Putting such a file last, which is what the rest means, is a
commit that adds a dependency after the code that needs it: a build graph will
not build what is between, and a typechecker resolving through `node_modules`
cannot see why.

A **sweep** is three files or more whose changed lines are the same text: a
rename, a lint fix, an import that moved. Their files have no definitions seam
can read, so one at a time they all fall to the rest; together they are one
thing to read, and they get a slice. It reads the diff of the two trees, which
`--table` does not change, so a plan built from a table already made has it
too; the worktree, which has no head commit to diff, does not.

```
  seam  main...
  60 definitions · 93 edges · 92 files → 8 slices

   1  Mode  7 definitions · 5 files · 67 lines
      what the other slices lean on, so it goes first
      hub.ts
      ...

   2  fa1 +3  27 definitions · 16 files · 584 lines
      a.spec.ts  with a.ts
      a.ts
      shared.ts  shared with 3
      ...

   8  the rest  62 files · 219 lines
      ...
```

A **story** is a connected piece of the graph: definitions that mention each
other, with nothing changed outside them mentioning them and nothing in them
mentioning anything changed outside. Cut between two stories and nothing
crosses the cut, so each slice reads on its own, and since each is closed under
what it mentions, none needs another to build. That is only as good as seam's
edges: a mention it did not find (an alias, a string, a route table) is a
dependency it did not see, and a branch built by `--branch` is worth building
before it is worth pushing.

A branch that reads as one blob usually is not one. It is a type, an enum or a
schema that every part mentions holding the rest together, and the score's
*15 stories* are fifteen entry points into one piece. So when a piece has six
or more definitions, seam looks for the one whose removal breaks it into the
most stories, and takes it, with everything it mentions in turn, into a first
slice the others lean on. That is what a person does by hand: the type in one
commit, then each thing that uses it. It cuts again while a cut still helps,
never taking more than half a piece. A foundation too small to be worth a
commit, by the same two tests that fold a small story into the one before
it, goes into the first slice that leans on it instead, since nothing before
that one needs it.

Then files. A member goes with its class. A definition on its own is not a
story, and neither is a story made of tests, so those go by file: a file whose
definitions all belong to one slice goes with it whole, a test goes with the
file it is named after (`foo.spec.ts` with `foo.ts`, `test_foo.py` with
`foo.py`) when that file is one slice's, and what nothing claims is **the
rest**, last. A file two slices both changed is **shared**: it is split by
line, each definition's lines going with its slice and the lines no definition
claims (an import, a top-level statement) with the first slice in it, which is
the one most likely to want the import. The plan says so against the file, so
it can be argued with before anything is written.

`--branch <name>` makes one commit a slice, in that order, on a new branch
from the base. Nothing is checked out and the worktree is not touched: each
commit is built in an index of its own, a whole file from the head, a shared
file rebuilt from the diff for every slice short of its last. The last commit
is checked against the head's tree, so a slice that did not add up cannot pass
quietly. Commit messages are the entry point's name with the slice's
definitions and files under it, to be rewritten — unless the slice is one of
the branch's own commits (below), when it is that commit's message. `--split --json` is the plan
as data, with the line owners of every shared file, for a picker that wants to
stage one slice rather than commit it.

A branch of more than one commit has been split once already, by someone who
knew what no diff says: that one part is a fix to ship today and another waits
on someone else's MR. seam blames every changed line on the commit that made
it (`git blame`, and `--reverse` for a line taken out) and reads those commits
three ways:

- A definition lands at the first commit that touched it, or after anything
  it newly mentions. So a story the author built over two commits is two
  slices in their order, and a dependency seam has no edge for — a field read
  in one definition and added in another, which neither names — holds,
  because the commit that reads it came after the one that added it. A
  mention the base already had holds nothing back: the author's commit built
  without the change to what it names, and the check says when it did not.
  A later fixup to a definition does not drag what uses it along.
- Stories nothing connects go in commit order, not by size.
- A file nothing else places goes with its commit's slice, before the
  directory rule.

A slice that holds all of a commit, and mostly that commit, says what the
commit said, subject and body, so a stack that mirrors the branch needs no
rewording. A commit that went to two slices is claimed by neither, since its
message would describe code one of them does not have. A fuse drops it, since the slice is then more than that commit. One
commit, or the worktree, says none of this and the plan is what it was.

## Extending it

seam stays small by asking the repo for everything it cannot know, and every
question is a file: looked for in `./.seam/`, then `$XDG_CONFIG_HOME/seam/`,
then the ones seam ships. A file of the same name nearer the front wins, so a
repo can replace what seam ships without forking it, and bring its own
compiler, build graph or test runner without seam knowing what any of them are.
`SEAM_PATH` is a colon-separated list of more directories laid out the same
way, read after yours and before seam's own: that is how a tool built on seam
brings its providers without writing into anyone's config (otis exports its
`share/seam`).

| file | what it answers | shape |
|---|---|---|
| `languages/<name>.yml` | what a definition is | ast-grep rules |
| `references/<name>` | what names what, for a split, from outside what seam reads | a command: table in, rows out |
| `references/<name>` with `#seam: split` | what one changed definition needs of another, where no name says | the same, run only to split |
| `kinds` | which files are tests, generated or config | a command: paths in, `path⇥kind` out |
| `first` | what goes before everything in a split | path patterns |
| `check` | whether a commit of the split stands up | a command |

**`languages/<name>.yml`** — implemented. ast-grep rules, plus `#seam:` lines
saying which extensions the language uses and which grammars to write the rules
against. Two things every rule must capture, because the rest of seam is built on
them: `$N`, the name, which is how a mention of it is found, and `$B`, the body,
so that everything before it is the interface. A rule with no `$N` is named by
its interface, which is how a Rust `impl Display for Foo` gets a name. A `method`
or `field` is a member of whatever definition it sits inside.

Two more `#seam:` lines say how to read the text for names. `noise` is a regex
for what is not code (comments and strings), and `keys` is which of `:` and `?`
make the name before them a key rather than a use (`:?` in TypeScript, `:` in
Rust, none in Python). A language that says neither reads like TypeScript. A
name is only looked for among definitions of its own language.

`share/languages/` has TypeScript, Rust and Python. A language file can read any
grammar seam is built with (`typescript`, `tsx`, `rust`, `python`) and needs no
code. A new grammar is a dependency and a line in `src/lang.rs`.

**`first`** — implemented. A path pattern a line, matched against the files in
the change that seam read no definitions from; those go in a slice of their own
before every other, and `--split --branch` writes them as the first commit.
seam has no language for a migration, a schema or a lockfile, so it has no edge
from the code to them and puts them last, where nothing in the graph reaches —
which is the wrong way round whenever the code waits on them. It does not
guess: a migration wants to go first and a test file wants to go last, and both
are equally unreadable to it.

```
# .seam/first
prisma/*
*.sql
```

`first.local` is read before `first`, and this repo's before this user's, so a
repo that comes to ship one does not quietly take over yours. A `.local` is
the one of these git is not meant to see.

**`references/<name>`** — implemented. A command reading the definition table
on stdin and writing `from⇥to⇥facet⇥note` rows. Only the split asks. Inside the change seam finds
edges itself; the ones that matter most come from outside it and from things
seam should never know about — a compiler, an HTTP route table, a build graph,
a test name. An end is a definition (`path#name`, `path#Class.member`) or a
plain path, for a file the change has no definitions in. That last is what an
import sweep is made of: without a provider a hundred files whose whole change
is an import fall to the rest, in the last commit, while the thing they import
moved in the first. With one they go with what they name, and `--split` reads
the sweep as the one thing it is. A provider that fails says nothing; seam
worked before it existed and works if it breaks. The two trees are in its
environment as `SEAM_REV`, `SEAM_BASE` and `SEAM_HEAD` (the last empty for the
worktree), since what the change did to a *line* is most of what a provider
wants and the table carries only what it did to a definition. `note` is why it is worth
asking something that can compile — *this caller no longer compiles* is worth
more than *this caller exists*.

seam ships one, `references/typescript`: for each file seam read no
definitions from, which changed definition its changed import statements name
(an import the change left alone places nothing), matched by the name and
the module's last path segment together. It reads nothing but git, errs
towards a coarser split rather than a wrong one, and a compiler would do
better, which is what replacing it is for. A file of the same name that is not
executable (an empty one will do) turns off the one seam ships.

A provider whose first lines say `#seam: split` is one too slow for every read
but worth waiting for when its answer moves a slice. Only `--split` runs it,
and what it says is kept by the two commits, like the callers. A row from one
changed definition to another is an edge to the split, and one the change
made: it joins stories and holds the one that needs the other after it. That
is the dependency seam's names cannot see — a definition reading a field
another one added, which neither names — and what a compiler can.

**`kinds`** — implemented. An executable reading paths on stdin and writing each
back with its kind (`tests`, `gen`, `cfg`, or nothing for source). Without one
seam goes by the path (`src/kinds.rs`). `SEAM_KINDS` names a command for one run, over the
file.

**`check`** — implemented. The command `--split --branch` checks every commit
with when none is named; see *Splitting a branch*.

## The table

One row a definition, entry points first and then by file and line, members
right after their class, tab separated:

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

It is one program that asks git for the bytes and does the rest itself. On real
commits of a large repo (8.4GB of `.git`), in milliseconds, against the shell over
`ast-grep`, `jq` and `awk` it replaced:

| change | table | score | page |
|---|---|---|---|
| 1 file | 459 → 43 | 498 → 41 | 553 → 38 |
| 14 files | 543 → 70 | 597 → 70 | 741 → 71 |
| 43 files | 1612 → 78 | 1693 → 89 | 2003 → 91 |
| 120 files | 777 → 96 | 819 → 98 | 1068 → 102 |

What is left is mostly git finding the blobs in its packs. The program is 7MB with
its four grammars, where `ast-grep` alone was 49MB, and `seam.wasm` is 773KB over the
wire. Over 130 commits and 9342 rows the line diff, which is imara-diff's and not
git's, placed a line differently in 9 commits: counts a line or two apart, and one
definition seam no longer lists.

## What it wants

`git`, and `jq` for `--split`. To build it, `cargo build --release`, and for the
browser's, zig and the `wasm32-unknown-unknown` target.

```
bin/seam-test
cargo test
```

## Status

Extracted from [otis](https://github.com/booka66/otis)'s symbol picker, which is the first
thing to read a seam table and still the reason it exists. TypeScript and TSX,
Rust and Python today. The second and third found what the first had baked in:
comment and string syntax, what a colon means, members belonging only to a
`class`, and names matching across languages.

What they do not do yet: items inside an inline Rust `mod` are not listed (it is
nearly always `mod tests`, which would pull everything it tests off the top
level), so its lines count on the file row; and a struct or class gaining a field
reads as a body change of the struct, with the field listed under it as added.
