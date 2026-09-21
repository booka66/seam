# order.jq: the graph on stdin, with .reading: the definitions in the order a
# reviewer takes them, as threads, and what that order leaves them carrying.
#
# A thread reads top down, a definition before what it mentions, and every
# mention of a changed definition not read yet is a loose end: a name the
# reviewer has met and not seen the change to. Reading a definition ties off
# the loose ends that lead to it and leaves one for each changed definition it
# mentions that is still to come. The order is the one that leaves the fewest
# loose at any point, as near as a step at a time gets it:
#
#   1. Nothing is unmet (read before anything that mentions it) unless it is
#      an entry point, or a circle leaves no other way in.
#   2. The fewest loose ends after it: its own unread mentions less the read
#      definitions that mention it.
#   3. The thread read last goes on: what the latest definition read mentions
#      comes before what an earlier one does. Without this the order starts
#      a second thread beside the first and carries half as many again.
#   4. A mention in a signature before one in a body, since that is where the
#      change carries outward.
#   5. The same file as the last one.
#   6. The table's order, so a change always reads the same way.
#
# Definitions no mention joins are read apart, one group through before the
# next, and each kind of file keeps to its own as it does in the graph: a
# spec mentions all it tests, which says nothing about how to read the code.
# Source first; a definition joined to nothing after the rest of its kind. A
# member goes with its class.
def rank: {source: 0, tests: 1, gen: 2, cfg: 3}[.] // 4;
def top: if test("#[^#]*\\.") then sub("\\.[^.#]*$"; "") else . end;

. as $g
| [$g.definitions[] | select(.class == null)] as $tops
| ($tops | to_entries | map({key: .value.id, value: .key}) | from_entries) as $pos
| ($tops | map({key: .id, value: .}) | from_entries) as $by
| [$g.edges[] | {from: (.from | top), to: (.to | top), facet}
   | select(.from != .to and $by[.from] and $by[.to] and $by[.from].box.kind == $by[.to].box.kind)] as $all
| ($all | map({from, to}) | unique) as $E
| ($all | map(select(.facet == "iface") | {key: "\(.from)\t\(.to)", value: true}) | from_entries) as $sig
| (reduce $E[] as $e ({}; .[$e.from] += [$e.to])) as $uses
| (reduce $E[] as $e ({}; .[$e.to] += [$e.from])) as $users
| def group($start):
    {seen: {($start): true}, queue: [$start], list: []}
    | until(.queue | length == 0;
        .queue[0] as $x | .queue |= .[1:] | .list += [$x]
        | reduce ((($uses[$x] // []) + ($users[$x] // []))[]) as $y (.;
            if .seen[$y] then . else .seen[$y] = true | .queue += [$y] end))
    | .list | sort_by($pos[.]);
  def unmet($s; $x): (($users[$x] // []) | length) > 0 and ($s.met[$x] // 0) == 0;
  # The unread definitions reached from $x along $m.
  def reach($s; $m; $x):
    {seen: {}, queue: [$x]}
    | until(.queue | length == 0;
        .queue[0] as $y | .queue |= .[1:]
        | reduce (($m[$y] // [])[] | select($s.read[.] | not)) as $z (.;
            if .seen[$z] then . else .seen[$z] = true | .queue += [$z] end))
    | .seen;
  # With every one left unmet, the way in: a circle nothing else leads to,
  # found by climbing from $x to whatever mentions it and is not mentioned
  # back, until nothing is.
  def way_in($s; $x):
    reach($s; $users; $x) as $up
    | reach($s; $uses; $x) as $down
    | ([$up | keys[] | select($down[.] | not)] | .[0]) as $z
    | if $z == null then [$x] + ($up | keys) | unique else way_in($s; $z) end;
  def thread($ids):
    {read: {}, at: {}, met: {}, order: [], unmet: [], loose: 0, most: 0, last: null,
     left: ($ids | map({key: ., value: (($uses[.] // []) | length)}) | from_entries)}
    | until(.order | length == ($ids | length);
        . as $s
        | [$ids[] | select($s.read[.] | not)] as $unread
        | [$unread[] | select(unmet($s; .) | not)] as $ready
        | (if ($ready | length) > 0 then $ready else way_in($s; $unread[0]) end)
        | min_by(. as $x | [$users[$x] // [] | .[] | select($s.read[.])] as $readers | [
            (if unmet($s; $x) then 1 else 0 end),
            $s.left[$x] - ($readers | length),
            - ([$readers[] | $s.at[.]] | max // -1),
            (if any($readers[]; $sig["\(.)\t\($x)"]) then 0 else 1 end),
            (if $s.last != null and $by[$x].path != $s.last then 1 else 0 end),
            $pos[$x]]) as $x
        | $s
        | (if unmet(.; $x) then .unmet += [$x] else . end)
        | .loose += .left[$x] - (.met[$x] // 0)
        | .read[$x] = true | .at[$x] = (.order | length) | .order += [$x] | .last = $by[$x].path
        | reduce (($users[$x] // [])[]) as $u (.; .left[$u] -= 1)
        | reduce (($uses[$x] // [])[]) as $t (.; .met[$t] += 1)
        | .most = ([.most, .loose] | max));
  (reduce $tops[].id as $id ({done: {}, groups: []};
     if .done[$id] then . else group($id) as $c | .groups += [$c] | reduce $c[] as $x (.; .done[$x] = true) end)
   | .groups | sort_by([($by[.[0]].box.kind | rank), (length == 1)])
   | map(thread(.))) as $threads
| [$threads[].order[]] as $order
| $g + {reading: {
    threads: [$threads[].order],
    unmet: [$threads[].unmet[]],
    loose: ([$threads[].most] | max // 0),
    hops: ([range(1; $order | length) | select($by[$order[.]].path != $by[$order[. - 1]].path)] | length)
  }}
