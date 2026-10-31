//! What a file is by its path: a test, something generated, config, or source
//! (the empty string). Each kind keeps to itself in the graph, because a spec
//! mentions everything it tests and a codegen dump mentions everything.
use regex::RegexSet;
use std::sync::OnceLock;

const TESTS: &[&str] = &[
    r"\.(spec|test)\.[a-zA-Z0-9]+$",
    r"(^|/)(tests?|__tests__|spec)/",
    r"(^|/)(test_[^/]*|[^/]*_test|conftest)\.py$",
];
const GENERATED: &[&str] = &[
    r"(^|/)(package-lock\.json|npm-shrinkwrap\.json|yarn\.lock|pnpm-lock\.yaml|bun\.lockb|Cargo\.lock|Gemfile\.lock|composer\.lock|poetry\.lock|go\.sum|flake\.lock|uv\.lock)$",
    r"(^|/)__generated__/",
    r"\.(gen|generated)\.[A-Za-z0-9]+$",
    r"\.pb\.(go|ts|js)$",
    r"_pb2(_grpc)?\.pyi?$",
    r"\.min\.(js|css)$",
    r"\.snap$",
];
const CONFIG: &[&str] = &[
    r"(^|/)(BUILD|BUILD\.bazel|MODULE\.bazel|WORKSPACE|WORKSPACE\.bazel|\.bazelrc|\.bazelversion)$",
    r"\.(bzl|bazelrc)$",
    r"(^|/)(package\.json|go\.mod|Cargo\.toml|pyproject\.toml|\.npmrc|\.nvmrc|\.yarnrc|\.yarnrc\.yml|\.editorconfig|\.gitignore|\.dockerignore|\.babelrc)$",
    r"(^|/)[tj]sconfig[^/]*\.json$",
    r"(^|/)\.(eslintrc|prettierrc)[^/]*$",
    r"(^|/)(eslint|prettier|vitest|vite|jest|babel)\.config\.[a-z]+$",
];

pub fn kind_of(path: &str) -> &'static str {
    static SETS: OnceLock<[RegexSet; 3]> = OnceLock::new();
    let [tests, generated, config] = SETS.get_or_init(|| {
        [TESTS, GENERATED, CONFIG].map(|patterns| RegexSet::new(patterns).expect("kinds compile"))
    });
    if tests.is_match(path) {
        "tests"
    } else if generated.is_match(path) {
        "gen"
    } else if config.is_match(path) {
        "cfg"
    } else {
        ""
    }
}
