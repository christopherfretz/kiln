// [except.handle]: try-blocks and handlers (catch by value, reference,
// pointer, class with copy and destructor, catch-all, rethrow,
// nesting, loops), [except.ctor]/1: locals destroyed on unwind.
void f();
int g(int);
struct S { int a; S(); S(const S&); ~S(); };
struct T { int a, b; };
struct E {};
struct B { virtual ~B(); };
struct D : B { int x; };

int c_int() { try { f(); } catch (int x) { return x; } return 0; }
int c_ref() { try { f(); } catch (int &x) { return x; } return 0; }
int c_ptr() { try { f(); } catch (int *p) { return *p; } return 0; }
int c_ptr_ref() { try { f(); } catch (const int *&p) { return *p; } return 0; }
int c_cls_ptr_ref() { try { f(); } catch (B *&p) { return 1; } return 0; }
int c_all() { try { f(); } catch (...) { return 1; } return 0; }
int c_multi() {
  try { f(); }
  catch (int) { return 1; }
  catch (long x) { return (int)x; }
  catch (...) { return 3; }
  return 0;
}
int c_two() {
  try { f(); }
  catch (int) { return 1; }
  catch (double) { return 2; }
  return 0;
}
int c_trivial() { try { f(); } catch (T t) { return t.a + t.b; } return 0; }
int c_class() { try { f(); } catch (S s) { return s.a; } return 0; }
int c_class_ref() { try { f(); } catch (const S &s) { return s.a; } return 0; }
int c_empty() { try { f(); } catch (E e) { return 1; } return 0; }
void c_rethrow() { try { f(); } catch (...) { g(1); throw; } }
void c_nested() {
  try {
    try { f(); } catch (int) { g(1); }
  } catch (...) { g(2); }
}
void c_dtor() {
  S s;
  try { S t; f(); } catch (int) { g(3); }
}
bool c_bool() { try { f(); } catch (bool b) { return b; } return false; }
void c_nullptr() { try { f(); } catch (decltype(nullptr) n) { } }
void c_notry() { try { g(1); } catch (...) {} }
void c_noexcept() noexcept;
void c_ne() { try { c_noexcept(); } catch (...) { g(1); } }
void c_loop() { for (int i = 0; i < 3; ++i) { try { f(); } catch (int) { continue; } catch (...) { break; } } }
