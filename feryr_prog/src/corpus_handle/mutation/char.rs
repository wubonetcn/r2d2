//! Mutate value of `char` like types.
#[cfg(debug_assertions)]
use crate::corpus_handle::mutation::call::display_value_diff;
use crate::corpus_handle::{
    context::Context,
    gen::int::{gen_flags_bitmask, gen_flags_non_bitmask, gen_char, gen_proc},
    ty::CharType,
    value::Value,
    RngType,
};
use rand::prelude::*;

pub fn mutate_char(ctx: &mut Context, rng: &mut RngType, val: &mut Value) -> bool {
    let ty = val.ty(ctx.target);

    if rng.gen() {
        let new_val = gen_char(ctx, rng, ty, val.dir());
        // debug_info!(
        //     "mutate_char(gen): {}",
        //     display_value_diff(val, &new_val, ctx.target)
        // );
        let mutated = new_val.checked_as_char().val != val.checked_as_char().val;
        *val = new_val;
        return mutated;
    }

    let val = val.checked_as_char_mut();
    let ty = ty.checked_as_char();
    let bit_sz = ty.bit_size();
    let mut new_val = if ty.align() == 0 {
        do_mutate_char(rng, val.val, ty)
    } else {
        do_mutate_aligned_char(rng, val.val, ty)
    };
    if bit_sz < 64 {
        new_val &= (1 << bit_sz) - 1;
    }

    debug_info!("mutate_char: {:#x} -> {:#x}", val.val, new_val);
    let mutated = val.val != new_val;
    val.val = new_val;

    mutated
}

fn do_mutate_char(rng: &mut RngType, old_val: u64, ty: &IntType) -> u64 {
    if rng.gen_ratio(1, 3) {
        old_val.wrapping_add(rng.gen_range(1..=4))
    } else if rng.gen_ratio(1, 2) {
        old_val.wrapping_sub(rng.gen_range(1..=4))
    } else {
        let bit_sz = ty.bit_size();
        old_val ^ (1 << rng.gen_range(0..bit_sz))
    }
}

fn do_mutate_aligned_char(rng: &mut RngType, old_val: u64, ty: &IntType) -> u64 {
    let r = ty.range().cloned().unwrap_or(0..=u64::MAX);
    let start = *r.start();
    let mut end = *r.end();
    if start == 0 && end == u64::MAX {
        end = 1_u64.wrapping_shl(ty.bit_size() as u32).wrapping_sub(1);
    }
    let index = old_val.wrapping_sub(start) / ty.align();
    let miss = old_val.wrapping_sub(start) % ty.align();
    let mut index = do_mutate_char(rng, index, ty);
    let last_index = end.wrapping_sub(start) / ty.align();
    index %= last_index + 1;
    start
        .wrapping_add(index.wrapping_mul(ty.align()))
        .wrapping_add(miss)
}
 