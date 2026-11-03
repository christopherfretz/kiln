// [basic.def.odr]/5, [temp.inst]/4: a namespace-scope variable named in a
// template is odr-used by the instantiated definition, also where the
// expression naming it is not dependent; it is then needed, and
// -Wunneeded-internal-declaration stays silent. One named only in an
// uninstantiated template, or not evaluated, is still unneeded.
//@ args: -Wunneeded-internal-declaration -Wunused-variable
//@ strict-diags
//@ diag: 12:12 warning "variable 'c' is not needed and will not be emitted"
//@ diag: 13:12 warning "variable 'd' is not needed and will not be emitted"
static int a;
static int b;
static int c;
static int d;
static int e;
template <class T> void f() { a += 1; }
template <class T> struct Q { static int g() { return b; } };
int h() { return sizeof(c); }
template <class T> int k() { return d; }
template <class T> int m() { return e + T(); }
int main() {
  f<int>();
  return Q<int>::g() + h() + m<int>();
}
