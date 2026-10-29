// [stmt.if]/4: both substatements of a consteval if statement are
// compound statements; a constexpr if needs a parenthesized condition
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:6 error "expected { after consteval"
//@ diag: 10:31 error "expected { after else"
//@ diag: 11:16 error "expected '(' after 'if'"
int f(int v) {
  if consteval return v;
  if !consteval { return 1; } else return 2;
  if constexpr v;
  return 0;
}
