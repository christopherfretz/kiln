// [cmp.categories.pre]: a std::strong_ordering that is not of the
// expected form is diagnosed when the builtin `<=>` needs it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:12 error "standard library implementation of 'std::strong_ordering' is not supported; member 'equal' is missing"
namespace std {
struct strong_ordering {
  int n;
  static const strong_ordering less;
};
}
auto a = 1 <=> 2;
