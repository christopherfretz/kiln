// [dcl.attr.deprecated]/4, [temp.expl.spec], [temp.spec.partial]: a use of
// a deprecated variable template specialization is warned: an explicit
// specialization carries its own attribute (before the declarator-id or
// after it), an implicit one instantiated from a deprecated partial
// specialization or primary template inherits it; the primary's attribute
// does not reach an explicit specialization declared without one.
//@ strict-diags
//@ diag: 24:9 warning "'v<int>' is deprecated"
//@ diag: 19:15 note "'v<int>' has been explicitly marked deprecated here"
//@ diag: 25:9 warning "'v<char>' is deprecated"
//@ diag: 20:27 note "'v<char>' has been explicitly marked deprecated here"
//@ diag: 26:9 warning "'v<int *>' is deprecated"
//@ diag: 21:22 note "'v<int *>' has been explicitly marked deprecated here"
//@ diag: 27:9 warning "'pv<int>' is deprecated: msg"
//@ diag: 22:22 note "'pv<int>' has been explicitly marked deprecated here"
//@ diag: 30:20 warning "'v<int>' is deprecated"
//@ diag: 19:15 note "'v<int>' has been explicitly marked deprecated here"
template <class T> int v = 0;
template <> [[deprecated]] int v<int> = 1;
template <> int v<char> [[deprecated]] = 1;
template <class T> [[deprecated]] int v<T *> = 2;
template <class T> [[deprecated("msg")]] int pv = 2;
template <> int pv<long> = 3;
int a = v<int>;
int b = v<char>;
int c = v<int *>;
int d = pv<int>;
int e = pv<long>;
int g = v<short>;
void f() { int h = v<int>; }
