#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>

#[cfg(feature = "std")]
use frame_support::traits::GenesisBuild;
use frame_support::{
	pallet_prelude::*, PalletId,
	log,
	traits::{Currency, ReservableCurrency, ExistenceRequirement, WithdrawReasons},
};
use codec::{Encode, Decode};
use sp_std::prelude::*;
use integer_sqrt::IntegerSquareRoot;
use sp_runtime::traits::AccountIdConversion;
pub use pallet::*;
use scale_info::TypeInfo;


#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::*;

const MAX_STRING_FIELD_LENGTH: usize = 256;

#[frame_support::pallet]
pub mod pallet {
	use frame_system::pallet_prelude::*;
	use frame_support::pallet_prelude::*;
	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_identity::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type PalletId: Get<PalletId>;

		type Currency: ReservableCurrency<Self::AccountId>;

		type MaxProposalsPerRound: Get<u32>;

		type MaxWithdrawalExpiration: Get<Self::BlockNumber>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::storage]
	#[pallet::getter(fn projects)]
	pub type Projects<T> = StorageMap<_, Blake2_128Concat, ProjectIndex, Option<ProjectOf<T>>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn project_count)]
	pub type ProjectCount<T> = StorageValue<_, ProjectIndex, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn rounds)]
	pub type Rounds<T> = StorageMap<_, Blake2_128Concat, RoundIndex, Option<RoundOf<T>>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn round_count)]
	pub type RoundCount<T> = StorageValue<_, RoundIndex, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn max_proposal_count_per_round)]
	pub type MaxProposalCountPerRound<T> = StorageValue<_, u32, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn withdrawal_expiration)]
	pub type WithdrawalExpiration<T> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn is_identity_required)]
	pub type IsIdentityRequired<T> = StorageValue<_, bool, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub init_max_proposal_count_per_round: u32,
		pub init_withdrawal_expiration: BlockNumberFor<T>,
		pub init_is_identity_required: bool,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				init_max_proposal_count_per_round: 5,
				init_withdrawal_expiration: Default::default(),
				init_is_identity_required: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			MaxProposalCountPerRound::<T>::put(self.init_max_proposal_count_per_round);
			WithdrawalExpiration::<T>::put(self.init_withdrawal_expiration);
			IsIdentityRequired::<T>::put(self.init_is_identity_required);
		}
	}

	// Pallets use events to inform users when important changes are made.
	// https://substrate.dev/docs/en/knowledgebase/runtime/events
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		ProjectCreated(ProjectIndex),
		RoundCreated(RoundIndex),
		ContributeSucceed(T::AccountId, ProjectIndex, BalanceOf<T>, T::BlockNumber),
		ProposalCanceled(RoundIndex, ProjectIndex),
		ProposalWithdrawn(RoundIndex, ProjectIndex, BalanceOf<T>, BalanceOf<T>),
		ProposalApproved(RoundIndex, ProjectIndex),
		RoundCanceled(RoundIndex),
		FundSucceed(),
		RoundFinalized(RoundIndex),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		StorageOverflow,
		/// There was an overflow.
		Overflow,
		///
		RoundStarted,
		RoundNotEnded,
		StartBlockNumberInvalid,
		EndBlockNumberInvalid,
		EndTooEarly,
		NoActiveRound,
		NoActiveProposal,
		InvalidParam,
		ProposalCanceled,
		ProposalWithdrawn,
		ProposalApproved,
		ProposalNotApproved,
		InvalidAccount,
		IdentityNeeded,
		StartBlockNumberTooSmall,
		RoundNotProcessing,
		RoundCanceled,
		RoundFinalized,
		RoundNotFinalized,
		ProposalAmountExceed,
		WithdrawalExpirationExceed,
		NotEnoughFund,
		InvalidProjectIndexes,
		ParamLimitExceed,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create project
		#[pallet::weight(<T as Config>::WeightInfo::create_project())]
		pub fn create_project(origin: OriginFor<T>, name: Vec<u8>, logo: Vec<u8>, description: Vec<u8>, website: Vec<u8>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			// Check if identity is required
			let is_identity_needed = IsIdentityRequired::<T>::get();
			if is_identity_needed {
				let identity = pallet_identity::Pallet::<T>::identity(who.clone()).ok_or(Error::<T>::IdentityNeeded)?;
				let mut is_found_judgement = false;
				for judgement in identity.judgements.iter() {
					if judgement.1 == pallet_identity::Judgement::Reasonable || judgement.1 == pallet_identity::Judgement::KnownGood {
						is_found_judgement = true;
						break;
					}
				}
				ensure!(is_found_judgement, Error::<T>::IdentityNeeded);
			}

			// Validation
			ensure!(name.len() > 0, Error::<T>::InvalidParam);
			ensure!(logo.len() > 0, Error::<T>::InvalidParam);
			ensure!(description.len() > 0, Error::<T>::InvalidParam);
			ensure!(website.len() > 0, Error::<T>::InvalidParam);

			// ensure!(name.len() <= MAX_STRING_FIELD_LENGTH, Error::<T>::ParamLimitExceed);
			// ensure!(logo.len() <= MAX_STRING_FIELD_LENGTH, Error::<T>::ParamLimitExceed);
			// ensure!(description.len() <= MAX_STRING_FIELD_LENGTH, Error::<T>::ParamLimitExceed);
			// ensure!(website.len() <= MAX_STRING_FIELD_LENGTH, Error::<T>::ParamLimitExceed);
			
			let index = ProjectCount::<T>::get();
			let next_index = index.checked_add(1).ok_or(Error::<T>::Overflow)?;

			// Create a proposal 
			let project = ProjectOf::<T> {
				name: name,
				logo: logo,
				description: description,
				website: website,
				owner: who,
				create_block_number: <frame_system::Pallet<T>>::block_number(),
			};

			// Add proposal to list
			<Projects<T>>::insert(index, Some(project));
			ProjectCount::<T>::put(next_index);

			Self::deposit_event(Event::ProjectCreated(index));

			Ok(().into())
		}

		/// Funding to matching fund pool
		#[pallet::weight(<T as Config>::WeightInfo::fund())]
		pub fn fund(origin: OriginFor<T>, fund_balance: BalanceOf<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(fund_balance > (0 as u32).into(), Error::<T>::InvalidParam);

			// Transfer matching fund to module account
			// No fees are paid here if we need to create this account; that's why we don't just
			// use the stock `transfer`.
			<T as Config>::Currency::resolve_creating(&Self::account_id(), <T as Config>::Currency::withdraw(
				&who,
				fund_balance,
				WithdrawReasons::from(WithdrawReasons::TRANSFER),
				ExistenceRequirement::AllowDeath,
			)?);

			Self::deposit_event(Event::FundSucceed());

			Ok(().into())
		}

		/// Schedule a round
		/// proposal_indexes: the proposals were selected for this round
		#[pallet::weight(<T as Config>::WeightInfo::schedule_round(MaxProposalCountPerRound::<T>::get()))]
		pub fn schedule_round(origin: OriginFor<T>, start: T::BlockNumber, end: T::BlockNumber, matching_fund: BalanceOf<T>, project_indexes: Vec<ProjectIndex>) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let now = <frame_system::Pallet<T>>::block_number();

			// Check whether the funds are sufficient
			ensure!(project_indexes.len() > 0, Error::<T>::InvalidProjectIndexes);
			// The number of items cannot exceed the maximum
			ensure!(project_indexes.len() as u32 <= MaxProposalCountPerRound::<T>::get(), Error::<T>::ProposalAmountExceed);
			// The end block must be greater than the start block
			ensure!(end > start, Error::<T>::EndTooEarly);
			// Both the starting block number and the ending block number must be greater than the current number of blocks
			ensure!(start > now, Error::<T>::StartBlockNumberInvalid);
			ensure!(end > now, Error::<T>::EndBlockNumberInvalid);

			// project_index should be smaller than project count
			let project_count = ProjectCount::<T>::get();
			for project_index in project_indexes.iter() {
				ensure!(*project_index < project_count, Error::<T>::InvalidProjectIndexes);
			}

			// Find the last valid round
			let mut last_valid_round: Option<RoundOf::<T>> = None;
			let index = RoundCount::<T>::get();

			for _i in (0..index).rev() {
				let round = <Rounds<T>>::get(index-1).unwrap();
				if !round.is_canceled {
					last_valid_round = Some(round);
					break;
				}
			}

			// The start time must be greater than the end time of the last valid round
			match last_valid_round {
				Some(round) => {
					ensure!(start > round.end, Error::<T>::StartBlockNumberTooSmall);
				},
				None => {}
			}

			let next_index = index.checked_add(1).ok_or(Error::<T>::Overflow)?;

			let round = RoundOf::<T>::new(start, end, matching_fund, project_indexes);

			// Add proposal round to list
			<Rounds<T>>::insert(index, Some(round));
			RoundCount::<T>::put(next_index);

			Self::deposit_event(Event::RoundCreated(index));

			Ok(().into())
		}

		/// Cancel a round
		/// This round must have not started yet
		#[pallet::weight(<T as Config>::WeightInfo::cancel_round())]
		pub fn cancel_round(origin: OriginFor<T>, round_index: RoundIndex) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let now = <frame_system::Pallet<T>>::block_number();
			let count = RoundCount::<T>::get();
			let mut round = <Rounds<T>>::get(round_index).ok_or(Error::<T>::NoActiveRound)?;

			// Ensure current round is not started
			ensure!(round.start > now, Error::<T>::RoundStarted);
			// This round cannot be cancelled
			ensure!(!round.is_canceled, Error::<T>::RoundCanceled);

			round.is_canceled = true;
			<Rounds<T>>::insert(round_index, Some(round.clone()));

			Self::deposit_event(Event::RoundCanceled(count-1));

			Ok(().into())
		}

		/// Finalize a round
		#[pallet::weight(<T as Config>::WeightInfo::finalize_round())]
		pub fn finalize_round(origin: OriginFor<T>, round_index: RoundIndex) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let now = <frame_system::Pallet<T>>::block_number();
			let mut round = <Rounds<T>>::get(round_index).ok_or(Error::<T>::NoActiveRound)?;
			
			// This round cannot be cancelled or finalized
			ensure!(!round.is_canceled, Error::<T>::RoundCanceled);
			ensure!(!round.is_finalized, Error::<T>::RoundFinalized);
			// This round must be over
			ensure!(now > round.end, Error::<T>::RoundNotEnded);

			let mut proposal_clrs: Vec<BalanceOf<T>> = Vec::new();
			let mut total_clr: BalanceOf<T> = (0 as u32).into();

			// Calculate proposal CLR
			let proposals = &mut round.proposals;
			
			// Calculates project CLRs and total ClR
			for i in 0..proposals.len() {
				let proposal = &proposals[i];

				if proposal.is_canceled {
					proposal_clrs.push((0 as u32).into());
					continue;
				} 

				let mut sqrt_sum: BalanceOf<T> = (0 as u32).into();
				for contribution in proposal.contributions.iter() {
					let contribution_value: BalanceOf<T> = contribution.value;
					sqrt_sum += contribution_value.integer_sqrt();
				}

				let proposal_clr: BalanceOf<T> = sqrt_sum * sqrt_sum;
				proposal_clrs.push(proposal_clr);
				total_clr += proposal_clr;
			}

			// Calculate proposal matching fund
			if total_clr != (0 as u32).into() {
				for i in 0..proposals.len() {
					let proposal = &mut proposals[i];
	
					if proposal.is_canceled {
						continue;
					} 
	
					proposal.matching_fund = round.matching_fund * proposal_clrs[i] / total_clr;
				}
			}

			round.is_finalized = true;
			<Rounds<T>>::insert(round_index, Some(round.clone()));

			Self::deposit_event(Event::RoundFinalized(round_index));

			Ok(().into())
		}

		/// Contribute a proposal
		#[pallet::weight(<T as Config>::WeightInfo::contribute())]
		pub fn contribute(origin: OriginFor<T>, project_index: ProjectIndex, value: BalanceOf<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(value > (0 as u32).into(), Error::<T>::InvalidParam);
			let project_count = ProjectCount::<T>::get();
			ensure!(project_index < project_count, Error::<T>::InvalidParam);
			let now = <frame_system::Pallet<T>>::block_number();
			
			// round list must be not none
			let round_index = RoundCount::<T>::get();
			ensure!(round_index > 0, Error::<T>::NoActiveRound);

			// Find processing round
			let mut processing_round: Option<RoundOf::<T>> = None;
			for i in (0..round_index).rev() {
				let round = <Rounds<T>>::get(i).unwrap();
				if !round.is_canceled && round.start < now && round.end > now {
					processing_round = Some(round);
				}
			}

			let mut round = processing_round.ok_or(Error::<T>::RoundNotProcessing)?;

			// Find proposal by index
			let mut found_proposal: Option<&mut ProposalOf::<T>> = None;
			for proposal in round.proposals.iter_mut() {
				if proposal.project_index == project_index {
					found_proposal = Some(proposal);
					break;
				}
			}

			let proposal = found_proposal.ok_or(Error::<T>::NoActiveProposal)?;
			ensure!(!proposal.is_canceled, Error::<T>::ProposalCanceled);

			// Find previous contribution by account_id
			// If you have contributed before, then add to that contribution. Otherwise join the list.
			let mut found_contribution: Option<&mut ContributionOf::<T>> = None;
			for contribution in proposal.contributions.iter_mut() {
				if contribution.account_id == who {
					found_contribution = Some(contribution);
					break;
				}
			}

			match found_contribution {
				Some(contribution) => {
					contribution.value += value;
				},
				None => {
					proposal.contributions.push(ContributionOf::<T> {
						account_id: who.clone(),
						value: value,
					});
				}
			}

			// Transfer contribute to proposal account
			<T as Config>::Currency::transfer(
				&who,
				&Self::project_account_id(project_index),
				value,
				ExistenceRequirement::AllowDeath
			)?;
			
			<Rounds<T>>::insert(round_index-1, Some(round));

			Self::deposit_event(Event::ContributeSucceed(who, project_index, value, now));

			Ok(().into())
		}

		/// Approve project
		/// If the project is approve, the project owner can withdraw funds
		#[pallet::weight(<T as Config>::WeightInfo::approve())]
		pub fn approve(origin: OriginFor<T>, round_index: RoundIndex, project_index: ProjectIndex) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let mut round = <Rounds<T>>::get(round_index).ok_or(Error::<T>::NoActiveRound)?;
			ensure!(round.is_finalized, Error::<T>::RoundNotFinalized);
			ensure!(!round.is_canceled, Error::<T>::RoundCanceled);
			let proposals = &mut round.proposals;

			// The round must have ended
			let now = <frame_system::Pallet<T>>::block_number();
			// This round must be over
			ensure!(round.end < now, Error::<T>::RoundNotEnded);

			// Find proposal from list
			let mut found_proposal: Option<&mut ProposalOf::<T>> = None;
			for proposal in proposals.iter_mut() {
				if proposal.project_index == project_index {
					found_proposal = Some(proposal);
					break;
				}
			}
			let mut proposal = found_proposal.ok_or(Error::<T>::NoActiveProposal)?;

			// Can't let users vote in the cancered round
			ensure!(!proposal.is_canceled, Error::<T>::ProposalCanceled);
			ensure!(!proposal.is_approved, Error::<T>::ProposalApproved);

			// set is_approved
			proposal.is_approved = true;
			proposal.withdrawal_expiration = now + <WithdrawalExpiration<T>>::get();

			<Rounds<T>>::insert(round_index, Some(round.clone()));

			Self::deposit_event(Event::ProposalApproved(round_index, project_index));
			
			Ok(().into())
		}

		/// Withdraw
		#[pallet::weight(<T as Config>::WeightInfo::withdraw())]
		pub fn withdraw(origin: OriginFor<T>, round_index: RoundIndex, project_index: ProjectIndex) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let now = <frame_system::Pallet<T>>::block_number();

			// Only project owner can withdraw
			let project = Projects::<T>::get(project_index).ok_or(Error::<T>::NoActiveProposal)?;
			ensure!(who == project.owner, Error::<T>::InvalidAccount);

			let mut round = <Rounds<T>>::get(round_index).ok_or(Error::<T>::NoActiveRound)?;
			let mut found_proposal: Option<&mut ProposalOf::<T>> = None;
			for proposal in round.proposals.iter_mut() {
				if proposal.project_index == project_index {
					found_proposal = Some(proposal);
					break;
				}
			}

			let proposal = found_proposal.ok_or(Error::<T>::NoActiveProposal)?;
			ensure!(now <= proposal.withdrawal_expiration, Error::<T>::WithdrawalExpirationExceed);

			// This proposal must not have distributed funds
			ensure!(proposal.is_approved, Error::<T>::ProposalNotApproved);
			ensure!(!proposal.is_withdrawn, Error::<T>::ProposalWithdrawn);

			// Calculate contribution amount
			let mut contribution_amount: BalanceOf<T>  = (0 as u32).into();
			for contribution in proposal.contributions.iter() {
				let contribution_value = contribution.value;
				contribution_amount += contribution_value;
			}

			let matching_fund = proposal.matching_fund;

			// Distribute CLR amount
			// Return funds to caller without charging a transfer fee
			let _ = <T as Config>::Currency::resolve_into_existing(&project.owner, <T as Config>::Currency::withdraw(
				&Self::account_id(),
				matching_fund,
				WithdrawReasons::from(WithdrawReasons::TRANSFER),
				ExistenceRequirement::AllowDeath,
			)?);

			// Distribute contribution amount
			let _ = <T as Config>::Currency::resolve_into_existing(&project.owner, <T as Config>::Currency::withdraw(
				&Self::project_account_id(project_index),
				contribution_amount,
				WithdrawReasons::from(WithdrawReasons::TRANSFER),
				ExistenceRequirement::AllowDeath,
			)?);


			// Set is_withdrawn
			proposal.is_withdrawn = true;
			proposal.withdrawal_expiration = now + <WithdrawalExpiration<T>>::get();

			<Rounds<T>>::insert(round_index, Some(round.clone()));

			Self::deposit_event(Event::ProposalWithdrawn(round_index, project_index, matching_fund, contribution_amount));

			Ok(().into())
		}

		/// Cancel a problematic project
		/// If the project is cancelled, users cannot donate to it, and project owner cannot withdraw funds.
		#[pallet::weight(<T as Config>::WeightInfo::cancel())]
		pub fn cancel(origin: OriginFor<T>, round_index: RoundIndex, project_index: ProjectIndex) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			let mut round = <Rounds<T>>::get(round_index).ok_or(Error::<T>::NoActiveRound)?;

			// This round cannot be cancelled or finalized
			ensure!(!round.is_canceled, Error::<T>::RoundCanceled);
			ensure!(!round.is_finalized, Error::<T>::RoundFinalized);

			let proposals = &mut round.proposals;

			let mut found_proposal: Option<&mut ProposalOf::<T>> = None;

			// Find proposal with project index
			for proposal in proposals.iter_mut() {
				if proposal.project_index == project_index {
					found_proposal = Some(proposal);
					break;
				}
			}

			let proposal = found_proposal.ok_or(Error::<T>::NoActiveProposal)?;

			// This proposal must not have canceled
			ensure!(!proposal.is_canceled, Error::<T>::ProposalCanceled);
			ensure!(!proposal.is_approved, Error::<T>::ProposalApproved);

			proposal.is_canceled = true;

			Rounds::<T>::insert(round_index, Some(round));

			Self::deposit_event(Event::ProposalCanceled(round_index, project_index));

			Ok(().into())
		}

		/// Set max proposal count per round
		#[pallet::weight(<T as Config>::WeightInfo::set_max_proposal_count_per_round(T::MaxProposalsPerRound::get()))]
		pub fn set_max_proposal_count_per_round(origin: OriginFor<T>, max_proposal_count_per_round: u32) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			ensure!(max_proposal_count_per_round > 0 || max_proposal_count_per_round <= T::MaxProposalsPerRound::get(), Error::<T>::ParamLimitExceed);
			MaxProposalCountPerRound::<T>::put(max_proposal_count_per_round);

			Ok(().into())
		}

		/// Set withdrawal expiration
		#[pallet::weight(<T as Config>::WeightInfo::set_withdrawal_expiration())]
		pub fn set_withdrawal_expiration(origin: OriginFor<T>, withdrawal_expiration: T::BlockNumber) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			ensure!(withdrawal_expiration > (0 as u32).into(), Error::<T>::InvalidParam);
			<WithdrawalExpiration<T>>::put(withdrawal_expiration);

			Ok(().into())
		}

		/// set is_identity_required
		#[pallet::weight(<T as Config>::WeightInfo::set_is_identity_required())]
		pub fn set_is_identity_required(origin: OriginFor<T>, is_identity_required: bool) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			IsIdentityRequired::<T>::put(is_identity_required);

			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// The account ID of the fund pot.
	///
	/// This actually does computation. If you need to keep using it, then make sure you cache the
	/// value and only call this once.
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account()
	}

	pub fn project_account_id(index: ProjectIndex) -> T::AccountId {
		T::PalletId::get().into_sub_account(index)
	}

	/// Get all projects
	pub fn get_projects() -> Vec<Project<AccountIdOf<T>, T::BlockNumber>> {
		let len = ProjectCount::<T>::get();
		let mut projects: Vec<Project<AccountIdOf<T>, T::BlockNumber>> = Vec::new();
		for i in 0..len {
			let project = <Projects<T>>::get(i).unwrap();
			projects.push(project);
		}
		projects
	}



	
}

pub type ProjectIndex = u32;
pub type RoundIndex = u32;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type BalanceOf<T> = <<T as Config>::Currency as Currency<AccountIdOf<T>>>::Balance;
type ProjectOf<T> = Project<AccountIdOf<T>, <T as frame_system::Config>::BlockNumber>;
type ContributionOf<T> = Contribution<AccountIdOf<T>, BalanceOf<T>>;
type RoundOf<T> = Round<AccountIdOf<T>, BalanceOf<T>, <T as frame_system::Config>::BlockNumber>;
type ProposalOf<T> = Proposal<AccountIdOf<T>, BalanceOf<T>, <T as frame_system::Config>::BlockNumber>;

/// Round struct
#[derive(Encode, Decode, Default, PartialEq, Eq, Clone, Debug, TypeInfo)]
pub struct Round<AccountId, Balance, BlockNumber> {
	start: BlockNumber,
	end: BlockNumber,
	matching_fund: Balance,
	proposals: Vec<Proposal<AccountId, Balance, BlockNumber>>,
	is_canceled: bool,
	is_finalized: bool,
}

impl<AccountId, Balance: From<u32>, BlockNumber: From<u32>> Round<AccountId, Balance, BlockNumber> {
		fn new(start: BlockNumber, end: BlockNumber, matching_fund: Balance, project_indexes: Vec<ProjectIndex>) -> Round<AccountId, Balance, BlockNumber> { 
		let mut proposal_round  = Round {
			start: start,
			end: end,
			matching_fund: matching_fund,
			proposals: Vec::new(),
			is_canceled: false,
			is_finalized: false,
		};

		// Fill in the proposals structure in advance
		for project_index in project_indexes {
			proposal_round.proposals.push(Proposal {
				project_index: project_index,
				contributions: Vec::new(),
				is_approved: false,
				is_canceled: false,
				is_withdrawn: false,
				withdrawal_expiration: (0 as u32).into(),
				matching_fund: (0 as u32).into(),
			});
		}

		proposal_round
	}
}
// Proposal in round
#[derive(Encode, Decode, Default, PartialEq, Eq, Clone, Debug, TypeInfo)]
pub struct Proposal<AccountId, Balance, BlockNumber> {
	project_index: ProjectIndex,
	contributions: Vec<Contribution<AccountId, Balance>>,
	is_approved: bool,
	is_canceled: bool,
	is_withdrawn: bool,
	withdrawal_expiration: BlockNumber,
	matching_fund: Balance,
}

/// The contribution users made to a proposal project.
#[derive(Encode, Decode, Default, PartialEq, Eq, Clone, Debug, TypeInfo)]
pub struct Contribution<AccountId, Balance> {
	account_id: AccountId,
	value: Balance,
}

/// Project struct
#[derive(Encode, Decode, Default, PartialEq, Eq, Clone, Debug, TypeInfo)]
pub struct Project<AccountId, BlockNumber> {
	name: Vec<u8>,
	logo: Vec<u8>,
	description: Vec<u8>,
	website: Vec<u8>,
	/// The account that will receive the funds if the campaign is successful
	owner: AccountId,
	create_block_number: BlockNumber,
}

#[cfg(feature = "std")]
impl<T: Config> GenesisConfig<T> {
	/// Direct implementation of `GenesisBuild::build_storage`.
	///
	/// Kept in order not to break dependency.
	pub fn build_storage(&self) -> Result<sp_runtime::Storage, String> {
		<Self as GenesisBuild<T>>::build_storage(self)
	}

	/// Direct implementation of `GenesisBuild::assimilate_storage`.
	///
	/// Kept in order not to break dependency.
	pub fn assimilate_storage(
		&self,
		storage: &mut sp_runtime::Storage
	) -> Result<(), String> {
		<Self as GenesisBuild<T>>::assimilate_storage(self, storage)
	}
}