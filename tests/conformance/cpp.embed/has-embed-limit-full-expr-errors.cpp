// [cpp.cond]/5, /12, [cpp.embed.param.limit]: errors in the `limit`
// parameter of a `__has_embed` inside `#if` are the `#if` evaluator's, as
// clang reports them: a `?` without `:`, a floating literal, overflow and
// division by zero.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-c23-extensions
//@ diag: 16:37 error "expected ':'"
//@ diag: 16:34 note "to match this '?'"
//@ diag: 16:39 error "expected value in expression"
//@ diag: 18:32 error "floating point literal in preprocessor expression"
//@ diag: 18:37 error "expected value in expression"
//@ diag: 20:52 warning "integer overflow in preprocessor expression"
//@ diag: 22:34 error "division by zero in preprocessor expression"
//@ diag: 22:39 error "expected value in expression"
#if __has_embed(__FILE__ limit(1 ? 2))
#endif
#if __has_embed(__FILE__ limit(1.0))
#endif
#if __has_embed(__FILE__ limit((0x7fffffffffffffff * 2) & 1))
#endif
#if __has_embed(__FILE__ limit(1 / 0))
#endif
