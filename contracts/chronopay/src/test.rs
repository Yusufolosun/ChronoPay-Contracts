#![cfg(test)]

use super::*;
use soroban_sdk::{vec, Env, String};

fn setup() -> (Env, ChronoPayContractClient<'static>) {
    let env = Env::default();
    let id = env.register(ChronoPayContract, ());
    let client = ChronoPayContractClient::new(&env, &id);
    (env, client)
}

fn alice(env: &Env) -> String {
    String::from_str(env, "alice")
}

fn bob(env: &Env) -> String {
    String::from_str(env, "bob")
}

// ── hello ─────────────────────────────────────────────────────────────────────

#[test]
fn test_hello() {
    let (env, client) = setup();
    let words = client.hello(&String::from_str(&env, "Dev"));
    assert_eq!(
        words,
        vec![
            &env,
            String::from_str(&env, "ChronoPay"),
            String::from_str(&env, "Dev"),
        ]
    );
}

// ── create_time_slot: happy paths ─────────────────────────────────────────────

#[test]
fn test_slot_ids_auto_increment() {
    let (env, client) = setup();
    let a = alice(&env);
    assert_eq!(client.create_time_slot(&a, &1000, &2000), 1);
    assert_eq!(client.create_time_slot(&a, &2000, &3000), 2);
    assert_eq!(client.create_time_slot(&a, &5000, &6000), 3);
}

#[test]
fn test_adjacent_slots_are_allowed() {
    // [1000,2000) then [2000,3000) — they touch but do not overlap
    let (env, client) = setup();
    let a = alice(&env);
    client.create_time_slot(&a, &1000, &2000);
    client.create_time_slot(&a, &2000, &3000); // must not panic
}

#[test]
fn test_gap_between_slots_is_allowed() {
    let (env, client) = setup();
    let a = alice(&env);
    client.create_time_slot(&a, &1000, &2000);
    client.create_time_slot(&a, &3000, &4000);
}

#[test]
fn test_different_professionals_same_window_allowed() {
    // Overlap enforcement is per-professional; alice and bob may share a window.
    let (env, client) = setup();
    client.create_time_slot(&alice(&env), &1000, &2000);
    client.create_time_slot(&bob(&env), &1000, &2000); // must not panic
}

// ── create_time_slot: error paths ─────────────────────────────────────────────

#[test]
#[should_panic]
fn test_invalid_range_start_equals_end() {
    let (env, client) = setup();
    client.create_time_slot(&alice(&env), &1000, &1000);
}

#[test]
#[should_panic]
fn test_invalid_range_start_after_end() {
    let (env, client) = setup();
    client.create_time_slot(&alice(&env), &2000, &1000);
}

#[test]
#[should_panic]
fn test_exact_duplicate_slot_rejected() {
    let (env, client) = setup();
    let a = alice(&env);
    client.create_time_slot(&a, &1000, &2000);
    client.create_time_slot(&a, &1000, &2000); // identical → overlap
}

#[test]
#[should_panic]
fn test_overlap_new_starts_inside_existing() {
    // existing [1000,3000), new [2000,4000) — overlaps
    let (env, client) = setup();
    let a = alice(&env);
    client.create_time_slot(&a, &1000, &3000);
    client.create_time_slot(&a, &2000, &4000);
}

#[test]
#[should_panic]
fn test_overlap_new_ends_inside_existing() {
    // existing [2000,4000), new [1000,3000) — overlaps
    let (env, client) = setup();
    let a = alice(&env);
    client.create_time_slot(&a, &2000, &4000);
    client.create_time_slot(&a, &1000, &3000);
}

#[test]
#[should_panic]
fn test_overlap_new_contains_existing() {
    // existing [1500,2500), new [1000,3000) — new wraps existing
    let (env, client) = setup();
    let a = alice(&env);
    client.create_time_slot(&a, &1500, &2500);
    client.create_time_slot(&a, &1000, &3000);
}

#[test]
#[should_panic]
fn test_overlap_existing_contains_new() {
    // existing [1000,3000), new [1500,2500) — new is inside existing
    let (env, client) = setup();
    let a = alice(&env);
    client.create_time_slot(&a, &1000, &3000);
    client.create_time_slot(&a, &1500, &2500);
}

// ── mint / buy / redeem ───────────────────────────────────────────────────────

#[test]
fn test_mint_returns_time_token_symbol() {
    let (env, client) = setup();
    let slot_id = client.create_time_slot(&alice(&env), &1000, &2000);
    let token = client.mint_time_token(&slot_id);
    assert_eq!(token, soroban_sdk::Symbol::new(&env, "TIME_TOKEN"));
}

#[test]
fn test_buy_time_token_returns_true() {
    let (env, client) = setup();
    let token = soroban_sdk::Symbol::new(&env, "TIME_TOKEN");
    assert!(client.buy_time_token(&token, &alice(&env), &bob(&env)));
}

#[test]
fn test_redeem_time_token_returns_true() {
    let (env, client) = setup();
    let token = soroban_sdk::Symbol::new(&env, "TIME_TOKEN");
    assert!(client.redeem_time_token(&token));
}
