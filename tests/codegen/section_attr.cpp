// GNU section attribute (clang SectionAttr; spec/codegen.md §8 "Sections"):
// on a variable or function declaration, definition, static local,
// static data member and function template specialization.
extern int ex __attribute__((section("sx")));
int use_ex() { return ex; }
int f2() { static int sl __attribute__((section("sl"))) = 1; return sl++; }
inline int f3() { static int il __attribute__((section("il"))) = 1; return il++; }
struct S { static int m __attribute__((section("sm"))); void mf() __attribute__((section("smf"))); };
int S::m = 2;
void S::mf() {}
template <class T> __attribute__((section("tf"))) T tf(T t) { return t; }
int g = tf(3) + f3();
