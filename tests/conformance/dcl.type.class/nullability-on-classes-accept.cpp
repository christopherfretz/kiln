// [dcl.type.class] (clang extension, nullability on classes): clang 19
// parses `_Nullable` after a class-key, mixed with the attributes there,
// on declarations, definitions, elaborated references and class template
// (partial) specializations. A class so marked may then carry a
// nullability qualifier as a pointer does, when any redeclaration is
// marked; a class template specialization when the primary template is.
//@ expect: compile-ok
//@ strict-diags
#if !__has_feature(nullability_on_classes)
#error "nullability_on_classes"
#endif

// The type-erased callable shape: a primary declaration and a partial
// specialization, both marked.
template <class Sig> class _Nullable Fn;
template <class R, class... A> class _Nullable Fn<R(A...)> {
public:
  R operator()(A...) const { return R(); }
};

class _Nullable C1;
struct _Nullable S1 {};
class _Nullable _Nullable C2;
class [[deprecated]] _Nullable C3;
class _Nullable [[deprecated]] C4;
class __attribute__((packed)) _Nullable C5 { char c; };
class _Nullable C6 final {};
class _Nullable { int a; } anon;
namespace n { class X; }
class _Nullable n::X {};

class Late;
class _Nullable Late {};
class _Nullable Early;
class Early {};

class Holder { friend class _Nullable C1; };
void take(class _Nullable C7 *p);
class _Nullable C8 *p8;

S1 _Nullable s1;
S1 _Nonnull s2;
S1 _Null_unspecified s3;
S1 *_Nullable s4;
Late _Nullable late;
Early _Nullable early;
n::X _Nullable nx;

template <class T> class _Nullable Box {};
Box<int> _Nullable box;
template <class T> using BoxAlias = Box<T>;
BoxAlias<long> _Nullable box_alias;
template <class T> void sink(Box<T> _Nullable);

int main() {
  Fn<int(int)> f;
  return f(1);
}
