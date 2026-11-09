//@ args: -freflection
//@ generate: nested-dup 200 6 4
// Repeated nested template-ids (spec/parse.md §2.5; WP-879, WP-919),
// generated at bench time (spec/bench.md "Generated entries"): 200
// reflection-keyed specializations of a function template and of a class
// template's static member, each keyed by a template-id 6 levels deep
// whose every level repeats the previous one twice, so the 1.3 MB source
// is mostly identical token runs. Measures template-id parsing, name
// classification and lookahead scanning at scale; the 5.1 MB depth-8
// variant of docs/status/perf.md "WP-919" has the same slope. The
// generator is xtask/src/bench/generate.rs. kiln only: clang 19/22 lack
// `^^`.
