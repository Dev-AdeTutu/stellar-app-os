#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, Address,
    BytesN, Env, Symbol,
};

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum CarbonCreditsError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAmount = 3,
    SpeciesNotFound = 4,
    InsufficientOffsets = 5,
    /// No regional baseline is registered for the project's region.
    BaselineNotSet = 6,
    /// A regional baseline must be strictly positive.
    InvalidBaseline = 7,
    /// No emissions record exists for the supplied project id.
    ProjectNotFound = 8,
    /// Emissions were already recorded for this project id (write-once).
    ProjectAlreadyRecorded = 9,
    /// Measured project emissions must be non-negative.
    InvalidEmissions = 10,
    /// The reporting period end must be strictly after its start.
    InvalidPeriod = 11,
    /// Minimum reduction threshold must be in basis points (0..=10_000).
    InvalidThreshold = 12,
    /// The project does not clear its regional baseline by the required margin.
    AdditionalityNotMet = 13,
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct SpeciesRate {
    pub slug: Symbol,
    /// kg CO₂/year × 100 (avoids floats on-chain). Example: 22 kg/yr → 2200
    pub co2_scaled: i128,
    pub maturity_years: u32,
    pub updated_at: u64,
}

/// Regional emissions baseline a project is measured against.
///
/// `baseline_co2` is expressed in the same integer unit as
/// [`ProjectEmissions::project_co2`] so the comparison is exact and on-chain
/// safe — no floating point anywhere.
#[contracttype]
#[derive(Clone, Debug)]
pub struct RegionalBaseline {
    pub region: Symbol,
    /// Baseline emissions for the region.
    pub baseline_co2: i128,
    /// Reference year the baseline was derived from.
    pub reference_year: u32,
    /// SHA-256 of the off-chain methodology document backing the baseline.
    pub methodology_digest: BytesN<32>,
    pub updated_at: u64,
}

/// Measured emissions for a single project over one reporting period.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ProjectEmissions {
    pub project_id: Symbol,
    pub region: Symbol,
    /// Measured emissions (same unit as the region's baseline).
    pub project_co2: i128,
    pub period_start: u64,
    pub period_end: u64,
    pub recorded_at: u64,
    pub recorded_by: Address,
}

/// Outcome of comparing a project's emissions to its regional baseline.
///
/// `is_additional` is the "would not have happened without the incentive"
/// verdict: the project must emit strictly less than the baseline *and* clear
/// the configured minimum reduction.
#[contracttype]
#[derive(Clone, Debug)]
pub struct AdditionalityVerdict {
    pub project_id: Symbol,
    pub region: Symbol,
    pub baseline_co2: i128,
    pub project_co2: i128,
    /// `baseline_co2 - project_co2`; negative when the project emits more.
    pub reduction_co2: i128,
    /// Minimum reduction required to qualify (basis points of the baseline).
    pub required_reduction: i128,
    pub is_additional: bool,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
enum DataKey {
    Admin,
    Rate(Symbol),
    TotalOffset(Address),
    RetiredOffset(Address),
    /// Regional baseline, keyed by region symbol.
    Baseline(Symbol),
    /// Measured project emissions, keyed by project id.
    Project(Symbol),
    /// Minimum reduction (basis points of the baseline) required to qualify.
    MinReductionBps,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct CarbonCredits;

#[contractimpl]
impl CarbonCredits {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, CarbonCreditsError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Admin-only: register or update a species sequestration rate.
    pub fn set_rate(env: Env, slug: Symbol, co2_scaled: i128, maturity_years: u32) {
        Self::require_admin(&env);

        if co2_scaled <= 0 {
            panic_with_error!(&env, CarbonCreditsError::InvalidAmount);
        }
        if maturity_years == 0 {
            panic_with_error!(&env, CarbonCreditsError::InvalidAmount);
        }

        let rate = SpeciesRate {
            slug: slug.clone(),
            co2_scaled,
            maturity_years,
            updated_at: env.ledger().timestamp(),
        };
        env.storage().persistent().set(&DataKey::Rate(slug), &rate);
    }

    pub fn get_rate(env: Env, slug: Symbol) -> SpeciesRate {
        env.storage()
            .persistent()
            .get(&DataKey::Rate(slug))
            .unwrap_or_else(|| panic_with_error!(&env, CarbonCreditsError::SpeciesNotFound))
    }

    /// Returns lifetime grams CO₂ for one tree of `slug` at `age_years`.
    /// Capped at maturity — a tree does not sequester more after it matures.
    ///
    /// Formula: min(age, maturity) × (co2_scaled × 10)
    ///   co2_scaled is kg/yr × 100; × 10 converts to grams/yr (÷100 × 1000).
    pub fn estimate_offset(env: Env, slug: Symbol, age_years: u32) -> u64 {
        let rate: SpeciesRate = env
            .storage()
            .persistent()
            .get(&DataKey::Rate(slug))
            .unwrap_or_else(|| panic_with_error!(&env, CarbonCreditsError::SpeciesNotFound));

        let capped_years = age_years.min(rate.maturity_years) as i128;
        let grams_per_year: i128 = rate.co2_scaled * 10;
        (capped_years * grams_per_year) as u64
    }

    /// Admin-only: accumulate CO₂ offset credits for a sponsor.
    pub fn record_credit(
        env: Env,
        sponsor: Address,
        slug: Symbol,
        tree_count: u32,
        age_years: u32,
    ) {
        Self::require_admin(&env);
        Self::accumulate_credit(env, sponsor, slug, tree_count, age_years);
    }

    /// Returns the total accumulated grams CO₂ offset for a sponsor.
    /// Returns 0 for an unknown sponsor.
    pub fn total_offset_for_sponsor(env: Env, sponsor: Address) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::TotalOffset(sponsor))
            .unwrap_or(0u64)
    }

    /// Sponsor-only: retire offsets by permanently deducting them from TotalOffset.
    pub fn retire_offset(env: Env, sponsor: Address, amount: u64) {
        sponsor.require_auth();

        if amount == 0 {
            panic_with_error!(&env, CarbonCreditsError::InvalidAmount);
        }

        let total_key = DataKey::TotalOffset(sponsor.clone());
        let current_total: u64 = env.storage().persistent().get(&total_key).unwrap_or(0u64);

        if amount > current_total {
            panic_with_error!(&env, CarbonCreditsError::InsufficientOffsets);
        }

        let retired_key = DataKey::RetiredOffset(sponsor.clone());
        let current_retired: u64 = env.storage().persistent().get(&retired_key).unwrap_or(0u64);

        // Update states
        env.storage()
            .persistent()
            .set(&total_key, &(current_total - amount));
        env.storage()
            .persistent()
            .set(&retired_key, &(current_retired + amount));

        env.events()
            .publish((symbol_short!("retire"), sponsor), amount);
    }

    /// Returns the total permanently retired offsets for a sponsor.
    pub fn total_retired_for_sponsor(env: Env, sponsor: Address) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::RetiredOffset(sponsor))
            .unwrap_or(0u64)
    }

    // ── Additionality verification / baseline emissions (v2) ─────────────────

    /// Admin-only: register or replace the emissions baseline for a region.
    ///
    /// `baseline_co2` must be strictly positive. `methodology_digest` is the
    /// SHA-256 of the off-chain methodology document so the baseline can be
    /// audited independently of the contract.
    pub fn set_regional_baseline(
        env: Env,
        region: Symbol,
        baseline_co2: i128,
        reference_year: u32,
        methodology_digest: BytesN<32>,
    ) {
        Self::require_admin(&env);

        if baseline_co2 <= 0 {
            panic_with_error!(&env, CarbonCreditsError::InvalidBaseline);
        }

        let baseline = RegionalBaseline {
            region: region.clone(),
            baseline_co2,
            reference_year,
            methodology_digest,
            updated_at: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&DataKey::Baseline(region.clone()), &baseline);

        env.events()
            .publish((symbol_short!("baseline"), region), baseline_co2);
    }

    /// Returns the regional baseline, or panics when none is registered.
    pub fn get_regional_baseline(env: Env, region: Symbol) -> RegionalBaseline {
        env.storage()
            .persistent()
            .get(&DataKey::Baseline(region))
            .unwrap_or_else(|| panic_with_error!(&env, CarbonCreditsError::BaselineNotSet))
    }

    /// Admin-only: minimum reduction (basis points of the regional baseline) a
    /// project must beat to count as additional. Defaults to `0`.
    pub fn set_min_reduction_bps(env: Env, bps: u32) {
        Self::require_admin(&env);

        if bps > 10_000 {
            panic_with_error!(&env, CarbonCreditsError::InvalidThreshold);
        }

        env.storage()
            .instance()
            .set(&DataKey::MinReductionBps, &bps);
        env.events()
            .publish((symbol_short!("minred"), symbol_short!("bps")), bps);
    }

    /// Returns the configured minimum reduction in basis points (`0` if unset).
    pub fn get_min_reduction_bps(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::MinReductionBps)
            .unwrap_or(0u32)
    }

    /// Admin-only: record the measured emissions for a project in a region.
    ///
    /// The regional baseline must already exist so an additionality verdict is
    /// always computable. Records are write-once per `project_id` to keep the
    /// audit trail immutable.
    pub fn record_project_emissions(
        env: Env,
        project_id: Symbol,
        region: Symbol,
        project_co2: i128,
        period_start: u64,
        period_end: u64,
    ) {
        let admin = Self::require_admin(&env);

        if project_co2 < 0 {
            panic_with_error!(&env, CarbonCreditsError::InvalidEmissions);
        }
        if period_end <= period_start {
            panic_with_error!(&env, CarbonCreditsError::InvalidPeriod);
        }
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Baseline(region.clone()))
        {
            panic_with_error!(&env, CarbonCreditsError::BaselineNotSet);
        }

        let key = DataKey::Project(project_id.clone());
        if env.storage().persistent().has(&key) {
            panic_with_error!(&env, CarbonCreditsError::ProjectAlreadyRecorded);
        }

        let record = ProjectEmissions {
            project_id: project_id.clone(),
            region: region.clone(),
            project_co2,
            period_start,
            period_end,
            recorded_at: env.ledger().timestamp(),
            recorded_by: admin,
        };
        env.storage().persistent().set(&key, &record);

        env.events()
            .publish((symbol_short!("project"), project_id), project_co2);
    }

    /// Returns the recorded emissions for a project, or panics when unknown.
    pub fn get_project_emissions(env: Env, project_id: Symbol) -> ProjectEmissions {
        env.storage()
            .persistent()
            .get(&DataKey::Project(project_id))
            .unwrap_or_else(|| panic_with_error!(&env, CarbonCreditsError::ProjectNotFound))
    }

    /// Compare a project's measured emissions against its regional baseline.
    ///
    /// A project is additional when it emits strictly less than the baseline
    /// **and** the reduction clears the configured minimum (basis points of the
    /// baseline). Read-only and safe to call from any client.
    pub fn evaluate_additionality(env: Env, project_id: Symbol) -> AdditionalityVerdict {
        let project: ProjectEmissions = env
            .storage()
            .persistent()
            .get(&DataKey::Project(project_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, CarbonCreditsError::ProjectNotFound));

        let baseline: RegionalBaseline = env
            .storage()
            .persistent()
            .get(&DataKey::Baseline(project.region.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, CarbonCreditsError::BaselineNotSet));

        let bps: u32 = env
            .storage()
            .instance()
            .get(&DataKey::MinReductionBps)
            .unwrap_or(0u32);

        let reduction_co2 = baseline.baseline_co2 - project.project_co2;
        let required_reduction = baseline.baseline_co2 * bps as i128 / 10_000;

        AdditionalityVerdict {
            project_id,
            region: project.region,
            baseline_co2: baseline.baseline_co2,
            project_co2: project.project_co2,
            reduction_co2,
            required_reduction,
            is_additional: reduction_co2 > 0 && reduction_co2 >= required_reduction,
        }
    }

    /// Admin-only: issue offset credits only after additionality passes.
    ///
    /// Identical to [`Self::record_credit`] except it refuses to mint when
    /// [`Self::evaluate_additionality`] does not return an additional verdict,
    /// so business-as-usual emissions never earn credits.
    pub fn record_verified_credit(
        env: Env,
        sponsor: Address,
        project_id: Symbol,
        slug: Symbol,
        tree_count: u32,
        age_years: u32,
    ) {
        Self::require_admin(&env);

        let verdict = Self::evaluate_additionality(env.clone(), project_id);
        if !verdict.is_additional {
            panic_with_error!(&env, CarbonCreditsError::AdditionalityNotMet);
        }

        Self::accumulate_credit(env, sponsor, slug, tree_count, age_years);
    }

    // ── internal ──────────────────────────────────────────────────────────────

    /// Require the stored admin's authorization and return the admin address.
    fn require_admin(env: &Env) -> Address {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, CarbonCreditsError::NotInitialized));
        admin.require_auth();
        admin
    }

    /// Shared credit accumulation used by both `record_credit` and
    /// `record_verified_credit` so the accounting logic lives in one place.
    fn accumulate_credit(
        env: Env,
        sponsor: Address,
        slug: Symbol,
        tree_count: u32,
        age_years: u32,
    ) {
        if tree_count == 0 {
            panic_with_error!(&env, CarbonCreditsError::InvalidAmount);
        }

        let per_tree = Self::estimate_offset(env.clone(), slug, age_years);
        let delta = per_tree * tree_count as u64;

        let key = DataKey::TotalOffset(sponsor.clone());
        let current: u64 = env.storage().persistent().get(&key).unwrap_or(0u64);
        env.storage().persistent().set(&key, &(current + delta));

        env.events().publish(
            (symbol_short!("credit"), symbol_short!("recorded")),
            (sponsor, delta),
        );
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env, Symbol};

    fn setup() -> (Env, Address, CarbonCreditsClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, CarbonCredits);
        let client = CarbonCreditsClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, admin, client)
    }

    fn digest(env: &Env, seed: u8) -> BytesN<32> {
        BytesN::from_array(env, &[seed; 32])
    }

    fn region(env: &Env, name: &str) -> Symbol {
        Symbol::new(env, name)
    }

    #[test]
    fn test_initialize() {
        let (_, _, _client) = setup();
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #1)")]
    fn test_double_init_panics() {
        let (_, admin, client) = setup();
        client.initialize(&admin);
    }

    #[test]
    fn test_set_and_get_rate() {
        let (env, _, client) = setup();

        let slug = Symbol::new(&env, "teak");
        client.set_rate(&slug, &2200_i128, &20_u32);

        let rate = client.get_rate(&slug);
        assert_eq!(rate.co2_scaled, 2200);
        assert_eq!(rate.maturity_years, 20);
    }

    #[test]
    fn test_estimate_offset_under_maturity() {
        let (env, _, client) = setup();

        // teak: 22 kg/yr → co2_scaled=2200, maturity=20yr
        let slug = Symbol::new(&env, "teak");
        client.set_rate(&slug, &2200_i128, &20_u32);

        // age=10yr < maturity=20yr → 10 × 2200 × 10 = 220_000 g
        let result = client.estimate_offset(&slug, &10_u32);
        assert_eq!(result, 220_000u64);
    }

    #[test]
    fn test_estimate_offset_at_maturity() {
        let (env, _, client) = setup();

        let slug = Symbol::new(&env, "teak");
        client.set_rate(&slug, &2200_i128, &20_u32);

        // age=20yr = maturity=20yr → 20 × 2200 × 10 = 440_000 g
        let result = client.estimate_offset(&slug, &20_u32);
        assert_eq!(result, 440_000u64);
    }

    #[test]
    fn test_estimate_offset_over_maturity() {
        let (env, _, client) = setup();

        let slug = Symbol::new(&env, "teak");
        client.set_rate(&slug, &2200_i128, &20_u32);

        // age=50yr > maturity=20yr → capped at 20 → 440_000 g (same as at_maturity)
        let result = client.estimate_offset(&slug, &50_u32);
        assert_eq!(result, 440_000u64);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #4)")]
    fn test_estimate_unknown_slug_panics() {
        let (env, _, client) = setup();
        client.estimate_offset(&Symbol::new(&env, "unknown"), &5_u32);
    }

    #[test]
    fn test_record_credit_accumulates() {
        let (env, _, client) = setup();

        let slug = Symbol::new(&env, "teak");
        client.set_rate(&slug, &2200_i128, &20_u32);

        let sponsor = Address::generate(&env);

        // First call: 5 trees × 10yr → 5 × 220_000 = 1_100_000 g
        client.record_credit(&sponsor, &slug, &5_u32, &10_u32);
        assert_eq!(client.total_offset_for_sponsor(&sponsor), 1_100_000u64);

        // Second call: 3 trees × 10yr → 3 × 220_000 = 660_000 g
        client.record_credit(&sponsor, &slug, &3_u32, &10_u32);
        assert_eq!(client.total_offset_for_sponsor(&sponsor), 1_760_000u64);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #3)")]
    fn test_record_credit_zero_tree_count_panics() {
        let (env, _, client) = setup();

        let slug = Symbol::new(&env, "teak");
        client.set_rate(&slug, &2200_i128, &20_u32);

        let sponsor = Address::generate(&env);
        client.record_credit(&sponsor, &slug, &0_u32, &10_u32);
    }

    #[test]
    fn test_total_offset_unknown_sponsor_zero() {
        let (env, _, client) = setup();

        let sponsor = Address::generate(&env);
        assert_eq!(client.total_offset_for_sponsor(&sponsor), 0u64);
        assert_eq!(client.total_retired_for_sponsor(&sponsor), 0u64);
    }

    #[test]
    fn test_retire_offset_success() {
        let (env, _, client) = setup();

        let slug = Symbol::new(&env, "teak");
        client.set_rate(&slug, &2200_i128, &20_u32);

        let sponsor = Address::generate(&env);
        client.record_credit(&sponsor, &slug, &5_u32, &10_u32); // 1_100_000 g

        assert_eq!(client.total_offset_for_sponsor(&sponsor), 1_100_000u64);
        assert_eq!(client.total_retired_for_sponsor(&sponsor), 0u64);

        // Retire 100_000 g
        client.retire_offset(&sponsor, &100_000u64);

        assert_eq!(client.total_offset_for_sponsor(&sponsor), 1_000_000u64);
        assert_eq!(client.total_retired_for_sponsor(&sponsor), 100_000u64);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #5)")]
    fn test_retire_offset_insufficient_balance() {
        let (env, _, client) = setup();

        let slug = Symbol::new(&env, "teak");
        client.set_rate(&slug, &2200_i128, &20_u32);

        let sponsor = Address::generate(&env);
        client.record_credit(&sponsor, &slug, &1_u32, &10_u32); // 220_000 g

        // Try to retire more than balance
        client.retire_offset(&sponsor, &220_001u64);
    }

    // ── Additionality verification (v2) ──────────────────────────────────────

    #[test]
    fn test_set_and_get_regional_baseline() {
        let (env, _, client) = setup();

        let r = region(&env, "north");
        client.set_regional_baseline(&r, &1_000_000_i128, &2024_u32, &digest(&env, 1));

        let baseline = client.get_regional_baseline(&r);
        assert_eq!(baseline.region, r);
        assert_eq!(baseline.baseline_co2, 1_000_000);
        assert_eq!(baseline.reference_year, 2024);
    }

    #[test]
    fn test_regional_baseline_can_be_replaced() {
        let (env, _, client) = setup();

        let r = region(&env, "north");
        client.set_regional_baseline(&r, &1_000_000_i128, &2024_u32, &digest(&env, 1));
        client.set_regional_baseline(&r, &900_000_i128, &2025_u32, &digest(&env, 2));

        assert_eq!(client.get_regional_baseline(&r).baseline_co2, 900_000);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #7)")]
    fn test_baseline_must_be_positive() {
        let (env, _, client) = setup();
        client.set_regional_baseline(&region(&env, "north"), &0_i128, &2024_u32, &digest(&env, 1));
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #6)")]
    fn test_get_missing_baseline_panics() {
        let (env, _, client) = setup();
        client.get_regional_baseline(&region(&env, "nowhere"));
    }

    #[test]
    fn test_default_min_reduction_is_zero() {
        let (_, _, client) = setup();
        assert_eq!(client.get_min_reduction_bps(), 0u32);
    }

    #[test]
    fn test_set_min_reduction_bps() {
        let (_, _, client) = setup();
        client.set_min_reduction_bps(&2500_u32);
        assert_eq!(client.get_min_reduction_bps(), 2500u32);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #12)")]
    fn test_invalid_threshold_rejected() {
        let (_, _, client) = setup();
        client.set_min_reduction_bps(&10_001_u32);
    }

    #[test]
    fn test_record_and_get_project_emissions() {
        let (env, _, client) = setup();

        let r = region(&env, "north");
        client.set_regional_baseline(&r, &1_000_000_i128, &2024_u32, &digest(&env, 1));

        let p = region(&env, "proj1");
        client.record_project_emissions(
            &p,
            &r,
            &800_000_i128,
            &1_700_000_000_u64,
            &1_702_592_000_u64,
        );

        let record = client.get_project_emissions(&p);
        assert_eq!(record.project_id, p);
        assert_eq!(record.region, r);
        assert_eq!(record.project_co2, 800_000);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #6)")]
    fn test_record_project_emissions_requires_baseline() {
        let (env, _, client) = setup();
        client.record_project_emissions(
            &region(&env, "proj1"),
            &region(&env, "nowhere"),
            &800_000_i128,
            &1_700_000_000_u64,
            &1_702_592_000_u64,
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #10)")]
    fn test_record_project_emissions_rejects_negative() {
        let (env, _, client) = setup();

        let r = region(&env, "north");
        client.set_regional_baseline(&r, &1_000_000_i128, &2024_u32, &digest(&env, 1));

        client.record_project_emissions(
            &region(&env, "proj1"),
            &r,
            &-1_i128,
            &1_700_000_000_u64,
            &1_702_592_000_u64,
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #11)")]
    fn test_record_project_emissions_rejects_bad_period() {
        let (env, _, client) = setup();

        let r = region(&env, "north");
        client.set_regional_baseline(&r, &1_000_000_i128, &2024_u32, &digest(&env, 1));

        client.record_project_emissions(
            &region(&env, "proj1"),
            &r,
            &800_000_i128,
            &1_702_592_000_u64,
            &1_700_000_000_u64,
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #9)")]
    fn test_project_emissions_are_write_once() {
        let (env, _, client) = setup();

        let r = region(&env, "north");
        client.set_regional_baseline(&r, &1_000_000_i128, &2024_u32, &digest(&env, 1));

        let p = region(&env, "proj1");
        client.record_project_emissions(
            &p,
            &r,
            &800_000_i128,
            &1_700_000_000_u64,
            &1_702_592_000_u64,
        );
        client.record_project_emissions(
            &p,
            &r,
            &700_000_i128,
            &1_700_000_000_u64,
            &1_702_592_000_u64,
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #8)")]
    fn test_get_missing_project_panics() {
        let (env, _, client) = setup();
        client.get_project_emissions(&region(&env, "proj1"));
    }

    #[test]
    fn test_additionality_true_when_below_baseline() {
        let (env, _, client) = setup();

        let r = region(&env, "north");
        client.set_regional_baseline(&r, &1_000_000_i128, &2024_u32, &digest(&env, 1));

        let p = region(&env, "proj1");
        client.record_project_emissions(
            &p,
            &r,
            &800_000_i128,
            &1_700_000_000_u64,
            &1_702_592_000_u64,
        );

        let verdict = client.evaluate_additionality(&p);
        assert!(verdict.is_additional);
        assert_eq!(verdict.reduction_co2, 200_000);
        assert_eq!(verdict.required_reduction, 0);
    }

    #[test]
    fn test_additionality_false_when_equal_to_baseline() {
        let (env, _, client) = setup();

        let r = region(&env, "north");
        client.set_regional_baseline(&r, &1_000_000_i128, &2024_u32, &digest(&env, 1));

        let p = region(&env, "proj1");
        client.record_project_emissions(
            &p,
            &r,
            &1_000_000_i128,
            &1_700_000_000_u64,
            &1_702_592_000_u64,
        );

        let verdict = client.evaluate_additionality(&p);
        assert!(!verdict.is_additional);
        assert_eq!(verdict.reduction_co2, 0);
    }

    #[test]
    fn test_additionality_false_when_above_baseline() {
        let (env, _, client) = setup();

        let r = region(&env, "north");
        client.set_regional_baseline(&r, &1_000_000_i128, &2024_u32, &digest(&env, 1));

        let p = region(&env, "proj1");
        client.record_project_emissions(
            &p,
            &r,
            &1_400_000_i128,
            &1_700_000_000_u64,
            &1_702_592_000_u64,
        );

        let verdict = client.evaluate_additionality(&p);
        assert!(!verdict.is_additional);
        assert_eq!(verdict.reduction_co2, -400_000);
    }

    #[test]
    fn test_threshold_blocks_borderline_reduction() {
        let (env, _, client) = setup();
        client.set_min_reduction_bps(&3000_u32); // 30%

        let r = region(&env, "north");
        client.set_regional_baseline(&r, &1_000_000_i128, &2024_u32, &digest(&env, 1));

        let p = region(&env, "proj1");
        client.record_project_emissions(
            &p,
            &r,
            &800_000_i128,
            &1_700_000_000_u64,
            &1_702_592_000_u64,
        );

        let verdict = client.evaluate_additionality(&p);
        assert!(!verdict.is_additional);
        assert_eq!(verdict.reduction_co2, 200_000);
        assert_eq!(verdict.required_reduction, 300_000);
    }

    #[test]
    fn test_threshold_met_marks_additional() {
        let (env, _, client) = setup();
        client.set_min_reduction_bps(&1000_u32); // 10%

        let r = region(&env, "north");
        client.set_regional_baseline(&r, &1_000_000_i128, &2024_u32, &digest(&env, 1));

        let p = region(&env, "proj1");
        client.record_project_emissions(
            &p,
            &r,
            &850_000_i128,
            &1_700_000_000_u64,
            &1_702_592_000_u64,
        );

        let verdict = client.evaluate_additionality(&p);
        assert!(verdict.is_additional);
        assert_eq!(verdict.required_reduction, 100_000);
    }

    #[test]
    fn test_record_verified_credit_issues_when_additional() {
        let (env, _, client) = setup();

        let r = region(&env, "north");
        client.set_regional_baseline(&r, &1_000_000_i128, &2024_u32, &digest(&env, 1));

        let slug = Symbol::new(&env, "teak");
        client.set_rate(&slug, &2200_i128, &20_u32);

        let p = region(&env, "proj1");
        client.record_project_emissions(
            &p,
            &r,
            &800_000_i128,
            &1_700_000_000_u64,
            &1_702_592_000_u64,
        );

        let sponsor = Address::generate(&env);
        client.record_verified_credit(&sponsor, &p, &slug, &5_u32, &10_u32);

        // Same accounting as record_credit: 5 × 220_000 g
        assert_eq!(client.total_offset_for_sponsor(&sponsor), 1_100_000u64);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #13)")]
    fn test_record_verified_credit_rejected_when_not_additional() {
        let (env, _, client) = setup();

        let r = region(&env, "north");
        client.set_regional_baseline(&r, &1_000_000_i128, &2024_u32, &digest(&env, 1));

        let slug = Symbol::new(&env, "teak");
        client.set_rate(&slug, &2200_i128, &20_u32);

        let p = region(&env, "proj1");
        // Emits exactly the regional baseline → business as usual.
        client.record_project_emissions(
            &p,
            &r,
            &1_000_000_i128,
            &1_700_000_000_u64,
            &1_702_592_000_u64,
        );

        let sponsor = Address::generate(&env);
        client.record_verified_credit(&sponsor, &p, &slug, &5_u32, &10_u32);
    }
}
