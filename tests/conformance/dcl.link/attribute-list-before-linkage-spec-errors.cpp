// [dcl.link]/2, [dcl.attr.grammar]/5: a linkage-specification has no
// attribute-specifier-seq, so a `[[...]]` or `alignas` list before
// `extern "C"` is ill-formed, alone or mixed with GNU attributes (clang
// `ProhibitAttributes`: "an attribute list cannot appear here"); the
// declaration it contains is still parsed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:1 error "an attribute list cannot appear here"
//@ diag: 13:1 error "an attribute list cannot appear here"
//@ diag: 14:1 error "an attribute list cannot appear here"
//@ diag: 15:25 error "an attribute list cannot appear here"
[[gnu::section("s")]] extern "C" int f(int);
[[deprecated]] extern "C" { int h(int x); }
alignas(4) extern "C" int g;
__attribute__((unused)) [[deprecated]] extern "C" int q2();
int use() { return f(1) + h(2) + g + q2(); }
