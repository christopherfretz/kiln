// [basic.start.main]/3, [temp.friend]/1, [temp.inst]/4 (WP-2540): `main`
// is not overloaded, so a declaration of it with another signature
// redeclares the earlier one; a friend `main` in a class template that is
// never instantiated declares nothing, so `int main(int, char **)` after it
// is the first declaration of `main` (it was "conflicting types").
//@ expect: compile-ok
//@ strict-diags
template <class T> struct A { friend int main(); };
int main(int, char **);
