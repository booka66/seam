# One row a declaration, from ast-grep's matches on the language rules in
# share/languages: side (the --arg), path, rule, start, end, name, head, and
# the text as code (strings and comments blanked), tab separated.
# A renderer that places something of its own in a declaration reads this too
# (otis's git-callers does, for the callers it finds outside the change).
# separated.
def clean: gsub("\\s+"; " ") | gsub("\\( "; "(") | gsub(" \\)"; ")") | gsub(", \\)"; ")") | gsub(" ,"; ",")
  | gsub("\\s*=>\\s*$"; "") | gsub("^\\s+|\\s+$"; "");
# The code alone: a name in a string or a comment is not a use of it.
# A template literal keeps its ${…} interpolations, which are code (a
# sql`…` query is mostly calls), and loses the text between them; then
# one alternation for the rest, so whichever opens first wins: the //
# in a URL string is string, the apostrophe in a comment is comment.
def code: gsub("`(?<t>(?:[^`\\\\]|\\\\.)*)`"; ([.t | scan("\\$\\{((?:[^{}]|\\{[^{}]*\\})*)\\}")] | map(.[0]) | join(" ") | " " + . + " "))
  | gsub("/\\*[\\s\\S]*?\\*/|//[^\\n]*|\"(?:[^\"\\\\\\n]|\\\\.)*\"|'(?:[^'\\\\\\n]|\\\\.)*'"; " ");
.[] | . as $m | (.text | split("\n")) as $L | .range.start.line as $s | .range.start.column as $sc
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
| [$side, (.file | sub("^\\./"; "")), $r, ($s + 1), (.range.end.line + 1),
   (.metaVariables.single.N.text // "" | gsub("\\s+"; " ")), ($head | clean),
   (if $r == "export" or $r == "comment" or $r == "method" or $r == "field" then "" else (.text | code) end)]
| @tsv
