// Class template instantiation depth (spec/sema-templates.md §11.4): one
// chain of 1,000 nested specializations of `Wrap`, each complete only once
// its field's type is, so the instantiation stack reaches depth 1,000
// (under the default -ftemplate-depth=1024). No library headers and no
// partial or explicit specializations. Measures the instantiation stack,
// per-frame state and the sema stack's headroom.

template <class T> struct Wrap {
  T inner;
  static constexpr unsigned long depth = sizeof(T);
};

#define KILN_W2(T) Wrap<Wrap<T>>
#define KILN_W10(T) KILN_W2(KILN_W2(KILN_W2(KILN_W2(KILN_W2(T)))))
#define KILN_W100(T) KILN_W10(KILN_W10(KILN_W10(KILN_W10(KILN_W10(KILN_W10( \
  KILN_W10(KILN_W10(KILN_W10(KILN_W10(T))))))))))
#define KILN_W1000(T) KILN_W100(KILN_W100(KILN_W100(KILN_W100(KILN_W100( \
  KILN_W100(KILN_W100(KILN_W100(KILN_W100(KILN_W100(T))))))))))

using Deep = KILN_W1000(char);
Deep deep;
static_assert(sizeof(Deep) == 1);
