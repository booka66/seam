# What a file is, by its path: a test, something generated, config, or source.
# Each kind keeps to itself in the graph, because a spec mentions everything it
# tests and a codegen dump mentions everything, and neither should pull a
# source definition off the top level.
#
# This is seam's fallback. A repo that knows better says so with SEAM_KINDS, a
# command that reads paths on stdin and writes each one back with a tab and its
# kind: tests, gen, cfg, or nothing for source. otis passes its own, which also
# reads what .gitattributes declares generated.
BEGIN { OFS = "\t" }
{ print $1, kindof($1) }
function kindof(p) {
  if (p ~ /\.(spec|test)\.[a-zA-Z0-9]+$/ || p ~ /(^|\/)(tests?|__tests__|spec)\//) return "tests"
  if (p ~ /(^|\/)(test_[^\/]*|[^\/]*_test|conftest)\.py$/) return "tests"
  if (generated(p)) return "gen"
  if (config(p)) return "cfg"
  return ""
}
function generated(p) {
  if (p ~ /(^|\/)(package-lock\.json|npm-shrinkwrap\.json|yarn\.lock|pnpm-lock\.yaml|bun\.lockb|Cargo\.lock|Gemfile\.lock|composer\.lock|poetry\.lock|go\.sum|flake\.lock|uv\.lock)$/) return 1
  if (p ~ /(^|\/)__generated__\//) return 1
  if (p ~ /\.(gen|generated)\.[A-Za-z0-9]+$/) return 1
  if (p ~ /\.pb\.(go|ts|js)$/ || p ~ /_pb2(_grpc)?\.pyi?$/) return 1
  if (p ~ /\.min\.(js|css)$/) return 1
  if (p ~ /\.snap$/) return 1
  return 0
}
function config(p) {
  if (p ~ /(^|\/)(BUILD|BUILD\.bazel|MODULE\.bazel|WORKSPACE|WORKSPACE\.bazel|\.bazelrc|\.bazelversion)$/) return 1
  if (p ~ /\.(bzl|bazelrc)$/) return 1
  if (p ~ /(^|\/)(package\.json|go\.mod|Cargo\.toml|pyproject\.toml|\.npmrc|\.nvmrc|\.yarnrc|\.yarnrc\.yml|\.editorconfig|\.gitignore|\.dockerignore|\.babelrc)$/) return 1
  if (p ~ /(^|\/)[tj]sconfig[^\/]*\.json$/) return 1
  if (p ~ /(^|\/)\.(eslintrc|prettierrc)[^\/]*$/) return 1
  if (p ~ /(^|\/)(eslint|prettier|vitest|vite|jest|babel)\.config\.[a-z]+$/) return 1
  return 0
}
