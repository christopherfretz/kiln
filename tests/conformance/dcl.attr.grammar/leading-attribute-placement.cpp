// [dcl.pre]/5, [dcl.attr.grammar]/5, [class.pre]/1, [class.mem.general]/1,
// [stmt.pre]/1, [namespace.udecl]/1, [namespace.udir]/1, [dcl.typedef]/2,
// [dcl.enum]/1, [temp.pre]/1: a leading attribute-specifier-seq is allowed
// only where the grammar has one. Like clang 19, "an attribute list cannot
// appear here" (attributes dropped) before a namespace, static_assert,
// template, linkage-specification, asm-declaration, alias-declaration,
// namespace-alias and friend non-definition, at namespace scope, inside a
// namespace, at block scope and in a member-specification; after the
// class-name of a class-head (moved onto the class) or after `final`
// (dropped); "misplaced attributes" on a free-standing tag declaration at
// namespace scope; the place-it warning for the attributes of a declaration
// that declares only a tag; declaration attributes on a non-declaration
// statement are errors; an enumerator cannot start with an attribute.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 97:1 error "an attribute list cannot appear here"
//@ diag: 98:1 error "an attribute list cannot appear here"
//@ diag: 99:1 error "an attribute list cannot appear here"
//@ diag: 100:1 error "an attribute list cannot appear here"
//@ diag: 101:3 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 102:1 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 103:3 warning "ISO C++ does not allow an attribute list to appear here [-Wcxx-attribute-extension]"
//@ diag: 103:3 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 104:1 warning "ISO C++ does not allow an attribute list to appear here [-Wcxx-attribute-extension]"
//@ diag: 104:1 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 105:1 error "an attribute list cannot appear here"
//@ diag: 106:1 error "an attribute list cannot appear here"
//@ diag: 107:1 error "an attribute list cannot appear here"
//@ diag: 108:1 error "an attribute list cannot appear here"
//@ diag: 109:3 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 110:1 error "an attribute list cannot appear here"
//@ diag: 111:1 error "an attribute list cannot appear here"
//@ diag: 112:21 error "misplaced attributes; expected attributes here"
//@ diag: 113:19 error "misplaced attributes; expected attributes here"
//@ diag: 114:21 error "misplaced attributes; expected attributes here"
//@ diag: 115:1 error "an attribute list cannot appear here"
//@ diag: 117:1 error "an attribute list cannot appear here"
//@ diag: 118:1 error "an attribute list cannot appear here"
//@ diag: 119:1 error "an attribute list cannot appear here"
//@ diag: 120:1 error "an attribute list cannot appear here"
//@ diag: 121:3 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 122:3 warning "ISO C++ does not allow an attribute list to appear here [-Wcxx-attribute-extension]"
//@ diag: 122:3 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 123:1 warning "ISO C++ does not allow an attribute list to appear here [-Wcxx-attribute-extension]"
//@ diag: 123:1 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 124:1 error "an attribute list cannot appear here"
//@ diag: 125:1 error "an attribute list cannot appear here"
//@ diag: 126:1 error "an attribute list cannot appear here"
//@ diag: 127:21 error "misplaced attributes; expected attributes here"
//@ diag: 128:1 error "an attribute list cannot appear here"
//@ diag: 131:3 error "an attribute list cannot appear here"
//@ diag: 132:3 error "an attribute list cannot appear here"
//@ diag: 133:3 error "an attribute list cannot appear here"
//@ diag: 134:5 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 135:5 warning "ISO C++ does not allow an attribute list to appear here [-Wcxx-attribute-extension]"
//@ diag: 135:5 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 136:3 warning "ISO C++ does not allow an attribute list to appear here [-Wcxx-attribute-extension]"
//@ diag: 136:3 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 137:3 error "an attribute list cannot appear here"
//@ diag: 138:3 error "an attribute list cannot appear here"
//@ diag: 139:5 warning "'nodiscard' attribute ignored [-Wignored-attributes]"
//@ diag: 140:5 error "'nodiscard' attribute cannot be applied to a statement"
//@ diag: 141:3 error "'alignas' attribute cannot be applied to a statement"
//@ diag: 142:5 warning "'[[nodiscard]]' attribute ignored when applied to a typedef; consider using '__attribute__((warn_unused_result))' or '[[clang::warn_unused_result]]' instead [-Wignored-attributes]"
//@ diag: 145:17 error "expected member name or ';' after declaration specifiers"
//@ diag: 146:3 error "an attribute list cannot appear here"
//@ diag: 147:3 error "an attribute list cannot appear here"
//@ diag: 148:5 warning "ISO C++ does not allow an attribute list to appear here [-Wcxx-attribute-extension]"
//@ diag: 148:5 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 149:3 warning "ISO C++ does not allow an attribute list to appear here [-Wcxx-attribute-extension]"
//@ diag: 149:3 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 150:17 error "expected member name or ';' after declaration specifiers"
//@ diag: 151:3 error "an attribute list cannot appear here"
//@ diag: 152:17 error "expected member name or ';' after declaration specifiers"
//@ diag: 154:5 warning "attribute 'nodiscard' is ignored, place it after \"struct\" to apply attribute to type declaration [-Wignored-attributes]"
//@ diag: 155:3 warning "attribute 'alignas' is ignored, place it after \"struct\" to apply attribute to type declaration [-Wignored-attributes]"
//@ diag: 156:17 error "expected member name or ';' after declaration specifiers"
//@ diag: 158:30 error "expected identifier"
//@ diag: 158:49 error "expected identifier"
//@ diag: 161:11 error "an attribute list cannot appear here"
//@ diag: 162:30 error "'nodiscard' attribute cannot be applied to types"
//@ diag: 162:30 warning "attribute 'nodiscard' is ignored, place it after \"struct\" to apply attribute to type declaration [-Wignored-attributes]"
//@ diag: 163:11 error "an attribute list cannot appear here"
//@ diag: 164:17 error "an attribute list cannot appear here"
//@ diag: 166:11 error "an attribute list cannot appear here"
//@ diag: 168:20 warning "ignoring return value of function declared with 'nodiscard' attribute [-Wunused-result]"
//@ diag: 169:11 error "an attribute list cannot appear here"
//@ diag: 171:28 error "an attribute list cannot appear here"
//@ diag: 172:35 error "an attribute list cannot appear here"
//@ diag: 173:13 error "'nodiscard' attribute cannot be applied to types"
//@ diag: 174:11 error "an attribute list cannot appear here"
//@ diag: 175:10 error "an attribute list cannot appear here"
//@ requires: clang<22
// clang 22 emits error: 'nodiscard' attribute cannot be applied to a statement; WP-355 residual
namespace N { void f(); struct B { void g(); void h(); }; template <class T> struct ST {}; }
[[nodiscard]] namespace N1 {}
alignas(8) using X0 = int;
[[nodiscard]] using X1 = int;
[[nodiscard]] static_assert(true);
[[nodiscard]] using namespace N;
alignas(4) using namespace N;
[[nodiscard]] using N::f;
alignas(4) using N::f;
[[nodiscard]] template <class T> void tf();
[[nodiscard]] extern "C" { }
[[nodiscard]] asm("");
[[nodiscard]] namespace NA = N;
[[nodiscard]];
[[nodiscard]] template <> struct N::ST<int> {};
[[nodiscard]] template struct N::ST<char>;
[[nodiscard]] struct S1 {};
[[nodiscard]] enum E1 {};
[[nodiscard]] struct S2;
[[nodiscard]] inline namespace IN {}
namespace O {
[[nodiscard]] namespace N1 {}
alignas(8) using X0 = int;
[[nodiscard]] using X1 = int;
[[nodiscard]] static_assert(true);
[[nodiscard]] using namespace N;
[[nodiscard]] using N::f;
alignas(4) using N::f;
[[nodiscard]] template <class T> void tf();
[[nodiscard]] asm("");
[[nodiscard]] namespace NA = N;
[[nodiscard]] struct S1 {};
[[nodiscard]] extern "C" { }
}
void blk() {
  [[nodiscard]] static_assert(true);
  [[nodiscard]] using X1 = int;
  alignas(8) using X0 = int;
  [[nodiscard]] using namespace N;
  [[nodiscard]] using N::f;
  alignas(4) using N::f;
  [[nodiscard]] namespace NA = N;
  [[nodiscard]] struct S1 {};
  [[nodiscard]] asm("");
  [[nodiscard]];
  alignas(4);
  [[nodiscard]] typedef int T;
}
struct D : N::B {
  [[nodiscard]] static_assert(true);
  [[nodiscard]] using X1 = int;
  alignas(8) using X0 = int;
  [[nodiscard]] using N::B::g;
  alignas(4) using N::B::h;
  [[nodiscard]] template <class T> void tf();
  [[nodiscard]] friend void fr();
  [[nodiscard]] public:
  [[nodiscard]];
  [[nodiscard]] struct S1 {};
  alignas(4) struct S3 {};
  [[nodiscard]] template <class T> using TA = T;
};
enum E2 { e1 [[deprecated]], [[deprecated]] e2, alignas(4) e3 };
template <class T> [[nodiscard]] T tf2();
template <class T> alignas(4) T tv;
struct S4 [[nodiscard]] {};
struct [[nodiscard]] S5 {} [[nodiscard]];
struct A1 [[nodiscard]];
struct A2 final [[nodiscard]] {};
A2 fa2();
struct A3 [[nodiscard]] final {};
A3 fa3();
void ga() { fa2(); fa3(); }
struct A6 [[nodiscard]] : N::B {};
template <class T> struct T1 {};
template <> struct T1<int> [[nodiscard]] {};
template <class T> struct T1<T *> alignas(8) final {};
struct A7 [[nodiscard]] *p7;
struct A8 alignas(4) alignas(8) {};
union A9 [[]] {};
