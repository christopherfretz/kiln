// [dcl.pre]/12 (C++26, P2741R3), [lex.charset]: a failed static_assert
// prints its message's characters as clang escapes a string argument
// (`pushEscapedString`, spec/diag.md §6.2): ASCII whitespace (here a
// vertical tab and a form feed) and format characters (U+00AD, U+200B)
// as they are; a line or paragraph separator, a C1 control, a
// private-use or noncharacter code point as <U+XXXX>; a byte that is
// not part of valid UTF-8 (an encoded surrogate too) as <XX> (WP-1259:
// kiln escaped the vertical tab, form feed and carriage return and
// printed U+2028 raw). clang 19 and 22 agree; GCC 16.1 prints every
// byte raw. clang's escape of an unassigned code point (U+0378 prints
// as <U+0378>) is not matched (spec/diag.md deviation D5).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:15 error "static assertion failed: ws|"
//@ diag: 27:15 error "static assertion failed: fmt­​|"
//@ diag: 28:15 error "static assertion failed: sep<U+2028><U+2029><U+0085>|"
//@ diag: 29:15 error "static assertion failed: pua<U+E000><U+FFFF>|"
//@ diag: 30:15 error "static assertion failed: bad<FF><C3>(<ED><BF><BF>|"
struct Msg {
  const char *p;
  unsigned long n;
  constexpr const char *data() const { return p; }
  constexpr unsigned long size() const { return n; }
};
template <unsigned long N> constexpr Msg msg(const char (&s)[N]) { return {s, N - 1}; }
static_assert(false, msg("ws\v\f|"));
static_assert(false, msg("fmt\u00ad\u200b|"));
static_assert(false, msg("sep\u2028\u2029\u0085|"));
static_assert(false, msg("pua\ue000\uffff|"));
static_assert(false, msg("bad\xff\xc3(\xed\xbf\xbf|"));
