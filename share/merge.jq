# merge.jq: a plan on stdin, the slice numbers in $fuse fused into one.
#
# The check is the only thing here that knows whether a stack stands up, so
# when it says slice one fails and names files slice nine has, those two are
# one commit and the plan was wrong. They fuse at the earlier of the two — the
# later slice's content moves up to where it is needed — and everything after
# renumbers. What that breaks, the next check catches; this only ever makes
# the stack shorter, and a shorter stack is one the change did not have.
($fuse | map(tonumber) | unique) as $g
| $g[0] as $into
| ([.slices[] | select(.n | IN($g[]))] | {
     definitions: (map(.definitions) | add),
     files: (map(.files) | add),
     roots: (map(.roots) | add),
     lines: (map(.lines) | add)
   }) as $one
| [.slices[] | select((.n | IN($g[])) | not)] as $rest
| ([.slices[] | select(.n == $into)][0]) as $host
| (($rest + [$host + $one]) | sort_by(.n)) as $merged
# A slice keeps its own numbering long enough to be sorted, then takes the
# place it ended up in, and everything that named a slice by number follows.
| ($merged | to_entries | map({key: (.value.n | tostring), value: (.key + 1)}) | from_entries) as $renum
| (reduce $g[] as $x ($renum; .[$x | tostring] = $renum[$into | tostring])) as $renum
| .slices = [$merged | to_entries[] | .value + {n: (.key + 1)}
             | .files |= map(.shared |= (map($renum[. | tostring]) | unique | map(select(. != null))))]
| .shared = [.shared[] | .rest = ($renum[.rest | tostring] // .rest)
             | .definitions |= with_entries(.value = ($renum[.value | tostring] // .value))]
