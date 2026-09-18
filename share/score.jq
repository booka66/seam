# score.jq: how a change reads, from the graph seam wrote.
#
# Not a judgement of the code. Every line is a count of something that makes a
# change harder to hold in your head, printed with the number that produced it
# and the definition to blame, so it can be argued with rather than trusted. The
# thresholds are what a reviewer can carry, not measurements of anything.
#
# Where each number comes from:
#   stories   entry points with something under them: a definition nothing else
#             mentions, that mentions something that changed. One change telling
#             four stories is four reviews in one branch.
#   loose     changed definitions that mention nothing changed and are mentioned
#             by nothing changed. Each one is its own small review, and a change
#             made mostly of them is a sweep rather than a change.
#   knots     a cycle. seam gives every definition a root to be read from, so a
#             root that something else mentions is one it had to cut a cycle at.
#   chain     the longest run of definitions a reader follows from one entry
#             point, counted up to twelve.
#   carry     changed interfaces that something else names in its own signature,
#             so the change does not stop there. The widest one is the blast.
#   heaviest  the most changed lines in a single definition.
#   spread    files and directories the change touches at all, the ones seam
#             read no definitions from included.
#   safe      definitions whose body changed and nothing else, which nothing
#             outside them can break.

def w($n): tostring | (. + "                              ")[0:$n];
def verdict($v): if $v == 2 then "bad" elif $v == 1 then "look" else "ok" end;
def rate($n; $look; $bad): if $n >= $bad then 2 elif $n >= $look then 1 else 0 end;

. as $g
| ($g.definitions | map(select(.class == null))) as $D
| ($g.edges | map(select(.from != .to))) as $E
| (reduce $E[] as $e ({}; .[$e.from] += [$e.to])) as $out
| (reduce $E[] as $e ({}; .[$e.to] += [$e.from])) as $into
| (reduce ($E[] | select(.facet == "iface")) as $e ({}; .[$e.to] += [$e.from])) as $ifin
| ($D | map(.id)) as $ids
| [$D[] | select(.root and ((($into[.id]) // []) | length) == 0 and ((($out[.id]) // []) | length) > 0)] as $stories
| [$D[] | select(((($into[.id]) // []) | length) == 0 and ((($out[.id]) // []) | length) == 0)] as $loose
| [$D[] | select(.root and ((($into[.id]) // []) | length) > 0)] as $knots
| (reduce range(0; 12) as $_ (($ids | map({key: ., value: 1}) | from_entries);
     . as $d | reduce $ids[] as $n (.;
       .[$n] = ([1] + [(($out[$n]) // [])[] | (($d[.]) // 1) + 1] | max)))) as $depth
| ([0] + [$depth[]] | max) as $chain
| [$D[] | select(.change == "signature" or .change == "removed")
        | {id, name, names: ((($ifin[.id]) // []) | length)}] as $ifaces
| [$ifaces[] | select(.names > 0)] as $carry
| ($ifaces | sort_by(-.names) | first) as $widest
| ($D | sort_by(-(((.added) // 0) + ((.removed) // 0))) | first) as $heavy
| (($heavy.added // 0) + ($heavy.removed // 0)) as $heavylines
| (($g.definitions | map(.path)) + ($g.files | map(.path)) | unique) as $paths
| ($paths | map(if test("/") then sub("/[^/]*$"; "") else "." end) | unique | length) as $dirs
| [$D[] | select(.change == "body")] as $safe
| [
    ["stories",  (($stories | length) | tostring) + " to follow",
      rate(($stories | length); 4; 8),
      (if ($stories | length) <= 1 then "one thing, read top to bottom"
       else "entry points with something under them" end)],
    ["loose", (($loose | length) | tostring) + " of " + (($D | length) | tostring) + " on their own",
      rate((($loose | length) * 100 / (if ($D | length) == 0 then 1 else ($D | length) end) | floor); 40; 70),
      "changed definitions nothing else changed touches"],
    ["knots", (if ($knots | length) == 0 then "none" else (($knots | length) | tostring) + " cut" end),
      rate(($knots | length); 1; 3),
      (if ($knots | length) == 0 then "nothing in the change depends on itself"
       else "a cycle, cut at " + ($knots | map(.name) | join(", ")) end)],
    ["chain", ($chain | tostring) + (if $chain >= 12 then "+" else "" end) + " deep",
      rate($chain; 6; 9), "definitions followed from one entry point"],
    ["carry", (($carry | length) | tostring) + " of " + (($ifaces | length) | tostring) + " changed or gone",
      rate(($carry | length); 4; 10),
      (if ($widest.names // 0) > 0
       then $widest.name + " is named in " + ($widest.names | tostring) + " other signature" + (if $widest.names == 1 then "" else "s" end)
       else "no changed interface is named in another signature" end)],
    ["heaviest", ($heavylines | tostring) + " lines in one",
      rate($heavylines; 120; 300), ($heavy.name // "nothing") + ", on its own"],
    ["spread", (($paths | length) | tostring) + " file" + (if ($paths | length) == 1 then "" else "s" end)
        + ", " + ($dirs | tostring) + " director" + (if $dirs == 1 then "y" else "ies" end),
      rate(($paths | length); 15; 40), "files the change touches at all"],
    ["safe", (($safe | length) | tostring) + " of " + (($D | length) | tostring) + " body only",
      0, "nothing outside them can break because of them"]
  ] as $rows
| ([$rows[] | .[2]] | max) as $worst
| (
    "seam  " + ($g.rev // "a change") + "  ·  " + (($g.definitions | length) | tostring) + " definitions, "
      + (($E | length) | tostring) + " edges",
    "",
    ($rows[] | "  " + (.[0] | w(10)) + (.[1] | w(30)) + (verdict(.[2]) | w(6)) + .[3]),
    "",
    "  " + (if $worst == 2 then "hard to review as one change"
            elif $worst == 1 then "reviewable, with the look lines read first"
            else "reads as one thing" end)
  )
