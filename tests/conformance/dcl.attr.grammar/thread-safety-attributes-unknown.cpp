// [dcl.attr.grammar]/6, [dcl.attr.unknown]: recognising clang's
// thread-safety attributes (WP-606; spec/sema.md §4.5) leaves every other
// attribute unknown -- a genuinely unknown GNU or standard attribute, a
// thread-safety name without its `clang::` scope or under `gnu::`, and the
// GNU-only names (`guarded_by`, `lockable`, `lock_returned`, ...: clang
// `GNU<"x">`, no `[[clang::x]]` spelling) written `[[clang::x]]` -- each still
// "unknown attribute ignored". clang 19 prints the bare name and 22 the scoped
// one; the substrings below are common to both.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 21:22 warning "unknown attribute 'frobnicate_lock' ignored"
//@ diag: 22:22 warning "unknown attribute 'quux_unrelated' ignored"
//@ diag: 23:3 warning "unknown attribute 'guarded_by' ignored"
//@ diag: 24:3 warning "guarded_by' ignored"
//@ diag: 25:3 warning "guarded_by' ignored"
//@ diag: 26:10 warning "lockable' ignored"
//@ diag: 27:3 warning "lock_returned' ignored"
//@ diag: 28:3 warning "capability' ignored"
struct Mutex {};
Mutex mu;
int a __attribute__((frobnicate_lock(mu)));
int b __attribute__((quux_unrelated(mu)));
[[guarded_by(mu)]] int c;
[[gnu::guarded_by(mu)]] int d;
[[clang::guarded_by(mu)]] int e;
struct [[clang::lockable]] Old {};
[[clang::lock_returned(mu)]] Mutex &f();
[[gnu::capability("m")]] int g;
