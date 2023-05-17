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

use frame_support::{pallet_prelude::*, traits::tokens::Balance as BalanceT};
use sp_arithmetic::traits::Saturating;
use sp_runtime::traits::CheckedDiv;
use sp_std::cmp::Eq;

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct ProjectMetadata<BoundedString, Balance: BalanceT, Hash> {
	/// Token Metadata
	pub token_information: CurrencyMetadata<BoundedString>,
	/// Total allocation of Contribution Tokens available for the Funding Round
	pub total_allocation_size: Balance,
	/// Minimum price per Contribution Token
	pub minimum_price: Balance,
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
	pub participation_currencies: Currencies,
	/// Additional metadata
	pub offchain_information_hash: Option<Hash>,
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct ProjectDetails<BlockNumber, Balance: BalanceT> {
	/// Whether the project is frozen, so no `metadata` changes are allowed.
	pub is_frozen: bool,
	/// The price decided after the Auction Round
	pub weighted_average_price: Option<Balance>,
	/// The current status of the project
	pub project_status: ProjectStatus,
	/// When the different project phases start and end
	pub phase_transition_points: PhaseTransitionPoints<BlockNumber>,
	/// Fundraising target amount in USD equivalent
	pub fundraising_target: Balance,
	/// The amount of Contribution Tokens that have not yet been sold
	pub remaining_contribution_tokens: Balance,
}

#[derive(Debug)]
pub enum ValidityError {
	PriceTooLow,
	TicketSizeError,
	ParticipantsSizeError,
}

impl<BoundedString, Balance: BalanceT, Hash> ProjectMetadata<BoundedString, Balance, Hash> {
	// TODO: PLMC-162. Perform a REAL validity check
	pub fn validity_check(&self) -> Result<(), ValidityError> {
		if self.minimum_price == Balance::zero() {
			return Err(ValidityError::PriceTooLow);
		}
		self.ticket_size.is_valid()?;
		self.participants_size.is_valid()?;
		Ok(())
	}
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct TicketSize<Balance: BalanceT> {
	pub minimum: Option<Balance>,
	pub maximum: Option<Balance>,
}

impl<Balance: BalanceT> TicketSize<Balance> {
	fn is_valid(&self) -> Result<(), ValidityError> {
		if self.minimum.is_some() && self.maximum.is_some() {
			if self.minimum < self.maximum {
				return Ok(());
			} else {
				return Err(ValidityError::TicketSizeError);
			}
		}
		if self.minimum.is_some() || self.maximum.is_some() {
			return Ok(());
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
	fn is_valid(&self) -> Result<(), ValidityError> {
		match (self.minimum, self.maximum) {
			(Some(min), Some(max)) => {
				if min < max && min > 0 && max > 0 {
					Ok(())
				} else {
					Err(ValidityError::ParticipantsSizeError)
				}
			}
			(Some(elem), None) | (None, Some(elem)) => {
				if elem > 0 {
					Ok(())
				} else {
					Err(ValidityError::ParticipantsSizeError)
				}
			}
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
	start: Option<BlockNumber>,
	end: Option<BlockNumber>,
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

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct BidInfo<BidId, ProjectId, Balance: BalanceT, AccountId, BlockNumber, PlmcVesting, CTVesting> {
	pub bid_id: BidId,
	pub project: ProjectId,
	#[codec(compact)]
	pub amount: Balance,
	#[codec(compact)]
	pub price: Balance,
	#[codec(compact)]
	pub ticket_size: Balance,
	// Removed due to only being used in the price calculation, and it's not really needed there
	// pub ratio: Option<Perbill>,
	pub when: BlockNumber,
	pub bidder: AccountId,
	// TODO: PLMC-159. Not used yet, but will be used to check if the bid is funded after XCM is implemented
	pub funded: bool,
	pub plmc_vesting_period: PlmcVesting,
	pub ct_vesting_period: CTVesting,
	pub status: BidStatus<Balance>,
}

impl<BidId, ProjectId, Balance: BalanceT, AccountId, BlockNumber, PlmcVesting, CTVesting>
	BidInfo<BidId, ProjectId, Balance, AccountId, BlockNumber, PlmcVesting, CTVesting>
{
	pub fn new(
		bid_id: BidId, project: ProjectId, amount: Balance, price: Balance, when: BlockNumber, bidder: AccountId,
		plmc_vesting_period: PlmcVesting, ct_vesting_period: CTVesting,
	) -> Self {
		let ticket_size = amount.saturating_mul(price);
		Self {
			bid_id,
			project,
			amount,
			price,
			ticket_size,
			// ratio: None,
			when,
			bidder,
			funded: false,
			plmc_vesting_period,
			ct_vesting_period,
			status: BidStatus::YetUnknown,
		}
	}
}

impl<BidId: Eq, ProjectId: Eq, Balance: BalanceT, AccountId: Eq, BlockNumber: Eq, PlmcVesting: Eq, CTVesting: Eq>
	sp_std::cmp::Ord for BidInfo<BidId, ProjectId, Balance, AccountId, BlockNumber, PlmcVesting, CTVesting>
{
	fn cmp(&self, other: &Self) -> sp_std::cmp::Ordering {
		self.price.cmp(&other.price)
	}
}

impl<BidId: Eq, ProjectId: Eq, Balance: BalanceT, AccountId: Eq, BlockNumber: Eq, PlmcVesting: Eq, CTVesting: Eq>
	sp_std::cmp::PartialOrd for BidInfo<BidId, ProjectId, Balance, AccountId, BlockNumber, PlmcVesting, CTVesting>
{
	fn partial_cmp(&self, other: &Self) -> Option<sp_std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct ContributionInfo<Balance, PLMCVesting, CTVesting> {
	// Tokens you paid in exchange for CTs
	pub contribution_amount: Balance,
	pub plmc_vesting: PLMCVesting,
	pub ct_vesting: CTVesting,
}

// TODO: PLMC-157. Use SCALE fixed indexes
#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum Currencies {
	DOT,
	KSM,
	#[default]
	USDC,
	USDT,
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum ProjectStatus {
	#[default]
	Application,
	EvaluationRound,
	AuctionInitializePeriod,
	EvaluationFailed,
	AuctionRound(AuctionPhase),
	CommunityRound,
	RemainderRound,
	FundingEnded,
	ReadyToLaunch,
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum AuctionPhase {
	#[default]
	English,
	Candle,
}

#[derive(Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
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

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum RejectionReason {
	/// The bid was submitted after the candle auction ended
	AfterCandleEnd,
	/// The bid was accepted but too many tokens were requested. A partial amount was accepted
	NoTokensLeft,
}

/// Enum used to identify PLMC named reserves
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Copy, Ord, PartialOrd)]
pub enum BondType {
	Evaluation,
	Bidding,
	Contributing,
	LongTermHolderBonus,
	Staking,
	Governance,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct EvaluationBond<ProjectId, AccountId, Balance, BlockNumber> {
	pub project: ProjectId,
	pub account: AccountId,
	pub amount: Balance,
	pub when: BlockNumber,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct BiddingBond<ProjectId, AccountId, Balance, BlockNumber> {
	pub project: ProjectId,
	pub account: AccountId,
	pub amount: Balance,
	pub when: BlockNumber,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ContributingBond<ProjectId, AccountId, Balance> {
	pub project: ProjectId,
	pub account: AccountId,
	pub amount: Balance,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Vesting<BlockNumber: Copy, Balance> {
	// Amount of tokens vested
	pub amount: Balance,
	// number of blocks after project ends, when vesting starts
	pub start: BlockNumber,
	// number of blocks after project ends, when vesting ends
	pub end: BlockNumber,
	// number of blocks between each withdrawal
	pub step: BlockNumber,
	// absolute block number of next block where withdrawal is possible
	pub next_withdrawal: BlockNumber,
}

impl<
		BlockNumber: Saturating + Copy + CheckedDiv,
		Balance: Saturating + CheckedDiv + Copy + From<u32> + Eq + sp_std::ops::SubAssign,
	> Vesting<BlockNumber, Balance>
{
	pub fn calculate_next_withdrawal(&mut self) -> Result<Balance, ()> {
		if self.amount == 0u32.into() {
			Err(())
		} else {
			let next_withdrawal = self.next_withdrawal.saturating_add(self.step);
			let withdraw_amount = self.amount;
			self.next_withdrawal = next_withdrawal;
			self.amount -= withdraw_amount;
			Ok(withdraw_amount)
		}
	}
}

/// Tells on_initialize what to do with the project
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Copy, Ord, PartialOrd)]
pub enum UpdateType {
	EvaluationEnd,
	EnglishAuctionStart,
	CandleAuctionStart,
	CommunityFundingStart,
	RemainderFundingStart,
	FundingEnd,
}