// [expr.const], spec/oven.md "Register locals": where kiln keeps the
// locals of Inputs/cases.h. An evaluation of a q_* function allocates no
// abstract memory (its locals are registers); one of an m_* function
// allocates its local (8 bytes, one granule). cases.cpp checks the values
// with every compiler. q_capture_copy allocates its closure (8 bytes)
// and the by-reference captures their closure and the local (16).
//@ oven: 35 bytes<=0
//@ oven: 36 bytes>=8
//@ oven: 37 bytes<=0
//@ oven: 38 bytes>=8
//@ oven: 39 bytes>=8
//@ oven: 40 bytes<=8
//@ oven: 41 bytes>=16
//@ oven: 42 bytes>=16
//@ oven: 43 bytes>=8
//@ oven: 44 bytes>=8
//@ oven: 45 bytes<=0
//@ oven: 46 bytes<=0
//@ oven: 47 bytes>=8
//@ oven: 48 bytes<=0
//@ oven: 49 bytes>=8
//@ oven: 50 bytes<=0
//@ oven: 51 bytes>=8
//@ oven: 52 bytes<=0
//@ oven: 53 bytes<=0
//@ oven: 54 bytes<=0
//@ oven: 55 bytes<=0
//@ oven: 56 bytes>=8
//@ oven: 57 bytes>=8
//@ oven: 58 bytes>=8
//@ oven: 59 bytes<=0
//@ oven: 60 bytes<=0
//@ oven: 61 bytes>=8
#include "Inputs/cases.h"
static_assert(q_addr(1) == 2);
static_assert(m_addr(1) == 2);
static_assert(q_ref(2) == 4);
static_assert(m_ref(1) == 2);
static_assert(m_ref_param(1) == 2);
static_assert(q_capture_copy(1) == 2);
static_assert(m_capture_ref(1) == 2);
static_assert(m_capture_default_ref(1) == 2);
static_assert(m_addressof(1) == 2);
static_assert(m_builtin_addressof(1) == 2);
static_assert(q_move(1, 2) == 21);
static_assert(q_forward(1) == 4);
static_assert(m_move_bound(1) == 2);
static_assert(q_unevaluated(1) == 9);
static_assert(m_unevaluated_and_ref(1) == 2);
static_assert(q_const_read(2) == 4);
static_assert(m_const_cast(1) == 2);
static_assert(q_assigned_first(3) == 6);
static_assert(q_assigned_both_branches(true) == 1);
static_assert(q_assigned_both_branches(false) == 2);
static_assert(q_assigned_each_iteration(4) == 6);
static_assert(m_assigned_one_branch(true) == 1);
static_assert(m_assigned_one_branch(false) == 2);
static_assert(m_case_in_scope(0) == 1);
static_assert(q_case_out_of_scope(0) == 1);
static_assert(q_reassigned(2) == 4);
static_assert(m_assigned_in_loop(5) == 5);
