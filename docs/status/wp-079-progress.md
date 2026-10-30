# WP-079 progress (codegen for classes)

Updated: 2026-10-30 18:50 EDT

## State
- `kiln_codegen` builds (dead-code warnings: Cleanup::Nop, fir replace_op/successors).
- IR-diff corpus matching clang on aarch64 + x86_64: class_structors,
  class_temporaries, class_copy, class_members, class_aggregate_bases,
  class_static_members.
- Conformance WP-079 xfails passing at runtime: friends, member-functions,
  derived-to-base, builtin.bit-cast-run, static-members.
- Still failing: expr.mptr.oper/ptr-to-member.cpp, conv.mem/base-to-derived.cpp
  (need member pointers).

## Next step
Member pointer codegen (new section in class.rs): `&C::f` constants,
NullToMemberPointer, Base<->Derived member pointer casts, `.*`/`->*`,
call through mfp, ==/!=, MemberPointerToBoolean. Test:
tests/codegen/class_member_pointers.cpp (clang refs in scratch mp.*.ll).
Then flip xfails, conformance both modes, spec "Landed (WP-079)", exit checks.
