#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod types;
pub use types::*;

use frame_support::traits::{Currency, Get, LockIdentifier, LockableCurrency, WithdrawReasons};
use sp_runtime::traits::{CheckedAdd, Zero};

/// The balance type of this pallet.
pub type BalanceOf<T> = <T as Config>::CurrencyBalance;

const LOCKING_ID: LockIdentifier = *b"evaluate";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::{ValueQuery, *};
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Identifier for the collection of item.
		type ProjectId: Member + Parameter + MaxEncodedLen + Copy;

		/// The maximum length of data stored on-chain.
		#[pallet::constant]
		type StringLimit: Get<u32>;

		/// The bonding balance.
		type Currency: LockableCurrency<
			Self::AccountId,
			Moment = Self::BlockNumber,
			Balance = Self::CurrencyBalance,
		>;

		/// Just the `Currency::Balance` type; we have this item to allow us to constrain it to
		/// `From<u64>`.
		type CurrencyBalance: sp_runtime::traits::AtLeast32BitUnsigned
			+ codec::FullCodec
			+ Copy
			+ MaybeSerializeDeserialize
			+ sp_std::fmt::Debug
			+ Default
			+ From<u64>
			+ TypeInfo
			+ MaxEncodedLen;

		#[pallet::constant]
		type EvaluationDuration: Get<Self::BlockNumber>;

		#[pallet::constant]
		type AuctionDuration: Get<Self::BlockNumber>;

		// Standard collection creation is only allowed if the origin attempting it and the
		// collection are in this set.

		// TODO: Should be helpful for allowing the calls only by the user in the set of
		// { Issuer, Retail, Professional, Institutional }

		// type CreateOrigin: EnsureOriginWithArg<
		//			Self::Origin,
		//	Self::CollectionId,
		//	Success = Self::AccountId,
		//>;

		// type ForceOrigin: EnsureOrigin<Self::Origin>;

		// Weight information for extrinsics in this pallet.
		// type WeightInfo: WeightInfo;
	}

	#[pallet::storage]
	#[pallet::getter(fn projects)]
	/// Information of a Project.
	pub type Projects<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::ProjectId,
		Project<T::AccountId, BoundedVec<u8, T::StringLimit>, T::BlockNumber>,
	>;

	#[pallet::storage]
	#[pallet::getter(fn evaluations)]
	/// Evaluation status of a Project.
	pub type Evaluations<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::ProjectId,
		EvaluationMetadata<T::BlockNumber, BalanceOf<T>>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn auctions)]
	/// Information of a Project.
	pub type Auctions<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::ProjectId,
		AuctionMetadata<T::BlockNumber, BalanceOf<T>>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn bonds)]
	/// Information of a Project.
	pub type Bonds<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::ProjectId,
		BondingLedger<T::AccountId, BalanceOf<T>>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A `project` was created.
		Created {
			project: T::ProjectId,
			issuer: T::AccountId,
		},
		/// Some `collection` was frozen.
		ProjectMetadataEdited(T::ProjectId, T::AccountId),
		EvaluationStarted(T::ProjectId, T::AccountId),
		EvaluationEndend(T::ProjectId, T::AccountId),
		AuctionStarted(T::ProjectId, T::AccountId, T::BlockNumber),
		AuctionEnded(T::ProjectId, T::AccountId),
		FundsBonded(T::ProjectId, T::AccountId, T::AccountId, BalanceOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		PriceTooLow,
		ParticipantsSizeError,
		TicketSizeError,
		ProjectIdInUse,
		ProjectNotExists,
		EvaluationAlreadyStarted,
		ContributionToThemself,
		EvaluationNotStarted,
		AuctionAlreadyStarted,
		Frozen,
		InsufficientBond,
		InsufficientBalance,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn create(
			origin: OriginFor<T>,
			project: Project<T::AccountId, BoundedVec<u8, T::StringLimit>, T::BlockNumber>,
			// TODO: Check if the "project_id" logic is correct.
			// from an UX PoV can this be a problem? Is there a better way to do it?
			project_id: T::ProjectId,
		) -> DispatchResult {
			// TODO: Ensure that the user is credentialized
			let issuer = ensure_signed(origin)?;

			ensure!(
				!Projects::<T>::contains_key(issuer.clone(), project_id),
				Error::<T>::ProjectIdInUse
			);

			match project.validity_check() {
				Err(error) => match error {
					ValidityError::PriceTooLow => Err(Error::<T>::PriceTooLow.into()),
					ValidityError::ParticipantsSizeError =>
						Err(Error::<T>::ParticipantsSizeError.into()),
					ValidityError::TicketSizeError => Err(Error::<T>::TicketSizeError.into()),
				},
				Ok(()) => Self::do_create(issuer, project, project_id),
			}
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn edit_metadata(
			origin: OriginFor<T>,
			project_metadata: ProjectMetadata<BoundedVec<u8, T::StringLimit>>,
			project_id: T::ProjectId,
		) -> DispatchResult {
			let issuer = ensure_signed(origin)?;
			ensure!(
				Projects::<T>::contains_key(issuer.clone(), project_id),
				Error::<T>::ProjectNotExists
			);
			ensure!(
				!Projects::<T>::get(issuer.clone(), project_id)
					.expect("The project exists")
					.is_frozen,
				Error::<T>::Frozen
			);
			Projects::<T>::mutate(issuer.clone(), project_id, |project| {
				project.as_mut().unwrap().metadata = project_metadata;
				Self::deposit_event(Event::<T>::ProjectMetadataEdited(project_id, issuer.clone()));
			});
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		/// Set the `evaluation_status` of a project to `EvaluationStatus::Started`
		pub fn start_evaluation(origin: OriginFor<T>, project_id: T::ProjectId) -> DispatchResult {
			let issuer = ensure_signed(origin)?;
			ensure!(
				Projects::<T>::contains_key(issuer.clone(), project_id),
				Error::<T>::ProjectNotExists
			);
			ensure!(
				Evaluations::<T>::get(issuer.clone(), project_id).evaluation_status ==
					EvaluationStatus::NotYetStarted,
				Error::<T>::EvaluationAlreadyStarted
			);
			Self::do_start_evaluation(issuer, project_id)
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn bond(
			origin: OriginFor<T>,
			project_issuer: T::AccountId,
			project_id: T::ProjectId,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;
			ensure!(
				Projects::<T>::contains_key(project_issuer.clone(), project_id),
				Error::<T>::ProjectNotExists
			);
			ensure!(from != project_issuer, Error::<T>::ContributionToThemself);
			ensure!(
				Evaluations::<T>::get(project_issuer.clone(), project_id).evaluation_status ==
					EvaluationStatus::Started,
				Error::<T>::EvaluationNotStarted
			);
			ensure!(T::Currency::free_balance(&from) > amount, Error::<T>::InsufficientBalance);

			// Reject a bond which is considered to be _dust_.
			// ensure!(amount > T::Currency::minimum_balance(), Error::<T>::InsufficientBond);

			T::Currency::set_lock(LOCKING_ID, &from, amount, WithdrawReasons::all());
			Bonds::<T>::insert(
				project_issuer.clone(),
				project_id,
				BondingLedger { stash: from.clone(), amount_bonded: amount },
			);
			Evaluations::<T>::mutate(project_issuer.clone(), project_id, |project| {
				project.amount_bonded =
					project.amount_bonded.checked_add(&amount).unwrap_or(project.amount_bonded)
			});
			Self::deposit_event(Event::<T>::FundsBonded(project_id, project_issuer, from, amount));
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn rebond(
			_origin: OriginFor<T>,
			_project_issuer: T::AccountId,
			_project_id: T::ProjectId,
			#[pallet::compact] _amount: BalanceOf<T>,
		) -> DispatchResult {
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		pub fn start_auction(origin: OriginFor<T>, project_id: T::ProjectId) -> DispatchResult {
			let issuer = ensure_signed(origin)?;
			ensure!(
				Projects::<T>::contains_key(issuer.clone(), project_id),
				Error::<T>::ProjectNotExists
			);
			ensure!(
				Auctions::<T>::get(issuer.clone(), project_id).auction_status ==
					AuctionStatus::NotYetStarted,
				Error::<T>::AuctionAlreadyStarted
			);
			Self::do_start_auction(issuer, project_id)
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(now: T::BlockNumber) {
			// TODO: Check if it's okay to iterate over an unbounded StorageDoubleMap.
			// I don't think so.
			for (project_issuer, project_id, mut project) in Evaluations::<T>::iter() {
				// Stop the evaluation period
				if project.evaluation_period_ends <= now &&
					project.evaluation_status == EvaluationStatus::Started
				{
					project.evaluation_status = EvaluationStatus::Ended;
				}
				// If more than 7 days are passed from the end of the evaluation, start the auction
				if project.evaluation_period_ends + T::AuctionDuration::get() <= now &&
					project.evaluation_status == EvaluationStatus::Ended &&
					todo!("Check if auction is not started yet")
				{
					Auctions::<T>::mutate(project_issuer.clone(), project_id, |auction| {
						auction.auction_status = AuctionStatus::Started;
						auction.auction_starting_block = now;
					});
					Self::deposit_event(Event::<T>::AuctionStarted(
						project_id,
						project_issuer,
						now,
					));
					// TODO: Remove the project from "Evaluations" storage
				}
			}
		}
	}
}

use frame_support::{pallet_prelude::DispatchError, BoundedVec};
// use sp_runtime::traits::Zero;

impl<T: Config> Pallet<T> {
	pub fn do_create(
		who: T::AccountId,
		project_info: Project<T::AccountId, BoundedVec<u8, T::StringLimit>, T::BlockNumber>,
		project_id: T::ProjectId,
	) -> Result<(), DispatchError> {
		Projects::<T>::insert(who.clone(), project_id, project_info);
		let current_block_number = <frame_system::Pallet<T>>::block_number();
		let evaluation_metadata = EvaluationMetadata {
			// When a project is created the evaluation phase doesn't start automatically
			evaluation_status: EvaluationStatus::NotYetStarted,
			evaluation_period_ends: current_block_number + T::EvaluationDuration::get(),
			amount_bonded: BalanceOf::<T>::zero(),
		};
		Evaluations::<T>::insert(who.clone(), project_id, evaluation_metadata);
		// TODO: Maybe rename `project_id` and `who` to project and issuer to use
		// the field init shorthand syntax
		Self::deposit_event(Event::<T>::Created { project: project_id, issuer: who });
		Ok(())
	}

	pub fn do_start_evaluation(
		who: T::AccountId,
		project_id: T::ProjectId,
	) -> Result<(), DispatchError> {
		Evaluations::<T>::try_mutate(who.clone(), project_id, |project_metadata| {
			// TODO: Get an element of `Projects` inside a `try_mutate()` of `Evaluations`, is it
			// ok?
			let mut project =
				Projects::<T>::get(&who, project_id).ok_or(Error::<T>::ProjectNotExists)?;
			project.is_frozen = true;
			project_metadata.evaluation_status = EvaluationStatus::Started;
			Self::deposit_event(Event::<T>::EvaluationStarted(project_id, who.clone()));
			let auction_metadata = AuctionMetadata {
				auction_status: AuctionStatus::NotYetStarted,
				// TODO: Proprely initiliaze every struct field and don't use default
				..Default::default()
			};
			Auctions::<T>::insert(who, project_id, auction_metadata);
			Ok(())
		})
	}

	pub fn do_start_auction(
		who: T::AccountId,
		project_id: T::ProjectId,
	) -> Result<(), DispatchError> {
		Auctions::<T>::try_mutate(who.clone(), project_id, |project| {
			let current_block_number = <frame_system::Pallet<T>>::block_number();
			project.auction_starting_block = current_block_number;
			project.auction_status = AuctionStatus::Started;
			Self::deposit_event(Event::<T>::AuctionStarted(project_id, who, current_block_number));
			Ok(())
		})
	}
}
