// [temp.pre]/1, [basic.def.odr] (WP-234): -Wunused-template, a variable
// template with internal linkage none of whose specializations is
// odr-used (or referenced and usable in constant expressions) or
// explicitly specialized.
//@ args: -Wunused-template
//@ strict-diags
//@ diag: 12:29 warning "unused variable template 'vt'"
//@ diag: 16:34 warning "unused variable template 'vt4'"
//@ diag: 17:31 warning "unused variable template 'vt5'"
//@ diag: 19:31 warning "unused variable template 'vt6'"
//@ diag: 25:31 warning "unused variable template 'vt9'"
template <class T> static T vt = T();
template <class T> static T vt2 = T();
int uvt2() { return vt2<int>; }
template <class T> T vt3 = T();
namespace { template <class T> T vt4 = T(); }
template <class T> static int vt5 = 1;
int u5() { return sizeof(vt5<int>); }
template <class T> static int vt6 = 1;
template <class T> static int vt6<T*> = 2;
template <class T> static const int vt7 = 1;
int u7() { return vt7<int>; }
template <class T> static int vt8 = 1;
template <> int vt8<char> = 2;
template <class T> static int vt9 = 1;
template <class T> int uvt9() { return vt9<T>; }
