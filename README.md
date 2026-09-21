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
seam HEAD~..HEAD --gloss    Claude's few words on each definition, as json
seam HEAD~..HEAD --callers  what a lazy provider finds outside the change, as json
seam HEAD~..HEAD --md       the change by definition as markdown, for an agent
seam HEAD~..HEAD --score    what makes the change hard to read, and how hard
seam main...     --split    how to split it: one slice a story, each file placed
seam main...     --split --branch cut   those slices as commits on a new branch
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

**Claude's notes**, opt in. With `git config seam.gloss true` (and Claude Code
installed), the page also carries a few words from Haiku: what each definition does, what the change does to it, what
each new mention is for, which entry point to read first and why, and one
sentence on the whole change. The page never waits for them. It opens at once
and fills them in where they go when they land, usually within ten seconds, and
nothing you have open moves. Haiku sees only the definitions and their diffs,
has no tools, and posts nothing. The notes are kept in the repo's git dir by the
two commits, so a change is glossed once. `seam <rev> --gloss` prints them as
json for any other renderer, and `--html --gloss` is the page with them for
that run alone: asking for them either way is opting in.
`git config seam.glossModel <id>` picks another model, and the page's menu hides
them in that browser.

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

## Splitting a branch

`--split` is the score's *split the PR* line taken seriously: not how many
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
never taking more than half a piece.

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
definitions and files under it, to be rewritten. `--split --json` is the plan
as data, with the line owners of every shared file, for a picker that wants to
stage one slice rather than commit it.

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
| `references/<name>` | what names what, from outside what seam reads | a command: table in, rows out |
| `references/<name>` with `#seam: lazy` | what calls the change from outside it, and what that breaks | the same, run in the background |
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

Adding a language is a data file and no code; see `share/languages/`, where
TypeScript, Rust and Python ship.

**`first`** — implemented. A path pattern a line, matched against the files in
the change that seam read no definitions from; those go in a slice of their own
before every other, and `--split --branch` writes them as the first commit.
seam has no language for a migration, a schema or a lockfile, so it has no edge
from the code to them and puts them last, where nothing in the graph reaches —
which is the wrong way round whenever the code waits on them. It does not
guess: a migration wants to go first and a test file wants to go last, and both
are equally unreadable to it. `--first <pattern>` says the same for one run.

```
# .seam/first
prisma/*
*.sql
```

`first.local` is read before `first`, and this repo's before this user's, so a
repo that comes to ship one does not quietly take over yours. A `.local` is
the one of these git is not meant to see.

**`references/<name>`** — implemented. A command reading the definition table
on stdin and writing `from⇥to⇥facet⇥note` rows. Inside the change seam finds
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

seam ships one, `references/typescript`: for each file whose only change is its
imports, which changed definition those imports name, matched by the name and
the module's last path segment together. It reads nothing but git, errs
towards a coarser split rather than a wrong one, and a compiler would do
better, which is what replacing it is for. A file of the same name that is not
executable (an empty one will do) turns off the one seam ships.

A provider whose first lines say `#seam: lazy` is one too slow to wait for, like
a compiler finding every caller in a monorepo. seam never runs it while reading
a change: `seam <rev> --callers` does, keeps what it says in the repo's git dir
by the two commits, and from then on `--json`, `--score`, `--md` and the page
all carry it. `--html` starts it in the background and fills it in on the open
page when it lands; `--html --callers` into a file waits for it. Its rows are
callers — something outside the change that names a changed definition — with
two more columns: `from⇥to⇥facet⇥note⇥line⇥breaks`, where `line` is where
the mention is and `breaks` is `1` when the change breaks it. A `to` of `-` is
something the change breaks without it calling anything the change touched.
The page lists them under the definition, a red *breaks* on its box, and the
score gains a line: any caller the change breaks makes it *breaks callers
outside it*. A lazy provider never moves a slice, since a caller is not in the
change. One that fails is asked again next time rather than kept as having
found nothing. otis ships one over tsgo.

**`containers/<name>`** — not yet. A path in, a box id out, so a definition can
sit in a package or a bazel target and not only a file.

**`kinds`** — implemented. An executable reading paths on stdin and writing each
back with its kind (`tests`, `gen`, `cfg`, or nothing for source). Without one
seam uses `share/kinds.awk`. `SEAM_KINDS` names a command for one run, over the
file.

**`check`** — implemented. The command `--split --branch` checks every commit
with when none is named; see *Splitting a branch*.

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

It is shell over `git`, `ast-grep` and `jq`. A change of 92 files, 145
definitions, reads in **1.3s** and draws its page in 1.5s, where the two took
4.6s and 5.2s. A synthetic one of 400 files and 8000 definitions, which nobody
can review, takes 8s where it took 53s.

ast-grep's own parse is the cheap part, and it reads a directory in parallel.
What costs is the jq that flattens its JSON and the awk that reads the two
sides against the diff — and the way to make either slow is to put a loop over
the whole change inside a loop over the whole change. Five of those have been
found and taken out:

- **Which definition mentions which.** It used to test every definition against
  every other. It is now one pass over each definition's text against an index
  of names.
- **Which definition a member belongs to.** Every method and field was tried
  against every declaration on both sides. Only the ones in its own file can
  hold it, and that alone was nine tenths of the awk pass.
- **The reading order.** Roots first and then the rest, each taken as the
  smallest of everything still unplaced, which is the square of the change. It
  is a merge sort now.
- **The members of a definition.** Printing a class read the whole table again
  looking for what sat under it; members are indexed by the definition they
  belong to.
- **The lines of a definition's diff** (`--html`). Every line of every changed
  file was tried against every definition in the change rather than against the
  ones in that file, which is what a page cost to draw.

Two more that are not loops:

- **jq held the whole parse before it read any of it.** ast-grep can hand its
  matches over one a line (`--json=stream`) rather than as one array, and a
  change of 8000 definitions then costs jq four megabytes instead of a gigabyte
  and a half. Neither side needs the other, so both run at once. `share/decls.jq`
  still takes an array, for a renderer that already feeds it one.
- **One huge file.** ast-grep hands back the text of every declaration it
  matches, so a single 2MB generated registry in a change became 28MB of JSON
  for a regex to walk, and nine minutes. A file over `SEAM_MAX_BYTES` (256KB)
  is listed as a plain file instead, the way one seam has no language for is.
  Nobody reads a file that size by definition anyway.

One more that is not a loop either, and was two thirds of the jq: **a regex
that rewrites a string walks and rebuilds it once a match**, so squeezing the
whitespace out of a declaration head is the square of the spaces in it. That is
nothing for a head of a line, and a head is not always a line — a declaration
whose body is its last block puts a whole configuration object in its
interface, and one of those runs to tens of kilobytes. The spaces now go by
splitting on them, which is no regex and one pass, and only whitespace that is
not a plain space is rewritten at all.

What is left is spread thin: about half the jq that flattens ast-grep's JSON,
and the rest the awk, `git` and the two trees coming out. There is no one thing
to take out next.

## What it wants

`git`, [`ast-grep`](https://ast-grep.github.io), `jq`, and a POSIX shell.

```
bin/seam-test
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
