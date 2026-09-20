# One row a declaration, from ast-grep's matches on the language rules in
# share/languages: side (the --arg), path, rule, start, end, name, head, and
# the text as code (strings and comments blanked), tab separated.
# A renderer that places something of its own in a declaration reads this too
# (otis's git-callers does, for the callers it finds outside the change).
# separated.
# Whitespace first, then the spaces a bracket or a comma does not want. A
# gsub of \s+ over every declaration head is most of what this file costs, and
# a head that is already single-spaced needs none of it, so it is asked first;
# what is left rebuilds from its non-space runs, which is the same answer and
# half the time. One pass takes the three spaces that go, since the fourth the
# long way round (", )") cannot survive the third.
def squeeze: if test("[\t\n\u000b\f\r]|  |^ | $") then [scan("\\S+")] | join(" ") else . end;
def clean: squeeze | gsub("(?<=\\() | (?=[,)])"; "") | gsub("\\s*=>\\s*$"; "");
# The code alone: a name in a string or a comment is not a use of it.
# What a string and a comment look like is the language's noise (a
# #seam: line in its file, passed in by extension as $langs), one
# alternation, so whichever opens first wins: the // in a URL string is
# string, the apostrophe in a comment is comment. A language that says
# nothing reads like TypeScript, where a template literal keeps its ${…}
# interpolations, which are code (a sql`…` query is mostly calls), and
# loses the text between them.
#
# Then what makes a name a use (the awk in seam reads that): a::b is a
# path, and names both; a name before : or ? is a key unless the
# language's keys say otherwise, so those it does not count go here.
def code($l): (if $l.noise == null
    then gsub("`(?<t>(?:[^`\\\\]|\\\\.)*)`"; ([.t | scan("\\$\\{((?:[^{}]|\\{[^{}]*\\})*)\\}")] | map(.[0]) | join(" ") | " " + . + " "))
      | gsub("/\\*[\\s\\S]*?\\*/|//[^\\n]*|\"(?:[^\"\\\\\\n]|\\\\.)*\"|'(?:[^'\\\\\\n]|\\\\.)*'"; " ")
    else gsub($l.noise; " ") end)
  | gsub("::"; " ")
  | ([":", "?"] - (($l.keys // ":?") | split(""))) as $plain
  | if $plain == [] then . else gsub("[" + ($plain | join("")) + "]"; " ") end;
($ARGS.named.langs // {}) as $langs
# ast-grep hands its matches over as one array (--json=compact) or one a line
# (--json=stream); seam asks for the second, which reads a change of this size
# in four megabytes rather than in one and a half gigabytes, and the first is
# taken too because a renderer already feeding this file has an array.
| (if type == "array" then .[] else . end) | . as $m | (.text | split("\n")) as $L | .range.start.line as $s | .range.start.column as $sc
# A node that takes its line's newline with it (a Rust // comment) ends on
# the line before the one its range ends on.
| (.range.end.line + (if .range.end.column == 0 and .range.end.line > $s then 0 else 1 end)) as $e
| .metaVariables.single.B as $B | .ruleId as $r
| (if $r == "export" or $r == "comment" then ""
   elif $B == null then $L[0] + (if ($L | length) > 1 then "…" else "" end)
   else ($B.range.start.line - $s) as $k
     | (if $k == 0 then $B.range.start.column - $sc else $B.range.start.column end) as $c
     | (($L[:$k] + [$L[$k][:$c]]) | join(" ")) as $pre
     | if ($r == "const" or $r == "type")
       then $pre + " " + $L[$k][$c:] + (if $B.range.end.line > $B.range.start.line then "…" else "" end)
       else $pre end
   end) as $head
# A rule with no $N (a Rust impl) is named by its interface.
| ($head | clean) as $head
| [$side, (.file | ltrimstr("./")), $r, ($s + 1), $e,
   (.metaVariables.single.N.text // $head | if test("\\s") then gsub("\\s+"; " ") else . end), $head,
   (if $r == "export" or $r == "comment" or $r == "method" or $r == "field" then ""
    else ($langs[.file | capture("(?<e>[.][^./]*)$").e // ""] // {}) as $l | .text | code($l) end)]
| @tsv
