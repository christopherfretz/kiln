//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:9 error "const_cast from rvalue to reference type 'int &&'"
//@ diag: 13:10 error "const_cast from rvalue to reference type 'int &'"
//@ diag: 14:10 error "const_cast from rvalue to reference type 'int &'"
//@ diag: 15:10 error "reinterpret_cast from rvalue to reference type 'int &'"
//@ diag: 16:9 error "reinterpret_cast from rvalue to reference type 'S &&'"
// [expr.const.cast]/3, [expr.reinterpret.cast]/11: only a class prvalue is
// materialized; a non-class rvalue does not bind (and the erroneous cast
// is not diagnosed again as an initializer).
struct S {}; S mk(); int x;
int i = const_cast<int &&>(1);
int &j = const_cast<int &>(1);
int &k = const_cast<int &>(static_cast<int &&>(x));
int &l = reinterpret_cast<int &>(1);
S &&m = reinterpret_cast<S &&>(mk());
S &n = reinterpret_cast<S &>(static_cast<S &&>(*new S));
