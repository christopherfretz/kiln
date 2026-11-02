// [dcl.type.general]/1, [dcl.stc]/1: a type-specifier-seq takes no
// storage class. In a type-id (cast, sizeof, alignof, typeid, template
// argument, alias-declaration), a new-type-id (also parenthesized), a
// conversion-type-id, a functional cast's type and an
// exception-declaration clang says "type name does not allow storage
// class to be specified" at the storage class (else at `thread_local`)
// and drops it (WP-327). `mutable int()` is no expression.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:21 error "type name does not allow storage class to be specified"
//@ diag: 29:16 error "type name does not allow storage class to be specified"
//@ diag: 30:17 error "type name does not allow storage class to be specified"
//@ diag: 31:17 error "type name does not allow storage class to be specified"
//@ diag: 32:5 error "type name does not allow storage class to be specified"
//@ diag: 33:19 error "type name does not allow storage class to be specified"
//@ diag: 34:20 error "type name does not allow storage class to be specified"
//@ diag: 35:13 error "type name does not allow storage class to be specified"
//@ diag: 36:24 error "type name does not allow storage class to be specified"
//@ diag: 37:13 error "type name does not allow storage class to be specified"
//@ diag: 38:13 error "type name does not allow storage class to be specified"
//@ diag: 39:12 error "expected expression"
//@ diag: 40:16 error "expected expression"
//@ diag: 41:17 error "type name does not allow storage class to be specified"
//@ diag: 42:17 error "type name does not allow storage class to be specified"
//@ diag: 43:18 error "type name does not allow storage class to be specified"
template <class T> struct X {};
struct C { operator static int(); };
void f() {
  int *p = new static int;
  int *q = new (static int);
  int *r = new (static int[2]);
  X<static int> x;
  auto s = sizeof(static int);
  auto a = alignof(extern int);
  auto c = (static int)1;
  auto d = static_cast<static int>(1);
  using U = static int;
  auto g = (register int)2;
  auto h = mutable int();
  auto i = int(static int);
  try {} catch (static int) {}
  try {} catch (thread_local int) {}
  int t = sizeof(static int(*)());
}
