// Polimec Blockchain – https://www.polimec.org/
// Copyright (C) Polimec 2022. All rights reserved.

// The Polimec Blockchain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The Polimec Blockchain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// If you feel like getting in touch with us, you can do so at info@polimec.org

//! Types for Funding pallet.

use crate::{
	traits::{BondingRequirementCalculation, ProvideStatemintPrice},
	BalanceOf,
};
use frame_support::{pallet_prelude::*, traits::tokens::Balance as BalanceT};
use sp_arithmetic::{traits::Saturating, FixedPointNumber, FixedPointOperand};
use sp_runtime::traits::CheckedDiv;
use sp_std::{cmp::Eq, collections::btree_map::*, prelude::*};

pub use config_types::*;
pub use inner_types::*;
pub use storage_types::*;

pub mod config_types {
	use super::*;

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Copy, Ord, PartialOrd)]
	pub struct Multiplier<T: crate::Config>(pub T::Balance);
	impl<T: crate::Config> BondingRequirementCalculation<T> for Multiplier<T> {
		fn calculate_bonding_requirement(&self, ticket_size: BalanceOf<T>) -> Result<BalanceOf<T>, ()> {
			ticket_size.checked_div(&self.0).ok_or(())
		}
	}
	impl<T: crate::Config> Default for Multiplier<T> {
		fn default() -> Self {
			Self(1u32.into())
		}
	}
	impl<T: crate::Config> From<u32> for Multiplier<T> {
		fn from(x: u32) -> Self {
			Self(x.into())
		}
	}

	/// Enum used to identify PLMC holds.
	/// It implements Serialize and Deserialize (only in the "std" feature set) to hold a fungible in the Genesis Configuration.
	#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, MaxEncodedLen, TypeInfo, Ord, PartialOrd)]
	#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
	pub enum LockType<ProjectId> {
		Evaluation(ProjectId),
		Participation(ProjectId),
	}

	pub struct ConstPriceProvider<AssetId, Price, Mapping>(PhantomData<(AssetId, Price, Mapping)>);
	impl<AssetId: Ord, Price: FixedPointNumber + Clone, Mapping: Get<BTreeMap<AssetId, Price>>> ProvideStatemintPrice
		for ConstPriceProvider<AssetId, Price, Mapping>
	{
		type AssetId = AssetId;
		type Price = Price;

		fn get_price(asset_id: AssetId) -> Option<Price> {
			Mapping::get().get(&asset_id).cloned()
		}
	}
}

pub mod storage_types {
	use super::*;

	#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct ProjectMetadata<BoundedString, Balance: BalanceT, Price: FixedPointNumber, AccountId, Hash> {
		/// Token Metadata
		pub token_information: CurrencyMetadata<BoundedString>,
		/// Mainnet Token Max Supply
		pub mainnet_token_max_supply: Balance,
		/// Total allocation of Contribution Tokens available for the Funding Round
		pub total_allocation_size: Balance,
		/// Minimum price per Contribution Token
		pub minimum_price: Price,
		/// Maximum and/or minimum ticket size
		pub ticket_size: TicketSize<Balance>,
		/// Maximum and/or minimum number of participants for the Auction and Community Round
		pub participants_size: ParticipantsSize,
		/// Funding round thresholds for Retail, Professional and Institutional participants
		pub funding_thresholds: Thresholds,
		/// Conversion rate of contribution token to mainnet token
		pub conversion_rate: u32,
		/// Participation currencies (e.g stablecoin, DOT, KSM)
		/// e.g. https://github.com/paritytech/substrate/blob/427fd09bcb193c1e79dec85b1e207c718b686c35/frame/uniques/src/types.rs#L110
		/// For now is easier to handle the case where only just one Currency is accepted
		pub participation_currencies: AcceptedFundingAsset,
		pub funding_destination_account: AccountId,
		/// Additional metadata
		pub offchain_information_hash: Option<Hash>,
	}
	impl<BoundedString, Balance: BalanceT, Price: FixedPointNumber, Hash, AccountId>
		ProjectMetadata<BoundedString, Balance, Price, Hash, AccountId>
	{
		// TODO: PLMC-162. Perform a REAL validity check
		pub fn validity_check(&self) -> Result<(), ValidityError> {
			if self.minimum_price == Price::zero() {
				return Err(ValidityError::PriceTooLow)
			}
			self.ticket_size.is_valid()?;
			self.participants_size.is_valid()?;
			Ok(())
		}
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct ProjectDetails<AccountId, BlockNumber, Price: FixedPointNumber, Balance: BalanceT, EvaluationRoundInfo> {
		pub issuer: AccountId,
		/// Whether the project is frozen, so no `metadata` changes are allowed.
		pub is_frozen: bool,
		/// The price in USD per token decided after the Auction Round
		pub weighted_average_price: Option<Price>,
		/// The current status of the project
		pub status: ProjectStatus,
		/// When the different project phases start and end
		pub phase_transition_points: PhaseTransitionPoints<BlockNumber>,
		/// Fundraising target amount in USD equivalent
		pub fundraising_target: Balance,
		/// The amount of Contribution Tokens that have not yet been sold
		pub remaining_contribution_tokens: Balance,
		/// Funding reached amount in USD equivalent
		pub funding_amount_reached: Balance,
		/// Cleanup operations remaining
		pub cleanup: Cleaner,
		/// Information about the total amount bonded, and the outcome in regards to reward/slash/nothing
		pub evaluation_round_info: EvaluationRoundInfo,
	}

	/// Tells on_initialize what to do with the project
	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum UpdateType {
		EvaluationEnd,
		EnglishAuctionStart,
		CandleAuctionStart,
		CommunityFundingStart,
		RemainderFundingStart,
		FundingEnd,
		ProjectDecision(FundingOutcomeDecision),
		StartSettlement,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Ord, PartialOrd)]
	pub struct EvaluationInfo<Id, ProjectId, AccountId, Balance, BlockNumber> {
		pub id: Id,
		pub project_id: ProjectId,
		pub evaluator: AccountId,
		pub original_plmc_bond: Balance,
		// An evaluation bond can be converted to participation bond
		pub current_plmc_bond: Balance,
		pub early_usd_amount: Balance,
		pub late_usd_amount: Balance,
		pub when: BlockNumber,
		pub rewarded_or_slashed: bool,
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct BidInfo<
		Id,
		ProjectId,
		Balance: BalanceT,
		Price: FixedPointNumber,
		AccountId,
		BlockNumber,
		Multiplier,
		VestingInfo
	> {
		pub id: Id,
		pub project_id: ProjectId,
		pub bidder: AccountId,
		pub status: BidStatus<Balance>,
		#[codec(compact)]
		pub original_ct_amount: Balance,
		pub original_ct_usd_price: Price,
		pub final_ct_amount: Balance,
		pub final_ct_usd_price: Price,
		pub funding_asset: AcceptedFundingAsset,
		pub funding_asset_amount_locked: Balance,
		pub multiplier: Multiplier,
		pub plmc_bond: Balance,
		pub plmc_vesting_info: VestingInfo,
		pub funded: bool,
		pub when: BlockNumber,
		pub funds_released: bool,
		pub ct_minted: bool,
	}

	impl<
			BidId: Eq,
			ProjectId: Eq,
			Balance: BalanceT + FixedPointOperand + Ord,
			Price: FixedPointNumber,
			AccountId: Eq,
			BlockNumber: Eq + Ord,
			Multiplier: Eq,
			VestingInfo: Eq
		> Ord for BidInfo<BidId, ProjectId, Balance, Price, AccountId, BlockNumber, Multiplier, VestingInfo>
	{
		fn cmp(&self, other: &Self) -> sp_std::cmp::Ordering {
			match self.original_ct_usd_price.cmp(&other.original_ct_usd_price) {
				sp_std::cmp::Ordering::Equal => Ord::cmp(&self.when, &other.when),
				other => other,
			}
		}
	}

	impl<
			BidId: Eq,
			ProjectId: Eq,
			Balance: BalanceT + FixedPointOperand,
			Price: FixedPointNumber,
			AccountId: Eq,
			BlockNumber: Eq + Ord,
			Multiplier: Eq,
			VestingInfo: Eq
		> PartialOrd for BidInfo<BidId, ProjectId, Balance, Price, AccountId, BlockNumber, Multiplier, VestingInfo>
	{
		fn partial_cmp(&self, other: &Self) -> Option<sp_std::cmp::Ordering> {
			Some(self.cmp(other))
		}
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct ContributionInfo<Id, ProjectId, AccountId, Balance, VestingInfo> {
		pub id: Id,
		pub project_id: ProjectId,
		pub contributor: AccountId,
		pub ct_amount: Balance,
		pub usd_contribution_amount: Balance,
		pub funding_asset: AcceptedFundingAsset,
		pub funding_asset_amount: Balance,
		pub plmc_bond: Balance,
		pub plmc_vesting_info: VestingInfo,
		pub funds_released: bool,
		pub ct_minted: bool,
	}
}

pub mod inner_types {
	use super::*;

	#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct CurrencyMetadata<BoundedString> {
		/// The user friendly name of this asset. Limited in length by `StringLimit`.
		pub name: BoundedString,
		/// The ticker symbol for this asset. Limited in length by `StringLimit`.
		pub symbol: BoundedString,
		/// The number of decimals this asset uses to represent one unit.
		pub decimals: u8,
	}

	#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct TicketSize<Balance: BalanceT> {
		pub minimum: Option<Balance>,
		pub maximum: Option<Balance>,
	}
	impl<Balance: BalanceT> TicketSize<Balance> {
		pub(crate) fn is_valid(&self) -> Result<(), ValidityError> {
			if self.minimum.is_some() && self.maximum.is_some() {
				return if self.minimum < self.maximum { Ok(()) } else { Err(ValidityError::TicketSizeError) }
			}
			if self.minimum.is_some() || self.maximum.is_some() {
				return Ok(())
			}

			Err(ValidityError::TicketSizeError)
		}
	}

	#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct ParticipantsSize {
		pub minimum: Option<u32>,
		pub maximum: Option<u32>,
	}
	impl ParticipantsSize {
		pub(crate) fn is_valid(&self) -> Result<(), ValidityError> {
			match (self.minimum, self.maximum) {
				(Some(min), Some(max)) =>
					if min < max && min > 0 && max > 0 {
						Ok(())
					} else {
						Err(ValidityError::ParticipantsSizeError)
					},
				(Some(elem), None) | (None, Some(elem)) =>
					if elem > 0 {
						Ok(())
					} else {
						Err(ValidityError::ParticipantsSizeError)
					},
				(None, None) => Err(ValidityError::ParticipantsSizeError),
			}
		}
	}

	#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct Thresholds {
		#[codec(compact)]
		retail: u8,
		#[codec(compact)]
		professional: u8,
		#[codec(compact)]
		institutional: u8,
	}

	// TODO: PLMC-157. Use SCALE fixed indexes
	#[derive(Default, Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum AcceptedFundingAsset {
		#[default]
		USDT,
		USDC,
		DOT,
	}
	impl AcceptedFundingAsset {
		pub fn to_statemint_id(&self) -> u32 {
			match self {
				AcceptedFundingAsset::USDT => 1984,
				AcceptedFundingAsset::DOT => 0,
				AcceptedFundingAsset::USDC => 420,
			}
		}
	}

	#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum ProjectStatus {
		#[default]
		Application,
		EvaluationRound,
		EvaluationFailed,
		AuctionInitializePeriod,
		AuctionRound(AuctionPhase),
		CommunityRound,
		RemainderRound,
		FundingFailed,
		AwaitingProjectDecision,
		FundingSuccessful,
		ReadyToLaunch,
	}

	#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum AuctionPhase {
		#[default]
		English,
		Candle,
	}

	#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct PhaseTransitionPoints<BlockNumber> {
		pub application: BlockNumberPair<BlockNumber>,
		pub evaluation: BlockNumberPair<BlockNumber>,
		pub auction_initialize_period: BlockNumberPair<BlockNumber>,
		pub english_auction: BlockNumberPair<BlockNumber>,
		pub random_candle_ending: Option<BlockNumber>,
		pub candle_auction: BlockNumberPair<BlockNumber>,
		pub community: BlockNumberPair<BlockNumber>,
		pub remainder: BlockNumberPair<BlockNumber>,
	}

	#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
	pub struct BlockNumberPair<BlockNumber> {
		pub start: Option<BlockNumber>,
		pub end: Option<BlockNumber>,
	}

	impl<BlockNumber: Copy> BlockNumberPair<BlockNumber> {
		pub fn new(start: Option<BlockNumber>, end: Option<BlockNumber>) -> Self {
			Self { start, end }
		}

		pub fn start(&self) -> Option<BlockNumber> {
			self.start
		}

		pub fn end(&self) -> Option<BlockNumber> {
			self.end
		}

		pub fn update(&mut self, start: Option<BlockNumber>, end: Option<BlockNumber>) {
			let new_state = match (start, end) {
				(Some(start), None) => (Some(start), self.end),
				(None, Some(end)) => (self.start, Some(end)),
				(Some(start), Some(end)) => (Some(start), Some(end)),
				(None, None) => (self.start, self.end),
			};
			(self.start, self.end) = (new_state.0, new_state.1);
		}

		pub fn force_update(&mut self, start: Option<BlockNumber>, end: Option<BlockNumber>) -> Self {
			Self { start, end }
		}
	}

	#[derive(Debug)]
	pub enum ValidityError {
		PriceTooLow,
		TicketSizeError,
		ParticipantsSizeError,
	}

	#[derive(Default, Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum BidStatus<Balance: BalanceT> {
		/// The bid is not yet accepted or rejected
		#[default]
		YetUnknown,
		/// The bid is accepted
		Accepted,
		/// The bid is rejected, and the reason is provided
		Rejected(RejectionReason),
		/// The bid is partially accepted. The amount accepted and reason for rejection are provided
		PartiallyAccepted(Balance, RejectionReason),
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum RejectionReason {
		/// The bid was submitted after the candle auction ended
		AfterCandleEnd,
		/// The bid was accepted but too many tokens were requested. A partial amount was accepted
		NoTokensLeft,
		/// Error in calculating ticket_size for partially funded request
		BadMath,
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct VestingInfo<BlockNumber, Balance> {
		// Amount of tokens vested
		pub amount_per_block: Balance,
		// number of blocks after project ends, when vesting starts
		pub start: BlockNumber,
		// number of blocks after project ends, when vesting ends
		pub end: BlockNumber,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum FundingOutcome {
		Success(SuccessReason),
		Failure(FailureReason),
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum SuccessReason {
		SoldOut,
		ReachedTarget,
		ProjectDecision,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum FailureReason {
		EvaluationFailed,
		AuctionFailed,
		TargetNotReached,
		ProjectDecision,
		Unknown,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct Success;

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct Failure;

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum CleanerState<T> {
		Initialized(PhantomData<T>),
		// Success or Failure
		EvaluationRewardOrSlash(u64, PhantomData<T>),
		EvaluationUnbonding(u64, PhantomData<T>),
		// Branch
		// A. Success only
		BidPLMCVesting(u64, PhantomData<T>),
		BidCTMint(u64, PhantomData<T>),
		ContributionPLMCVesting(u64, PhantomData<T>),
		ContributionCTMint(u64, PhantomData<T>),
		BidFundingPayout(u64, PhantomData<T>),
		ContributionFundingPayout(u64, PhantomData<T>),
		// B. Failure only
		BidFundingRelease(u64, PhantomData<T>),
		BidUnbonding(u64, PhantomData<T>),
		ContributionFundingRelease(u64, PhantomData<T>),
		ContributionUnbonding(u64, PhantomData<T>),
		// Merge
		// Success or Failure
		Finished(PhantomData<T>),
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum Cleaner {
		NotReady,
		Success(CleanerState<Success>),
		Failure(CleanerState<Failure>),
	}
	impl TryFrom<ProjectStatus> for Cleaner {
		type Error = ();

		fn try_from(value: ProjectStatus) -> Result<Self, ()> {
			match value {
				ProjectStatus::FundingSuccessful => Ok(Cleaner::Success(CleanerState::Initialized(PhantomData))),
				ProjectStatus::FundingFailed | ProjectStatus::EvaluationFailed =>
					Ok(Cleaner::Failure(CleanerState::Initialized(PhantomData))),
				_ => Err(()),
			}
		}
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct EvaluationRoundInfo<Balance> {
		pub total_bonded_usd: Balance,
		pub total_bonded_plmc: Balance,
		pub evaluators_outcome: EvaluatorsOutcome<Balance>,
	}

	#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum EvaluatorsOutcome<Balance> {
		Unchanged,
		Rewarded(RewardInfo<Balance>),
		Slashed,
	}

	#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct RewardInfo<Balance> {
		pub early_evaluator_reward_pot_usd: Balance,
		pub normal_evaluator_reward_pot_usd: Balance,
		pub early_evaluator_total_bonded_usd: Balance,
		pub normal_evaluator_total_bonded_usd: Balance,
	}

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum FundingOutcomeDecision {
		AcceptFunding,
		RejectFunding,
	}
}
