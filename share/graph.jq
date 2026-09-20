# graph.jq: seam's table on stdin, as the graph a renderer draws.
#
# A definition a row, an edge a mention of one definition's name inside
# another's text. An edge carries its facet: where the mention sits. In the
# user's own signature (iface) a change to what it names changes the user's
# interface too, so it carries on outward; in its body (body) it stops there.
# `new` says the base side of the user did not mention the name: rewiring,
# where the rest is behaviour.
#
# The head of a const or a type keeps the first line of its body (there is no
# signature to speak of), so a mention on that line reads as iface.
def cells: split("\n") | map(select(length > 0) | split("\t"));
def num: if . == null or . == "" then null else tonumber end;
def flag: if . == null or . == "" then null else . == "1" end;
def esc: gsub("(?<c>[.$^|()\\[\\]{}*+?\\\\-])"; "\\" + .c);
def bare: sub("^type "; "");
def mentions($n): test("(^|[^A-Za-z0-9_$.])" + ($n | esc) + "([^A-Za-z0-9_$]|$)");
def defid: .[0] + "#" + (if (.[3] // "") == "" then .[2] else .[3] + "." + .[2] end);

($kinds | cells | map({key: .[0], value: (.[1] // "")}) | from_entries) as $kind
| cells as $rows
| def box($p): {file: $p, kind: (if ($kind[$p] // "") == "" then "source" else $kind[$p] end)};
  ($rows | map(select(.[1] != "file"))) as $defs
| ([$defs[] | defid] | map({key: ., value: true}) | from_entries) as $isdef
| ($refs | cells | map(select(length >= 2) | {
    from: .[0], to: .[1],
    facet: (if .[2] == "iface" then "iface" else "body" end),
    note: (if (.[3] // "") == "" then null else .[3] end),
    via: (.[4] // null)
  })) as $rs
| {
  rev: $rev,
  base: (if $base == "" then null else $base end),
  head: (if $head == "" then null else $head end),
  definitions: ($defs | map({
    id: defid,
    path: .[0], kind: .[1], name: .[2],
    class: (if (.[3] // "") == "" then null else .[3] end),
    exported: (.[4] | flag),
    change: .[5],
    side: .[6],
    start: (.[7] | num), end: (.[8] | num), open: (.[9] | num),
    added: (.[10] | num), removed: (.[11] | num),
    head: .[12],
    base: (if (.[13] // "") == "" then null else {start: (.[13] | num), end: (.[14] | num)} end),
    staged: (.[15] | flag), unstaged: (.[16] | flag),
    root: ((.[18] // "") == "1"),
    box: box(.[0])
  })),
  # What a provider in references/ said, kept apart from the edges seam found
  # itself: an end of one may be a file the change has no definitions in, which
  # is no node in this graph but is still a thing that has to move with what it
  # names. Every one carries the provider that said it and whatever note it
  # wrote, which is the whole point of asking something that can compile.
  references: $rs,
  edges: [ $defs[] | . as $r
    | (if ($r[17] // "") == "" then [] else ($r[17] | split("|")) end)[]
    | . as $to
    | ($to | sub("^[^#]*#"; "") | bare) as $n
    | {from: ($r | defid), to: $to,
       facet: (if (($r[12] // "") | mentions($n)) then "iface" else "body" end),
       new: ((if ($r[19] // "") == "" then [] else ($r[19] | split("|")) end) | index($to) != null)}
  ]
  # A reference with a definition at both ends is an edge seam should have
  # had, so it joins the ones it found, with what said it.
  + ($rs | map(select($isdef[.from] and $isdef[.to] and .from != .to) | . + {new: false})),
  files: ($rows | map(select(.[1] == "file") | {
    path: .[0], change: .[5], added: (.[10] | num), removed: (.[11] | num), box: box(.[0])
  }))
}
