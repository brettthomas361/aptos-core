// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

use crate::{assert_success, MoveHarness};
use aptos::common::types::RotationProofChallenge;
use aptos_cached_packages::aptos_stdlib;
use aptos_crypto::{
    multi_ed25519::{MultiEd25519PrivateKey, MultiEd25519PublicKey},
    Signature, SigningKey, Uniform, ValidCryptoMaterial,
};
use aptos_language_e2e_tests::account::Account;
use aptos_types::{
    account_address::AccountAddress,
    account_config::{AccountResource, CORE_CODE_ADDRESS},
    state_store::{state_key::StateKey, table::TableHandle},
    transaction::authenticator::AuthenticationKey,
};
use move_core_types::parser::parse_struct_tag;

#[test]
fn rotate_auth_key_ed25519_to_ed25519() {
    let mut harness = MoveHarness::new();
    let account1 = harness.new_account_with_key_pair();

    let account2 = harness.new_account_with_key_pair();
    // assert that the payload is successfully processed (the signatures are correct)
    assert_successful_key_rotation_transaction(
        0,
        0,
        &mut harness,
        account1.clone(),
        *account1.address(),
        10,
        account2.privkey.clone(),
        account2.pubkey.clone(),
    );

    // verify that we can still get to account1's originating address
    verify_originating_address(&mut harness, account2.auth_key(), *account1.address(), 1);
}

#[test]
fn rotate_auth_key_ed25519_to_multi_ed25519() {
    let mut harness = MoveHarness::new();
    let account1 = harness.new_account_with_key_pair();

    let private_key = MultiEd25519PrivateKey::generate_for_testing();
    let public_key = MultiEd25519PublicKey::from(&private_key);
    let auth_key = AuthenticationKey::multi_ed25519(&public_key);

    // assert that the payload is successfully processed (the signatures are correct)
    assert_successful_key_rotation_transaction(
        0,
        1,
        &mut harness,
        account1.clone(),
        *account1.address(),
        10,
        private_key,
        public_key,
    );

    // verify that we can still get to account1's originating address
    verify_originating_address(&mut harness, auth_key.to_vec(), *account1.address(), 1);
}

#[test]
fn rotate_auth_key_twice() {
    let mut harness = MoveHarness::new();
    let mut account1 = harness.new_account_with_key_pair();

    let account2 = harness.new_account_with_key_pair();
    // assert that the payload is successfully processed (the signatures are correct)
    assert_successful_key_rotation_transaction(
        0,
        0,
        &mut harness,
        account1.clone(),
        *account1.address(),
        10,
        account2.privkey.clone(),
        account2.pubkey.clone(),
    );
    // rotate account1's keypair to account2
    account1.rotate_key(account2.privkey, account2.pubkey);
    // verify that we can still get to account1's originating address
    verify_originating_address(&mut harness, account1.auth_key(), *account1.address(), 1);

    let account3 = harness.new_account_with_key_pair();
    assert_successful_key_rotation_transaction(
        0,
        0,
        &mut harness,
        account1.clone(),
        *account1.address(),
        11,
        account3.privkey.clone(),
        account3.pubkey.clone(),
    );
    account1.rotate_key(account3.privkey, account3.pubkey);
    verify_originating_address(&mut harness, account1.auth_key(), *account1.address(), 2);
}

pub fn assert_successful_key_rotation_transaction<
    S: SigningKey + ValidCryptoMaterial,
    V: ValidCryptoMaterial,
>(
    from_scheme: u8,
    to_scheme: u8,
    harness: &mut MoveHarness,
    current_account: Account,
    originator: AccountAddress,
    sequence_number: u64,
    new_private_key: S,
    new_public_key: V,
) {
    // Construct a proof challenge struct that proves that
    // the user intends to rotate their auth key.
    let rotation_proof = RotationProofChallenge {
        account_address: CORE_CODE_ADDRESS,
        module_name: String::from("account"),
        struct_name: String::from("RotationProofChallenge"),
        sequence_number,
        originator,
        current_auth_key: AccountAddress::from_bytes(current_account.auth_key()).unwrap(),
        new_public_key: new_public_key.to_bytes().to_vec(),
    };

    let rotation_msg = bcs::to_bytes(&rotation_proof).unwrap();

    // Sign the rotation message by the current private key and the new private key.
    let signature_by_curr_privkey = current_account
        .privkey
        .sign_arbitrary_message(&rotation_msg);
    let signature_by_new_privkey = new_private_key.sign_arbitrary_message(&rotation_msg);

    assert_success!(harness.run_transaction_payload(
        &current_account,
        aptos_stdlib::account_rotate_authentication_key(
            from_scheme,
            current_account.pubkey.to_bytes().to_vec(),
            to_scheme,
            new_public_key.to_bytes().to_vec(),
            signature_by_curr_privkey.to_bytes().to_vec(),
            signature_by_new_privkey.to_bytes().to_vec(),
        )
    ));
}

pub fn verify_originating_address(
    harness: &mut MoveHarness,
    auth_key: Vec<u8>,
    expected_address: AccountAddress,
    expected_num_of_events: u64,
) {
    // Get the address redirection table
    let originating_address_handle = harness
        .read_resource::<TableHandle>(
            &CORE_CODE_ADDRESS,
            parse_struct_tag("0x1::account::OriginatingAddress").unwrap(),
        )
        .unwrap();
    let state_key = &StateKey::table_item(
        originating_address_handle,
        AccountAddress::from_bytes(auth_key).unwrap().to_vec(),
    );
    // Verify that the value in the address redirection table is expected
    let result = harness.read_state_value(state_key).unwrap();
    assert_eq!(result, expected_address.to_vec());

    let account_resource = parse_struct_tag("0x1::account::Account").unwrap();
    let key_rotation_events = harness
        .read_resource::<AccountResource>(&expected_address, account_resource)
        .unwrap()
        .key_rotation_events()
        .clone();

    assert_eq!(key_rotation_events.count(), expected_num_of_events);
}
