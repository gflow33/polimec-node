use did::DidVerificationKeyRelationship;
use pallet_dip_consumer::traits::DipCallOriginFilter;
use runtime_common::dip::{
	consumer::{DidMerkleProofVerifier, VerificationResult},
	ProofLeaf,
};
use sp_std::vec::Vec;

use crate::{
	AccountId, BlakeTwo256, BlockNumber, Hash, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin,
};

pub type DidIdentifier = AccountId;
pub type Hasher = BlakeTwo256;

impl pallet_dip_consumer::Config for Runtime {
	type BlindedValue = Vec<Vec<u8>>;
	type DipCallOriginFilter = DipCallFilter;
	type Identifier = DidIdentifier;
	type ProofLeaf = ProofLeaf<Hash, BlockNumber>;
	type ProofDigest = Hash;
	type ProofVerifier = DidMerkleProofVerifier<Hash, BlockNumber, Hasher>;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
}

fn derive_verification_key_relationship(
	call: &RuntimeCall,
) -> Option<DidVerificationKeyRelationship> {
	match call {
		RuntimeCall::DidLookup { .. } => Some(DidVerificationKeyRelationship::Authentication),
		RuntimeCall::Utility(pallet_utility::Call::batch { calls }) =>
			single_key_relationship(calls).ok(),
		RuntimeCall::Utility(pallet_utility::Call::batch_all { calls }) =>
			single_key_relationship(calls).ok(),
		RuntimeCall::Utility(pallet_utility::Call::force_batch { calls }) =>
			single_key_relationship(calls).ok(),
		_ => None,
	}
}

// Taken and adapted from `impl
// did::DeriveDidCallAuthorizationVerificationKeyRelationship for RuntimeCall`
// in Spiritnet/Peregrine runtime.
fn single_key_relationship(calls: &[RuntimeCall]) -> Result<DidVerificationKeyRelationship, ()> {
	let first_call_relationship =
		calls.get(0).and_then(derive_verification_key_relationship).ok_or(())?;
	calls.iter().skip(1).map(derive_verification_key_relationship).try_fold(
		first_call_relationship,
		|acc, next| {
			if next == Some(acc) {
				Ok(acc)
			} else {
				Err(())
			}
		},
	)
}

pub struct DipCallFilter;

impl DipCallOriginFilter<RuntimeCall> for DipCallFilter {
	type Error = ();
	type Proof = VerificationResult<BlockNumber>;
	type Success = ();

	// Accepts only a DipOrigin for the DidLookup pallet calls.
	fn check_proof(call: RuntimeCall, proof: Self::Proof) -> Result<Self::Success, Self::Error> {
		let key_relationship = single_key_relationship(&[call])?;
		if proof.0.iter().any(|l| l.relationship == key_relationship.into()) {
			Ok(())
		} else {
			Err(())
		}
	}
}

#[cfg(test)]
mod dip_call_origin_filter_tests {
	use super::*;

	use frame_support::assert_err;

	#[test]
	fn test_key_relationship_derivation() {
		// Can call DidLookup functions with an authentication key
		let did_lookup_call = RuntimeCall::DidLookup(pallet_did_lookup::Call::associate_sender {});
		assert_eq!(
			single_key_relationship(&[did_lookup_call]),
			Ok(DidVerificationKeyRelationship::Authentication)
		);
		// Can't call System functions with a DID key (hence a DIP origin)
		let system_call = RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
		assert_err!(single_key_relationship(&[system_call]), ());
		// Can't call empty batch with a DID key
		let empty_batch_call =
			RuntimeCall::Utility(pallet_utility::Call::batch_all { calls: vec![] });
		assert_err!(single_key_relationship(&[empty_batch_call]), ());
		// Can call batch with a DipLookup with an authentication key
		let did_lookup_batch_call = RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![pallet_did_lookup::Call::associate_sender {}.into()],
		});
		assert_eq!(
			single_key_relationship(&[did_lookup_batch_call]),
			Ok(DidVerificationKeyRelationship::Authentication)
		);
		// Can't call a batch with different required keys
		let did_lookup_batch_call = RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![
				// Authentication key
				pallet_did_lookup::Call::associate_sender {}.into(),
				// No key
				frame_system::Call::remark { remark: vec![] }.into(),
			],
		});
		assert_err!(single_key_relationship(&[did_lookup_batch_call]), ());
	}
}