# WP-156 progress — simd_utils next layer (151 follow-ups)

Branch `wp/156-simd-next`, worktree `kiln/.worktrees/wp-156`, base 3539dd8.
clang 19 is the oracle for every item; each behaviour has one conformance
test, and codegen cases are in `tests/codegen/vector_{ops,abi}.cpp`
(`ir_vs_clang`, aarch64 and x86-64).

libc++ M3 gate before: 318 / 642 pass (54.4%).

## Items
1. [x] GNU/`__declspec` attribute between a lambda's parameter list and
   its specifiers (`kiln_parse`, simd_utils.h:114/:129).
2. [x] Vector operations (`kiln_sema/src/vector_ops.rs`):
   - unary, binary, shift, logical and comparison operators; scalar splats
     follow clang `CheckVectorOperands` / `checkVectorShift`
   - subscripts (an element lvalue whose address cannot be taken and
     which a non-const reference cannot bind)
   - list-initialization, lax casts between same-size vectors and integers
     (never between an integer and an Ext vector)
   - `__builtin_convertvector`, `__builtin_reduce_{add,mul,and,or,xor,min,max}`
   - codegen reproduces clang -O0 IR, including the AArch64 and x86-64
     vector calling conventions (`kiln_abi/src/call.rs`), scalar sret
     epilogues, and `K` in mangled vector pointees.
   - Tests: `ext.vector/{operators,operator-diagnostics,convertvector,
     convertvector-diagnostics,list-init}.cpp`.
3. [x] `vector_size` among the decl-specifiers applies to the
   decl-specifier type, so functions can return vectors; after a
   declarator it applies to the whole type, and GNU attributes after an
   array bound now reach the declarator. Tests:
   `ext.vector/return-type{,-diagnostics}.cpp`.
4. [x] "cannot initialize return object / a parameter" now points at the
   start of the operand (it pointed at a call's `(`).

5. [x] [temp.inst]/12: default arguments of members of class template
   specializations (and of their member templates) are instantiated at
   the first call that uses them (`kiln_sema/src/templ/default_args.rs`,
   `InstState::lazy_defaults`); a function-local class or lambda still
   instantiates them eagerly. A failed instantiation is retried, and
   reported again, at the next use, as clang does. The note is clang's
   "in instantiation of default function argument expression for 'f<B>'".
   Tests: `temp.inst/lazy-default-args{,-diagnostics,-retry}.cpp`.
6. [x] A default argument's conversion failure is reported at the
   parameter with "passing argument to parameter 'x' here"; copy-
   initialization of a class from a non-class expression only lists
   converting constructors (no default-constructor note). Test:
   `dcl.fct.default/conversion-diagnostics.cpp`.
7. [x] `T()` (value-initialization through a default constructor with
   default arguments) dropped the arguments: wrong constant values and an
   IR lowering failure. Classes nested in a class template specialization
   are named `%"struct.A<B>::In"` in IR, as clang names them. Test:
   `tests/codegen/nested_spec_record_name.cpp`.
8. [x] A dependent vector attribute is diagnosed at the attribute when
   substituted, and outside SFINAE the type is then left unchanged (the
   member typedef survives, no "no type named" cascade). Test:
   `ext.vector/substitution-diagnostics.cpp`.

## Not done (follow-ups)
- Oven folding of vector values (`kiln_oven/src/ty.rs` is owned by WP-154),
  and constant folding of vector binops.
- Vector `?:`, brace elision of vector members, `-Wpsabi`,
  `-Wtautological-compare` on vectors.
- Return types lose typedef sugar in diagnostics (`'TX'` prints as `'X'`),
  and "no viable conversion from returned value" uses the generic text.
