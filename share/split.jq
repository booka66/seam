# split.jq: how to split a change, from the graph seam wrote. One slice a
# story, each a change a reviewer can take on its own, one before them for
# what they all lean on when there is such a thing, and one after for what is
# left; and for every file, which slice it goes to.
#
# A story is a connected piece of the graph: definitions that mention each
# other, with nothing changed outside them mentioning them and nothing in them
# mentioning anything changed outside. Split at the gaps between stories and
# nothing crosses the cut, so each slice reads on its own, and since each is
# closed under what it mentions, none needs another to build. The graph is
# seam's own, so this is as good as seam's edges: a mention it did not find (an
# alias, a string, a route table) is a dependency it did not see.
#
# A branch that reads as one blob usually is not one: it is a few things every
# part of it mentions (a type, an enum, a schema) holding the rest together.
# So when a piece is big, seam looks for the definition whose removal breaks
# it into the most stories, and takes it, with everything it mentions in turn,
# into a foundation that goes first. That is what a person does by hand: the
# type in one commit, then each thing that uses it.
#
# A member goes with its class: its diff sits inside the class's, and a class
# split across two slices is not a class. A definition on its own (loose, in the
# score's words) is not a story, and neither is a story made of tests: those go
# by file, with everything seam read no definitions from. A file whose
# definitions all belong to one slice goes with it whole. A test goes with the
# file it is named after (foo.spec.ts with foo.ts, test_foo.py with foo.py)
# when that file is one slice's. What nothing claims is the rest.
#
# A file two slices both changed is shared: it has to be split by line, each
# definition's lines going with its slice, and the lines no definition claims
# (an import, a top-level statement) with the first slice in it, which is the
# foundation when there is one and the largest story otherwise: the one most
# likely to want the import.

# A piece worth cutting has at least this many definitions, and a cut has to
# leave at least two stories behind while taking no more than half the piece.
def big: 6;
# A slice below this many changed lines, and below this share of the change,
# is not worth a commit of its own: a reviewer pays a context switch for every
# one, and a dozen lines does not cover it, so it goes in with the slice
# before it. Both tests, because twelve lines out of six hundred is an offcut
# and twelve out of forty is the change. Merging is always safe — two closed
# pieces taken in stack order are one closed piece — so this only ever makes
# the stack shorter, never wrong.
def small: 20;
def share: 12;

# A slice with no definition in it has no name to take, so it is named after
# what it carries: a commit subject reading "the rest" tells a reviewer
# nothing, and two file names tell them what it is.
def byfiles:
  (map(sub("^.*/"; ""))) as $n
  | if ($n | length) == 0 then "nothing"
    elif ($n | length) <= 2 then ($n | join(" and "))
    else $n[0] + " and " + (($n | length) - 1 | tostring) + " more" end;
# The directory a path sits in, and whether one path sits under another's.
def dir: if test("/") then sub("/[^/]*$"; "") else "" end;
def canon: if .class == null then .id else .path + "#" + .class end;
def lines: ((.added // 0) + (.removed // 0));
# Connected pieces of $nodes under $edges ([from, to] pairs): each node starts
# alone, and every edge merges its two ends.
def pieces($nodes; $edges):
  ($nodes | map({key: ., value: true}) | from_entries) as $in
  | reduce ($edges[] | select($in[.[0]] and $in[.[1]])) as [$x, $y] (
      {comp: ($nodes | map({key: ., value: .}) | from_entries),
       members: ($nodes | map({key: ., value: [.]}) | from_entries)};
      .comp[$x] as $a | .comp[$y] as $b
      | if $a == $b then . else
          (if (.members[$a] | length) >= (.members[$b] | length) then [$a, $b] else [$b, $a] end) as [$big, $small]
          | .members[$big] += .members[$small]
          | reduce .members[$small][] as $n (.; .comp[$n] = $big)
          | del(.members[$small])
        end)
  | .members | [.[]];
# Everything $start mentions, and what that mentions, within $in.
def reach($start; $out; $in):
  {seen: {($start): true}, q: [$start]}
  | until(.q == []; .q[0] as $x | .q = .q[1:]
      | reduce (($out[$x]) // [])[] as $y (.; if .seen[$y] or ($in[$y] | not) then . else .seen[$y] = true | .q += [$y] end))
  | .seen | keys;
def stories($ps): [$ps[] | select(length >= 2)];
# The file a test is named after: the same directory and extension, with the
# test marks taken out of the name.
def tested:
  (if test("/") then sub("/[^/]*$"; "/") else "" end) as $dir
  | (if test("/") then sub("^.*/"; "") else . end) as $base
  | ($base | split(".")) as $parts
  | if ($parts | length) < 2 then null else
      ($parts[-1]) as $ext
      | ($parts[:-1] | map(select(. as $p | ["spec", "test", "it", "e2e", "unit", "integration"] | index($p) | not))) as $stem
      | if ($stem | length) == 0 then null else
          ($stem | join(".") | sub("^test_"; "") | sub("_(test|spec)$"; "")) as $name
          | $dir + $name + "." + $ext
        end
    end;

. as $g
| ($g.definitions | map(select(.class == null))) as $D
| ($D | map({key: .id, value: .}) | from_entries) as $byid
| ($g.definitions | map({key: .id, value: canon}) | from_entries) as $canon
| ($g.edges | map([$canon[.from], $canon[.to]] | select(.[0] != .[1] and .[0] != null and .[1] != null)) | unique) as $E
| (reduce $E[] as [$a, $b] ({}; .[$a] += [$b])) as $out
| (reduce $E[] as [$a, $b] ({}; .[$b] += [$a])) as $into
| ([$D[] | select(.box.kind == "source") | .id]) as $src
# The foundation: cut the biggest pieces at the definition that breaks them
# into the most stories, until no cut does.
| (reduce range(0; 8) as $_ ({hubs: [], taken: {}, done: false};
    if .done then . else . as $st
    | ([$src[] | select($st.taken[.] | not)]) as $left
    | ($left | map({key: ., value: true}) | from_entries) as $in
    | ([pieces($left; $E)[] | select(length >= big) | . as $C
        | ($C | map({key: ., value: true}) | from_entries) as $inC
        | $C[] | select(((($into[.]) // []) | map(select($inC[.])) | length) >= 2) | . as $h
        | reach($h; $out; $inC) as $cl
        | ($C | map(select(. as $x | $cl | index($x) | not))) as $rem
        | {hub: $h, closure: $cl, stories: (stories(pieces($rem; $E)) | length), piece: ($C | length)}
        | select(.stories >= 2 and (.closure | length) * 2 <= .piece)]
       | sort_by(-.stories, (.closure | length)) | first) as $best
    | if $best == null then .done = true
      else .hubs += [$best.hub] | .taken += ($best.closure | map({key: ., value: true}) | from_entries) end
    end)) as $cut
| ([$src[] | select($cut.taken[.] | not)]) as $left
| ([pieces($left; $E)[] | select(length >= 2)
    | map($byid[.]) as $defs
    | ([$defs[] | select(((($into[.id]) // []) | map(select($cut.taken[.] | not)) | length) == 0)]
       | if length == 0 then [$defs | max_by(($out[.id] // []) | length)] else . end
       | sort_by(-lines) | map(.name)) as $roots
    | {kind: "story", roots: $roots, definitions: ($defs | sort_by(-lines) | map(.id)),
       size: ($defs | map(lines) | add)}]
   | sort_by(-.size)) as $found
| ((if ($cut.hubs | length) == 0 then [] else
      [($cut.taken | keys | map($byid[.])) as $defs
       | {kind: "base", roots: ($cut.hubs | map($byid[.].name)),
          definitions: ($defs | sort_by(-lines) | map(.id)), size: ($defs | map(lines) | add)}] end
    + $found)
   # The small ones folded into the slice before them, which for a story is
   # always a bigger one: the stories are in size order, and the base, when
   # there is one, leads.
   | (map(.size) | add // 0) as $whole
   | reduce .[] as $s ([];
       if (length > 0) and $s.kind == "story" and ($s.size < small) and ($s.size * share < $whole)
       then .[:-1] + [.[-1] + {roots: (.[-1].roots + $s.roots),
                               definitions: (.[-1].definitions + $s.definitions),
                               size: (.[-1].size + $s.size)}]
       else . + [$s] end)
   | to_entries | map(.value + {n: (.key + 1), paths: (.value.definitions | map($byid[.].path) | unique)})) as $slices
| (reduce $slices[] as $s ({}; reduce $s.definitions[] as $d (.; .[$d] = $s.n))) as $slot
| (reduce $slices[] as $s ({}; reduce $s.paths[] as $p (.; .[$p] += [$s.n]))) as $owners
# What a file needs, from the providers in references/: the slices of the
# definitions it names. A file with no definitions of its own had nothing to
# place it by and fell to the rest, which is where an import sweep went — a
# hundred files whose whole change is the import of something that moved, in
# the last commit, while the move is in the first. It goes with the last slice
# it needs, so everything it names is there when it lands.
# A sweep, from seam: three files or more whose changed lines are the same
# text, which is a rename or an import that moved and is one thing to read.
# It goes after the stories and before the rest, and only the files still
# unplaced join it — one that a reference has already put with what it needs
# stays there, since a build cares and a tidy commit does not.
| (($ARGS.named.sweeps // []) | map(map(select(($owners[.] // []) | length == 0)))
   | map(select(length >= 3))) as $sweepgroups
# A file the change has no definitions in, sitting in a directory whose
# definitions do belong somewhere: it goes with them. A BUILD file, a
# package.json, a tsconfig describes the package it sits in, and the package's
# code is right there under it — so putting it in the rest, last, is a commit
# that adds a dependency after the code that needs it. Bazel will not build
# what is between, and a typechecker resolving through node_modules cannot see
# why. Its own directory only, never further up, or a file at the root would
# claim the whole change.
#
# The earliest of them, when the code below it is spread across several: a
# dependency declared before it is used builds, and one declared after does
# not.
| (reduce (($g.references // [])[]) as $r ({};
     if $slot[$r.to] then .[$r.from] += [$slot[$r.to]] else . end)) as $needs
| (($g.definitions | map(.path)) + ($g.files | map(.path)) | unique) as $paths
# Every file: the slice it goes to and why, or the rest.
| ($slices | length) as $nstories
| ($sweepgroups | map(map(select(($needs[.] // []) | length == 0)))
   | map(select(length >= 3))) as $sweeps
| (reduce ($sweeps | to_entries)[] as $e ({};
     reduce $e.value[] as $p (.; .[$p] = ($nstories + 1 + $e.key)))) as $sweepslot
| ($nstories + ($sweeps | length) + 1) as $restn
| ([$paths[] | . as $p
    | if $owners[$p] then {path: $p, n: $owners[$p][0], shared: $owners[$p][1:], via: null}
      else (($needs[$p] // []) | max) as $needed
        | if $needed != null then {path: $p, n: $needed, shared: [], via: null, needs: true}
          elif $sweepslot[$p] then {path: $p, n: $sweepslot[$p], shared: [], via: null}
          else (($p | tested) as $t
            | if $t != null and $t != $p and (($owners[$t] // []) | length) == 1
              then {path: $p, n: $owners[$t][0], shared: [], via: $t}
              else (($p | dir)) as $d
                | (if $d == "" then null
                   else ([$owners | to_entries[] | select(.key | startswith($d + "/")) | .value[]] | min)
                   end) as $under
                | if $under != null then {path: $p, n: $under, shared: [], via: null}
                  else {path: $p, n: $restn, shared: [], via: null} end
              end)
          end
      end]) as $placed
| ($placed | map({key: .path, value: .}) | from_entries) as $where
# Lines, counted where they go: a whole file with its slice, a shared file's
# definitions each with theirs and the lines nothing claims with the first.
| ([$D[] | . as $d | ($where[$d.path]) as $w
     | {n: (if ($w.shared | length) == 0 then $w.n else ($slot[$d.id] // $w.n) end), lines: ($d | lines)}]
   + [$g.files[] | {n: $where[.path].n, lines: lines}]) as $counted
| (reduce $counted[] as $c ({}; .[$c.n | tostring] += $c.lines)) as $size
| ([$slices[] | . as $s
     | {n: $s.n, kind: $s.kind, name: $s.roots[0], roots: $s.roots, definitions: $s.definitions,
        files: [$placed[] | select(.n == $s.n) | {path, shared, via}],
        lines: ($size[$s.n | tostring] // 0)}]
   + [$sweeps | to_entries[] | . as $e | ($nstories + 1 + $e.key) as $sn
      | {n: $sn, kind: "sweep",
         name: "the same edit in " + ($e.value | length | tostring) + " files",
         roots: [], definitions: [],
         files: [$placed[] | select(.n == $sn) | {path, shared, via}],
         lines: ($size[$sn | tostring] // 0)}]
   + ([$placed[] | select(.n == $restn)] as $rest
      | if ($rest | length) == 0 then [] else
        [{n: $restn, kind: "rest", name: ($rest | map(.path) | byfiles), roots: [],
          definitions: [$D[] | select($where[.path].n == $restn) | .id],
          files: ($rest | map({path, shared, via})),
          lines: ($size[$restn | tostring] // 0)}] end)) as $slices
# For the writer: in a shared file, which slice each definition's lines go to,
# and which takes the lines no definition claims.
| ([$placed[] | select((.shared | length) > 0) | . as $w
     | {path: .path, rest: .n,
        definitions: ([$g.definitions[] | select(.path == $w.path)
          | {key: .id, value: ($slot[$canon[.id]] // $w.n)}] | from_entries)}]) as $shared
# What the code waits on, which seam cannot see: it has no language for a
# migration or a schema, so it has no edge from the code to them and puts them
# last, where nothing in the graph reaches. It does not guess which way round
# that is — a migration wants to go first and a test file wants to go last and
# both are unreadable to seam. What the repo says goes first (--first, or a
# pattern a line in .seam/first) is lifted into a slice before slice one.
# Only a file seam read no definitions from can be lifted, so no definition is
# ever taken out of the story it belongs to.
| (($ARGS.named.first // []) | map({key: ., value: true}) | from_entries) as $pull
| ([$g.files[] | select($pull[.path])]) as $lifted
| (if ($lifted | length) == 0 then {slices: $slices, shared: $shared} else
     (reduce $lifted[] as $f ({}; .[$where[$f.path].n | tostring] += ($f | lines))) as $minus
     | ([{old: 0, kind: "first", name: ([$lifted[].path] | byfiles), roots: [], definitions: [],
          files: [$slices[] | .files[] | select($pull[.path])],
          lines: ([$lifted[] | lines] | add // 0)}]
        + [$slices[] | . as $s
           | ([$s.files[] | select($pull[.path] | not)]) as $left
           # A slice named after its files is renamed when some of them have
           # been lifted out from under it, or it goes on naming what it no
           # longer carries.
           | {old: $s.n, kind: $s.kind, roots: $s.roots, definitions: $s.definitions, files: $left,
              name: (if ($s.definitions | length) == 0 then ($left | map(.path) | byfiles) else $s.name end),
              lines: ($s.lines - ($minus[$s.n | tostring] // 0))}]
        | map(select((.definitions | length) > 0 or (.files | length) > 0))) as $kept
     | ($kept | to_entries | map({key: (.value.old | tostring), value: (.key + 1)}) | from_entries) as $renum
     | {slices: [$kept | to_entries[] | .value + {n: (.key + 1)} | del(.old)
                 | .files |= map(.shared |= map($renum[. | tostring]))],
        shared: [$shared[] | .rest = $renum[.rest | tostring]
                 | .definitions |= with_entries(.value = $renum[.value | tostring])]}
   end) as $plan
# And the rest, when there is almost nothing in it. It is never a story so the
# fold above never sees it, and a single line of a build file at the end of a
# stack is a commit a reviewer opens for nothing. Same two tests, and the same
# reason it is safe: the slice before it plus it, in that order, is what the
# two of them were.
| ($plan.slices) as $ss
| (($ss | map(.lines) | add) // 0) as $whole2
| (if ($ss | length) > 1 and ($ss[-1].kind == "rest")
      and ($ss[-1].lines < small) and ($ss[-1].lines * share < $whole2)
   then ($ss[:-2] + [$ss[-2] + {files: ($ss[-2].files + $ss[-1].files),
                                lines: ($ss[-2].lines + $ss[-1].lines)}])
   else $ss end) as $slices
| ($plan.shared | map(if .rest == ($ss | length) and ($slices | length) < ($ss | length)
                      then .rest = ($slices | length) else . end)) as $shared
| if $fmt == "json" then
    {rev: $g.rev, base: $g.base, head: $g.head, slices: $slices, shared: $shared}
  else
    ("rev\t" + ($g.rev // "a change")),
    ("totals\t" + ($D | length | tostring) + " definitions · " + ($E | length | tostring) + " edges · "
       + ($paths | length | tostring) + " files → " + ($slices | length | tostring) + " slices"),
    ($slices[] | . as $s
      | ("slice\t" + ($s.n | tostring) + "\t" + $s.kind + "\t"
         + (if $s.kind == "base" then ($s.roots | join(", ")) else $s.name end)
         + "\t" + (if $s.kind == "story" and ($s.roots | length) > 1 then (($s.roots | length) - 1 | tostring) + " more" else "" end)
         + "\t" + ($s.definitions | length | tostring) + "\t" + ($s.files | length | tostring) + "\t" + ($s.lines | tostring)),
        ($s.files[] | "file\t" + ($s.n | tostring) + "\t" + .path
           + "\t" + (if (.shared | length) > 0 then "shared with " + (.shared | map(tostring) | join(", ")) else "" end)
           + "\t" + (if .via then "with " + (.via | sub("^.*/"; "")) else "" end)))
  end
