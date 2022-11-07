#![cfg_attr(not(feature = "std"), no_std)]
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use frame_support::{
	pallet_prelude::DispatchResult,
	traits::{ChangeMembers, Get, InitializeMembers},
	BoundedVec,
};
use polimec_traits::{Credential, MemberRole, PolimecMembers};
use sp_runtime::{traits::StaticLookup, DispatchError};

type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Required origin for adding a member (though can always be Root).
		type AddOrigin: EnsureOrigin<Self::Origin>;

		/// Required origin for removing a member (though can always be Root).
		type RemoveOrigin: EnsureOrigin<Self::Origin>;

		/// Required origin for adding and removing a member in a single action.
		type SwapOrigin: EnsureOrigin<Self::Origin>;

		/// Required origin for resetting membership.
		type ResetOrigin: EnsureOrigin<Self::Origin>;

		/// Required origin for setting or resetting the prime member.
		type PrimeOrigin: EnsureOrigin<Self::Origin>;

		/// The receiver of the signal for when the membership has been initialized. This happens
		/// pre-genesis and will usually be the same as `MembershipChanged`. If you need to do
		/// something different on initialization, then you can change this accordingly.
		type MembershipInitialized: InitializeMembers<Self::AccountId>;

		/// The receiver of the signal for when the membership has changed.
		type MembershipChanged: ChangeMembers<Self::AccountId>;

		/// The maximum number of members that this membership can have.
		///
		/// This is used for benchmarking. Re-run the benchmarks if this changes.
		///
		/// This is enforced in the code; the membership size can not exceed this limit.
		type MaxMembersCount: Get<u32>;

		// Weight information for extrinsics in this pallet.
		// type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);
	/// Maps member type to members of each type.
	#[pallet::storage]
	#[pallet::getter(fn members)]
	pub type Members<T: Config> = StorageMap<
		_,
		Twox64Concat,
		MemberRole,
		BoundedVec<T::AccountId, T::MaxMembersCount>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The given member was added; see the transaction for who.
		MemberAdded,
		/// The given member was removed; see the transaction for who.
		MemberRemoved,
		/// Two members were swapped; see the transaction for who.
		MembersSwapped,
		/// The membership was reset; see the transaction for who the new set is.
		MembersReset,
		/// One of the members' keys changed.
		KeyChanged,
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Already a member.
		AlreadyMember,
		/// Not a member.
		NotMember,
		/// Too many members.
		TooManyMembers,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a member `who` to the set.
		///
		/// May only be called from `T::AddOrigin`.
		#[pallet::weight(50_000_000)]
		pub fn add_member(
			origin: OriginFor<T>,
			who: AccountIdLookupOf<T>,
			credential: Credential,
		) -> DispatchResult {
			T::AddOrigin::ensure_origin(origin)?;
			let who = T::Lookup::lookup(who)?;

			Self::do_add_member(&who, &credential)?;
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn do_add_member(who: &T::AccountId, credential: &Credential) -> Result<(), DispatchError> {
		let role = credential.role;

		<Members<T>>::try_mutate(role, |members| -> DispatchResult {
			let pos = members.binary_search(&who).err().ok_or(Error::<T>::AlreadyMember)?;
			members.try_insert(pos, who.clone()).map_err(|_| Error::<T>::TooManyMembers)?;
			Ok(())
		})?;

		// T::MembershipChanged::change_members_sorted(&[who], &[], &members[..]);

		Self::deposit_event(Event::MemberAdded);
		Ok(())
	}

	fn do_add_member_with_role(who: &T::AccountId, role: &MemberRole) -> Result<(), DispatchError> {
		<Members<T>>::try_mutate(role, |members| -> DispatchResult {
			let pos = members.binary_search(&who).err().ok_or(Error::<T>::AlreadyMember)?;
			members.try_insert(pos, who.clone()).map_err(|_| Error::<T>::TooManyMembers)?;
			Ok(())
		})?;

		// T::MembershipChanged::change_members_sorted(&[who], &[], &members[..]);

		Self::deposit_event(Event::MemberAdded);
		Ok(())
	}
}

impl<T: Config> PolimecMembers<T::AccountId> for Pallet<T> {
	fn is_in(who: &T::AccountId, role: &MemberRole) -> bool {
		let members = <Members<T>>::get(role);
		members.contains(who)
	}

	fn add_member(who: &T::AccountId, role: &MemberRole) -> Result<(), DispatchError> {
		Self::do_add_member_with_role(who, role)
	}
}
