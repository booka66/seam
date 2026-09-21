# score.jq: how a change reads, from the graph seam wrote, as rows for seam to
# draw: kind, name, value, rating (0 ok, 1 look, 2 bad), what it counts, and
# what to do about it when it is not ok.
#
# Not a judgement of the code. Every line is a count of something that makes a
# change harder to hold in your head, with the definition to blame, so it can be
# argued with rather than trusted. The thresholds are what a reviewer can carry,
# not measurements of anything, and they are all in the rate() calls below.
#
# Where each number comes from:
#   stories   entry points with something under them: a definition nothing else
#             mentions, that mentions something that changed. One change telling
#             four stories is four reviews in one branch.
#   loose     changed definitions that mention nothing changed and are mentioned
#             by nothing changed. A change made mostly of them is a sweep.
#   knots     a cycle. seam gives every definition an entry point to be read
#             from, so an entry point something else mentions is one it had to
#             cut a cycle at.
#   chain     the longest run of definitions a reader follows from one entry
#             point, counted up to twelve.
#   carry     changed interfaces that something else names in its own signature,
#             so the change does not stop there. The widest one is the blast.
#   heaviest  the most changed lines in a single definition.
#   spread    files and directories the change touches at all.
#   safe      definitions whose body changed and nothing else, which nothing
#             outside them can break. Never rated: it is the good news.
#   breaks    callers outside the change that it breaks, from a lazy provider
#             (seam --callers), once one has been asked. Any at all is bad:
#             it is the one line here that is a bug and not a burden.

def rate($n; $look; $bad): if $n >= $bad then 2 elif $n >= $look then 1 else 0 end;
# A row: name, value, rating, what it counts, what to do, then the number and
# the two thresholds it was rated against, so a page can draw that scale.
def row($name; $value; $n; $look; $bad; $note; $advice):
  [$name, $value, rate($n; $look; $bad), $note, $advice, $n, $look, $bad];
def plural($n; $one; $many): if $n == 1 then $one else $many end;

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
| ($D | length) as $n
| [
    row("stories"; (($stories | length) | tostring) + " to follow"; ($stories | length); 4; 8;
      "entry points with something under them";
      (($stories | length) | tostring) + " threads that nothing makes you read together. Split the branch at them and each half reads on its own."),

    row("loose"; (($loose | length) | tostring) + " of " + ($n | tostring) + " on their own";
      (($loose | length) * 100 / (if $n == 0 then 1 else $n end) | floor); 40; 70;
      "changed definitions nothing else changed touches";
      "Mostly unrelated edits, so this is a sweep: read it as one pass over a list, not as a change."),

    row("knots"; (if ($knots | length) == 0 then "none" else (($knots | length) | tostring) + " cut" end);
      ($knots | length); 1; 3;
      "definitions that depend on themselves, in a circle";
      "Cut at " + ($knots | map(.name) | join(", ")) + ". Neither end can be read first, so untangle it before asking anyone to."),

    row("chain"; ($chain | tostring) + (if $chain >= 12 then "+" else "" end) + " deep"; $chain; 6; 9;
      "definitions followed from one entry point";
      "The change threads through " + ($chain | tostring) + " layers. Expect a reviewer to lose the thread before the end."),

    row("carry"; (($carry | length) | tostring) + " of " + (($ifaces | length) | tostring) + " changed or gone";
      ($carry | length); 4; 10;
      "interfaces something else names in its own signature";
      (if ($widest.names // 0) > 0
       then "Widest is " + $widest.name + ", named in " + ($widest.names | tostring) + " other " + plural($widest.names; "signature"; "signatures") + ". Read those first: the change does not stop at them."
       else "" end)),

    row("heaviest"; ($heavylines | tostring) + " lines in one"; $heavylines; 120; 300;
      "the most changed lines in a single definition";
      ($heavy.name // "nothing") + " changed " + ($heavylines | tostring) + " lines. One definition that size is not reviewable as one diff."),

    row("spread"; (($paths | length) | tostring) + " " + plural(($paths | length); "file"; "files")
        + ", " + ($dirs | tostring) + " " + plural($dirs; "directory"; "directories");
      ($paths | length); 15; 40;
      "files the change touches at all";
      "Every file is a context switch, whether or not seam read definitions from it."),

    row("safe"; (($safe | length) | tostring) + " of " + ($n | tostring) + " body only"; 0; 1000000; 2000000;
      "nothing outside them can break because of them";
      "")
  ] + (if $g.callers == null then [] else
      ($g.callers | map(select(.breaks))) as $broken
      | ($broken | map(.from) | unique) as $bf
      | [row("breaks"; (($bf | length) | tostring) + " of " + (($g.callers | map(.from) | unique | length) | tostring) + " callers outside";
          ($bf | length); 1; 1;
          "callers the change did not touch that it breaks";
          "Breaks " + ($bf[:3] | map(sub("^.*#"; "")) | join(", ")) + (if ($bf | length) > 3 then " and " + (($bf | length) - 3 | tostring) + " more" else "" end)
            + ". Fix them in this change, or say why they are fine.")]
    end) as $rows
| ([$rows[] | .[2]] | max) as $worst
| (if any($rows[]; .[0] == "breaks" and .[2] == 2) then "breaks callers outside it"
   elif $worst == 2 then "hard to review as one change"
   elif $worst == 1 then "worth reading in an order"
   else "reads as one thing" end) as $verdict
| if $fmt == "json" then
    {verdict: $verdict, worst: $worst,
     totals: {definitions: ($g.definitions | length), edges: ($E | length), files: ($paths | length)},
     rows: [$rows[] | {name: .[0], value: .[1], rating: .[2], note: .[3], advice: .[4],
                       n: .[5], look: .[6], bad: .[7]}]}
  elif $brief == "1" then
    $verdict + ([$rows[] | select(.[2] > 0) | .[0] + " " + (.[1] | split(" ") | .[0])]
                | if length == 0 then "" else " · " + join(" · ") end)
  else
    ("rev\t" + ($g.rev // "a change")),
    ("totals\t" + (($g.definitions | length) | tostring) + " definitions · "
       + (($E | length) | tostring) + " edges · " + (($paths | length) | tostring) + " files"),
    ($rows[] | "row\t" + .[0] + "\t" + .[1] + "\t" + (.[2] | tostring) + "\t" + .[3] + "\t" + .[4]),
    ("verdict\t" + $verdict + "\t" + ($worst | tostring))
  end
