//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:5 error "ISO C++17 does not allow incrementing expression of type bool"
//@ diag: 13:3 error "ISO C++17 does not allow incrementing expression of type bool"
//@ diag: 14:5 error "cannot decrement expression of type bool"
//@ diag: 15:3 error "cannot decrement expression of type bool"
// WP-165: ++ on bool / _Atomic(bool) is a DefaultError extension in
// C++17 and later (-Wincrement-bool); -- is always an error.
_Atomic(bool) ab;
bool b;
void f() {
  ab++;
  ++b;
  ab--;
  --b;
}
