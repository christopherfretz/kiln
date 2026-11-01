// [stmt.for] (WP-234): -Wfor-loop-analysis, clang's two checks run as
// each 'for' statement is built (the template definition and then every
// instantiation). (1) A condition built only from literals, built-in
// operators and local non-volatile variables warns when none of those
// variables is written, has its address taken or is bound to a reference
// in the condition, the increment or the body, and nothing in them is a
// 'return', 'break' or 'goto' (unevaluated operands and lambda bodies are
// not searched; more than four variables are not named). (2) A braced
// body whose last statement increments (decrements) the same name as the
// increment expression, built-in or overloaded, warns unless the body
// has a 'continue' of this loop.
//@ args: -Wfor-loop-analysis
//@ strict-diags
//@ diag: 112:34 warning "variables 'i' and 'n' used in loop condition not modified in loop body"
//@ diag: 113:38 warning "variables 'i' and 'n' used in loop condition not modified in loop body"
//@ diag: 125:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 126:51 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 126:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 128:46 warning "variables used in loop condition not modified in loop body"
//@ diag: 129:42 warning "variables 'a', 'b', 'c', and 'd' used in loop condition not modified in loop body"
//@ diag: 130:38 warning "variables 'a', 'b', and 'c' used in loop condition not modified in loop body"
//@ diag: 131:34 warning "variables 'a' and 'b' used in loop condition not modified in loop body"
//@ diag: 132:30 warning "variable 'a' used in loop condition not modified in loop body"
//@ diag: 133:30 warning "variable 'a' used in loop condition not modified in loop body"
//@ diag: 134:31 warning "variable 'a' used in loop condition not modified in loop body"
//@ diag: 136:51 warning "variable 'i' is incremented both in the loop header and in the loop body"
//@ diag: 136:44 note "incremented here"
//@ diag: 137:49 warning "variable 'i' is incremented both in the loop header and in the loop body"
//@ diag: 137:42 note "incremented here"
//@ diag: 138:49 warning "variable 'i' is decremented both in the loop header and in the loop body"
//@ diag: 138:44 note "decremented here"
//@ diag: 141:74 warning "variable 'i' is incremented both in the loop header and in the loop body"
//@ diag: 141:44 note "incremented here"
//@ diag: 143:39 warning "variable 's' is incremented both in the loop header and in the loop body"
//@ diag: 143:32 note "incremented here"
//@ diag: 144:37 warning "variable 's' is decremented both in the loop header and in the loop body"
//@ diag: 144:30 note "decremented here"
//@ diag: 149:38 warning "variable 'b' used in loop condition not modified in loop body"
//@ diag: 150:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 151:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 156:32 warning "left operand of comma operator has no effect"
//@ diag: 158:56 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 158:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 160:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 161:56 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 162:51 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 162:51 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 163:15 note "in instantiation of function template specialization 't52<int>' requested here"
//@ diag: 162:51 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 163:41 note "in instantiation of function template specialization 't52<char>' requested here"
//@ diag: 164:49 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 165:15 note "in instantiation of function template specialization 't53<int>' requested here"
//@ diag: 164:49 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 165:41 note "in instantiation of function template specialization 't53<char>' requested here"
//@ diag: 166:70 warning "variable 'i' is incremented both in the loop header and in the loop body"
//@ diag: 166:63 note "incremented here"
//@ diag: 166:70 warning "variable 'i' is incremented both in the loop header and in the loop body"
//@ diag: 167:15 note "in instantiation of function template specialization 't54<int>' requested here"
//@ diag: 166:63 note "incremented here"
//@ diag: 166:70 warning "variable 'i' is incremented both in the loop header and in the loop body"
//@ diag: 167:44 note "in instantiation of function template specialization 't54<char>' requested here"
//@ diag: 166:63 note "incremented here"
//@ diag: 169:43 warning "expression result unused"
//@ diag: 170:38 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 171:33 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 172:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 173:38 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 175:33 warning "variable 'r' used in loop condition not modified in loop body"
//@ diag: 176:44 warning "variable 'r' used in loop condition not modified in loop body"
//@ diag: 184:41 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 185:41 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 186:31 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 188:31 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 188:45 warning "expression result unused"
//@ diag: 191:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 192:48 warning "variable 'a' is incremented both in the loop header and in the loop body"
//@ diag: 192:41 note "incremented here"
//@ diag: 193:55 warning "variable 'a' is incremented both in the loop header and in the loop body"
//@ diag: 193:48 note "incremented here"
//@ diag: 194:53 warning "variable 'a' is incremented both in the loop header and in the loop body"
//@ diag: 194:46 note "incremented here"
//@ diag: 199:56 warning "variable 'i' is incremented both in the loop header and in the loop body"
//@ diag: 199:44 note "incremented here"
//@ diag: 202:75 warning "variable 'i' is incremented both in the loop header and in the loop body"
//@ diag: 202:44 note "incremented here"
//@ diag: 203:79 warning "variable 'i' is incremented both in the loop header and in the loop body"
//@ diag: 203:44 note "incremented here"
//@ diag: 204:100 warning "variable 'i' is incremented both in the loop header and in the loop body"
//@ diag: 204:54 note "incremented here"
//@ diag: 207:80 warning "variable 'i' is incremented both in the loop header and in the loop body"
//@ diag: 207:44 note "incremented here"
//@ diag: 208:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 209:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 211:52 warning "left operand of comma operator has no effect"
//@ diag: 212:39 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 215:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 216:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 217:34 warning "variable 'f' used in loop condition not modified in loop body"
//@ diag: 218:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 220:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 221:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 222:50 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 223:34 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 224:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 225:60 warning "expression with side effects has no effect in an unevaluated context"
//@ diag: 225:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 226:32 warning "variable 'i' used in loop condition not modified in loop body"
//@ diag: 227:32 warning "variable 'i' used in loop condition not modified in loop body"
struct S { int m; int operator++(); int operator--(int); };
int g;
int f(int);
void t1(int n) { for (int i = 0; i < n; ) {} }
void t2() { int i = 0, n = 3; for (; i < n; ) {} }
void t3() { int i = 0, n = 3; for (; i < n; ) { ++i; } }
void t4() { int i = 0, n = 3; for (; i < n; ) { break; } }
void t5() { int i = 0, n = 3; for (; i < n; ) { if (n) return; } }
void t6() { volatile int i = 0; int n = 3; for (; i < n; ) {} }
void t7() { static int i = 0; int n = 3; for (; i < n; ) {} }
void t8() { int i = 0; for (; i < g; ) {} }
void t9(S s) { for (; s.m; ) {} }
void t10() { int i = 0; for (; f(i); ) {} }
void t11() { int *p = nullptr; for (; p != nullptr; ) {} }
void t12() { int *p = 0; for (; *p; ) {} }
void t13() { int i = 0; for (; i; ) { switch (i) { case 0: break; } } }
void t14() { int i = 0; for (; i; ) { [] { return; }(); } }
void t15() { int i = 0; for (; i; ) { (void)sizeof(i = 1); } }
void t16() { int i = 0; for (; i; ) { int *p = &i; (void)p; } }
void t17() { int a=0,b=0,c=0,d=0,e=0; for (; a+b+c+d+e; ) {} }
void t18() { int a=0,b=0,c=0,d=0; for (; a+b+c+d; ) {} }
void t19() { int a=0,b=0,c=0; for (; a+b+c; ) {} }
void t20() { int a=0,b=0; for (; a<b; ) {} }
void t21() { int a=0; for (; a ? a : 1; ) {} }
void t22() { int a=0; for (; a ?: 1; ) {} }
void t23() { int a=0; for (; (a); ) {} }
void t24() { int a=0; for (; a; ) { a++; } }
void t25(int n) { for (int i = 0; i < n; ++i) { ++i; } }
void t26(int n) { for (int i = 0; i < n; i++) { i++; } }
void t27(int n) { for (int i = 0; i < n; --i) { i--; } }
void t28(int n) { for (int i = 0; i < n; ++i) { --i; } }
void t29(int n) { for (int i = 0; i < n; ++i) { if (n) continue; ++i; } }
void t30(int n) { for (int i = 0; i < n; ++i) { for (;;) { continue; } ++i; } }
void t31(int n) { for (int i = 0; i < n; ++i) { switch (n) { case 1: continue; } ++i; } }
void t32(S s) { for (; true; ++s) { ++s; } }
void t33(S s) { for (; true; s--) { s--; } }
void t34(int n) { for (int i = 0; i < n; ++(i)) { ++i; } }
void t36(int n) { for (int i = 0; i < n; ++i) ++i; }
void t37(int n) { for (int i = 0; i < n; ++i) { ++i; ; } }
void t38() { int i = 0; for (; i < 4; ) { goto x; x:; } }
void t39() { bool b = false; for (; !b; ) {} }
void t40() { int i = 0; for (; i < 4.0; ) {} }
void t41() { int i = 0; for (; i < 'a'; ) {} }
void t42() { int i = 0; for (; true; ) { (void)i; } }
void t43() { for (int i = 0; int j = i; ) {} }
void t44() { int i = 0, n = 3; for (; i < n; ) { n = n; } }
void t45() { int i = 0, n = 3; for (; i < n; i = i) {} }
void t46() { int i = 0; for (; i, i; ) {} }
void t47() { int i = 0; for (; i += 1; ) {} }
void t48() { int i = 0; for (; i; ) { (void)noexcept(i = 1); } }
void t49() { int i = 0; for (; i; ) { [&] { i = 1; }(); } }
void t50() { int i = 0; for (; i; ) { [i] () mutable { i = 1; }(); } }
void t51() { int i = 0; for (; i; ) { (void)decltype(i = 1)(i); } }
template <class T> void t52() { int i = 0; for (; i < 3; ) {} }
template void t52<int>(); template void t52<char>();
template <class T> void t53() { T i = 0; for (; i < 3; ) {} }
template void t53<int>(); template void t53<char>();
template <class T> void t54(int n) { for (int i = 0; i < n; ++i) { ++i; } }
template void t54<int>(int); template void t54<char>(int);
void t55() { int arr[3]; int i = 0; for (; arr[i]; ) {} }
void t56() { int i = 0; for (; i < 3; ) { i; } }
void t57() { int i = 0; for (; (bool)i; ) {} }
void t58() { int i = 0; for (; -i; ) {} }
void t59() { int i = 0; for (; i == 0 && i < 3; ) {} }
void t60() { const int i = 0; for (; i; ) {} }
void t61() { thread_local int i = 0; for (; i; ) {} }
void t62() { int &r = g; for (; r; ) {} }
void t63() { int x = 0; int &r = x; for (; r; ) {} }
enum E { N = 3 };
struct P { int a, b; };
struct It { It &operator++(); It operator++(int); bool operator!=(const It &) const; };
void u1() { int i = 0; for (; i < N; ) {} }
template <int K> void u2() { int i = 0; for (; i < K; ) {} }
template void u2<1>();
void u3() { auto [a, b] = P{1, 2}; for (; a < 3; ) {} }
void u4() { int i = 0; int v[3]; for (; i < 3; ) { for (int x : v) (void)x; } }
void u5() { int i = 0; int v[3]; for (; i < 3; ) { for (int &x : v) (void)x; } }
void u6() { int i = 0; for (; i < 3; ) { if (int j = i) (void)j; } }
void u7() { int i = 0; for (; i < 3; ) { do { break; } while (0); } }
void u8() { int i = 0; for (; i < 3; ) { ({ i; }); } }
void u9() { int i = 0; for (; i < 3; ) { int &r = i; (void)r; } }
void u10() { int i = 0; for (; i < 3; ) { const int &r = i; (void)r; } }
void u11() { int i = 0; for (; i < 3; ) { int j = i; (void)j; } }
void u12(It a, It b) { for (; a != b; ++a) { ++a; } }
template <class T> void u13(T a) { for (; a; ++a) { ++a; } }
template <class T> void u14(T a) { for (; a; a++) { a++; } }
void u15() { int i = 0; for (; i < 3; ) { (++i); } }
void u16(int n) { for (int i = 0; i < n; ++i) { (++i); } }
void u17(int n) { for (int i = 0; i < n; (++i)) { ++i; } }
void u18(int n) { for (int i = 0, j = 0; i < n; ++i) { ++j; } }
void u19(int n) { for (int i = 0; i < n; ++i) { ++n; ++i; } }
void u20(int n) { for (int i = 0; i < n; ++i) { { ++i; } } }
void u21(int n) { for (int i = 0; i < n; ++i) { if (n) { } else { continue; } ++i; } }
void u22(int n) { for (int i = 0; i < n; ++i) { while (n) { continue; } ++i; } }
void u23(int n) { for (int i = 0; i < n; ++i) { do { continue; } while (0); ++i; } }
void u24(int n) { int v[2]; for (int i = 0; i < n; ++i) { for (int x : v) { (void)x; continue; } ++i; } }
void u25(int n) { for (int i = 0; i < n; ++i) { try { continue; } catch (...) {} ++i; } }
void u26(int n) { for (int i = 0; i < n; ++i) { l: ++i; } }
void u27(int n) { for (int i = 0; i < n; ++i) { [] { for (;;) continue; }(); ++i; } }
void u28() { int i = 0; for (; i < 3; ) { int *p = 0; (void)p; } }
void u29() { int i = 0; for (; i < 3 && i > 0; ) { int x = i ? i : 0; (void)x; } }
void u30() { int i = 0; for (; i < 3; ) { int &x = true ? i : i; (void)x; } }
void u31() { int i = 0; for (; i < 3; ) { int x = (i, 1); (void)x; } }
void u32() { int i = 0, k = 0; for (; i < 3; ) { int x = k ?: i; (void)x; } }
void u33() { int i = 0, k = 0; for (; i < 3; ) { int &x = k ?: i; (void)x; } }
void u34() { int i = 0; for (; i < 3; ) { i += 0; } }
void u35() { int i = 0; for (; i < 3; ) { throw 1; } }
void u36() { int i = 0; for (; i < 3; ) { switch (0) { default: ; } } }
void u37() { float f = 0; for (; f < 3; ) {} }
void u38() { int i = 0; for (; i < 3; ) { struct L { void m() { return; } }; } }
void u39(int i) { for (; i < 3; ) { asm("" : "=r"(i)); } }
void u40() { int i = 0; for (; i < 3 + sizeof(int) ; ) {} }
void u41() { int i = 0; for (; i < (int)3.0 ; ) {} }
void u42() { int i = 0; for (; static_cast<bool>(i) ; ) {} }
void u43() { int i = 0; for (; !!i ; ) {} }
void u44() { int i = 0; for (; i < 3 ; ) { static int s = i; (void)s; } }
void u45() { int i = 0; for (; i < 3 ; ) { (void)noexcept(i++); } }
void u46() { int i = 0; for (; i ; ) { if consteval { } } }
void u47() { int i = 0; for (; i ; ) { [=] { return i; }(); } }
void u48() { int i = 0; for (; i ; ) { [&i] { return i; }(); } }
