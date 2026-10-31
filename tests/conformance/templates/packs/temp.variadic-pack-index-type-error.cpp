//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:40 error "invalid index 0 for pack 'Ts' of size 0"
//@ diag: 9:1 note "in instantiation of template type alias 'g' requested here"
// [dcl.type.pack.index]/2: the index must be less than the pack's size;
// clang reports it at the index expression, naming the pattern in quotes.
// (The pack is empty here.)
template <class... Ts> using g = Ts...[0];
g<> *p;
