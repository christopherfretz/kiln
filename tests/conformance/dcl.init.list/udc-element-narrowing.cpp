// [dcl.init.list]/7, [over.ics.user]: narrowing is checked on the second
// standard conversion after a user-defined conversion, for an array element
// and a reference-to-array parameter as for a scalar (libc++ variant's
// `__check_for_narrowing`, `bool a[] = {X()}` with `operator void*`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:6 error "type 'void *' cannot be narrowed to 'bool' in initializer list"
//@ diag: 21:6 note "insert an explicit cast to silence this issue"
//@ diag: 22:7 error "non-constant-expression cannot be narrowed from type 'int' to 'char' in initializer list"
//@ diag: 22:7 note "insert an explicit cast to silence this issue"
//@ diag: 23:7 error "type 'void *' cannot be narrowed to 'bool' in initializer list"
//@ diag: 23:7 note "insert an explicit cast to silence this issue"
//@ diag: 24:16 error "type 'void *' cannot be narrowed to 'bool' in initializer list"
//@ diag: 24:16 note "insert an explicit cast to silence this issue"
struct X { operator void*(); };
struct Y { operator int(); };
void h(bool (&&)[1]);
void h2(char (&&)[1]);
void h3(bool);
void k() {
  h({X()}); bool ok[1] = {static_cast<bool>(X())};
  h2({Y()});
  h3({X()});
  bool a[1] = {X()};
  (void)ok; (void)a;
}
