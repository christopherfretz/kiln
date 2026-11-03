// [dcl.pre]/5, [dcl.attr.grammar]/5, [class.pre]/1, [class.friend]/1,
// [stmt.pre]/1, [namespace.udecl]/1, [namespace.udir]/1, [enum.udecl]/1,
// [dcl.link]/2, [dcl.asm]/1: more leading attribute-specifier-seq
// placements, as clang 19 reports them (see
// leading-attribute-placement.cpp): attribute-declarations, friend
// declarations, using forms, linkage specifications, block-scope asm, and
// the place-it warning for a tag declaration's attributes.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 63:1 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 65:3 error "'noreturn' attribute only applies to functions"
//@ diag: 66:16 warning "attribute 'packed' is ignored, place it after \"struct\" to apply attribute to type declaration [-Wignored-attributes]"
//@ diag: 67:33 error "misplaced attributes; expected attributes here"
//@ diag: 68:19 error "misplaced attributes; expected attributes here"
//@ diag: 69:20 error "misplaced attributes; expected attributes here"
//@ diag: 70:3 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 72:3 error "an attribute list cannot appear here"
//@ diag: 74:3 error "an attribute list cannot appear here"
//@ diag: 75:5 warning "attribute 'nodiscard' is ignored, place it after \"enum class\" to apply attribute to type declaration [-Wignored-attributes]"
//@ diag: 76:18 warning "attribute 'packed' is ignored, place it after \"struct\" to apply attribute to type declaration [-Wignored-attributes]"
//@ diag: 79:5 error "'nodiscard' attribute cannot be applied to a statement"
//@ diag: 80:5 error "'nodiscard' attribute cannot be applied to a statement"
//@ diag: 81:5 error "'nodiscard' attribute cannot be applied to a statement"
//@ diag: 82:3 error "an attribute list cannot appear here"
//@ diag: 83:5 error "'nodiscard' attribute cannot be applied to a statement"
//@ diag: 85:5 error "'nodiscard' attribute cannot be applied to a statement"
//@ diag: 85:16 error "'deprecated' attribute cannot be applied to a statement"
//@ diag: 87:1 error "an attribute list cannot appear here"
//@ diag: 88:3 warning "ISO C++ does not allow an attribute list to appear here [-Wcxx-attribute-extension]"
//@ diag: 88:14 warning "ISO C++ does not allow an attribute list to appear here [-Wcxx-attribute-extension]"
//@ diag: 88:3 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 88:14 warning "'deprecated' currently has no effect on a using declaration [-Wignored-attributes]"
//@ diag: 89:1 warning "ISO C++ does not allow an attribute list to appear here [-Wcxx-attribute-extension]"
//@ diag: 89:14 warning "ISO C++ does not allow an attribute list to appear here [-Wcxx-attribute-extension]"
//@ diag: 89:1 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 89:14 warning "'nodiscard' attribute only applies to Objective-C methods, enums, structs, unions, classes, functions, function pointers, and typedefs [-Wignored-attributes]"
//@ diag: 91:3 error "an attribute list cannot appear here"
//@ diag: 92:3 error "an attribute list cannot appear here"
//@ diag: 93:3 error "an attribute list cannot appear here"
//@ diag: 94:3 error "an attribute list cannot appear here"
//@ diag: 97:1 error "'alignas' attribute only applies to variables, data members and tag types"
//@ diag: 100:1 error "an attribute list cannot appear here"
//@ diag: 100:15 warning "declaration does not declare anything [-Wmissing-declarations]"
//@ diag: 101:3 warning "ISO C++ does not allow an attribute list to appear here [-Wcxx-attribute-extension]"
//@ diag: 102:3 warning "ISO C++ does not allow an attribute list to appear here [-Wcxx-attribute-extension]"
//@ diag: 104:13 warning "declaration does not declare anything [-Wmissing-declarations]"
//@ diag: 105:15 warning "declaration does not declare anything [-Wmissing-declarations]"
//@ diag: 106:5 warning "attribute 'unused' is ignored, place it after \"struct\" to apply attribute to type declaration [-Wignored-attributes]"
//@ diag: 109:3 error "an attribute list cannot appear here"
//@ diag: 109:17 warning "declaration does not declare anything [-Wmissing-declarations]"
//@ diag: 110:3 error "an attribute list cannot appear here"
//@ diag: 111:5 warning "'nodiscard' attribute ignored [-Wignored-attributes]"
//@ diag: 113:23 error "misplaced attributes; expected attributes here"
//@ diag: 115:21 error "'nodiscard' attribute cannot be applied to types"
//@ diag: 115:21 warning "attribute 'nodiscard' is ignored, place it after \"enum struct\" to apply attribute to type declaration [-Wignored-attributes]"
//@ diag: 116:1 error "an attribute list cannot appear here"
//@ diag: 117:1 error "an attribute list cannot appear here"
//@ diag: 118:1 error "an attribute list cannot appear here"
//@ requires: clang<22
// clang 19 spelling; leading-attribute-placement-2-clang22.cpp has clang 22's (WP-394).
struct Y;
alignas(4);
[[deprecated]];
[[noreturn]];
__attribute__((packed)) struct G1 {};
[[nodiscard, deprecated]] struct S1 {};
[[nodiscard]] enum class E1 {};
[[nodiscard]] union U1 {};
[[nodiscard]] struct S2 {} s2;
struct C {
  [[nodiscard]] friend class Y;
  [[nodiscard]] friend int fd() { return 0; }
  [[nodiscard]] friend int fe();
  [[nodiscard]] enum class E2 {};
  __attribute__((packed)) struct G2 {};
};
void blk() {
  [[nodiscard]] { }
  [[nodiscard]] if (true) {}
  [[nodiscard]] return;
  alignas(4) struct S3 {};
  [[nodiscard]] blk();
  alignas(4) int x;
  [[nodiscard, deprecated]];
}
[[nodiscard, deprecated]] using X1 = int;
[[nodiscard, deprecated]] using ::blk;
alignas(4) [[nodiscard]] using ::blk;
struct C3 {
  [[nodiscard, deprecated]] friend int fe();
  [[nodiscard]] [[deprecated]] friend int ff();
  alignas(4) friend int fg();
  [[deprecated]] friend struct Z;
};
namespace N3 { void f(); }
alignas(4) using namespace N3;
[[deprecated]] using namespace N3;
namespace N4 { void f(); enum E { a }; int g(); }
[[nodiscard]] int;
[[clang::using_if_exists]] using N4::g;
[[nodiscard]] using enum N4::E;
struct C4 {
  alignas(4);
  [[noreturn]];
  [[gnu::unused]] struct S8 {};
};
void blk4() {
  [[nodiscard]] int;
  [[gnu::unused]] struct S9 {};
  [[nodiscard]] asm("");
}
[[gnu::unused]] struct S7 {};
enum class [[nodiscard]] E5 {};
enum struct E6 {} [[nodiscard]];
[[nodiscard]] extern "C" int f1();
[[nodiscard]] extern "C++" { int f2(); }
alignas(4) extern "C" int v;
[[nodiscard]] __extension__ int f3();
