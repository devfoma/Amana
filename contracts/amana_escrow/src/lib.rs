#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol};

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Event emitted when the contract is successfully initialized.
#[contracttype]
#[derive(Clone, Debug)]
pub struct InitializedEvent {
    /// The administrator address set during initialization.
    pub admin: Address,
    /// Platform fee in basis points (e.g. 100 = 1%).
    pub fee_bps: u32,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------



// ---------------------------------------------------------------------------
// TradeStatus
// ---------------------------------------------------------------------------

/// Represents the various states a trade can be in during its lifecycle.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TradeStatus {
    /// Trade is created but not yet funded by the buyer.
    Created,
    /// Buyer has funded the escrow.
    Funded,
    /// Seller has delivered the goods or services.
    Delivered,
    /// Trade is completed and funds are released to the seller.
    Completed,
    /// A dispute has been raised by either party.
    Disputed,
    /// Trade is cancelled and funds are refunded to the buyer.
    Cancelled,
}

// ---------------------------------------------------------------------------
// Trade
// ---------------------------------------------------------------------------

/// The core data structure representing an escrow trade.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trade {
    /// Unique identifier for the trade.
    pub trade_id: u64,
    /// The buyer's address.
    pub buyer: Address,
    /// The seller's address.
    pub seller: Address,
    /// The trade amount in USDC.
    pub amount_usdc: i128,
    /// The current status of the trade.
    pub status: TradeStatus,
    /// The timestamp when the trade was created.
    pub created_at: u64,
    /// The timestamp when the trade was last updated.
    pub updated_at: u64,
}

// ---------------------------------------------------------------------------
// DataKey — namespaced storage keys
// ---------------------------------------------------------------------------

/// Keys for all storage namespaces used by this contract.
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataKey {
    /// Maps a trade ID to a Trade struct in persistent storage.
    Trade(u64),
    /// Boolean flag: whether the contract has been initialized. Stored in instance storage.
    Initialized,
    /// The administrator address set during initialization. Stored in instance storage.
    Admin,
    /// The USDC token contract address. Stored in instance storage.
    UsdcContract,
    /// Platform fee expressed in basis points (e.g. 100 = 1%). Stored in instance storage.
    FeeBps,
}

// ---------------------------------------------------------------------------
// Legacy symbol-based constants (kept for backward-compatible methods)
// ---------------------------------------------------------------------------

const SELLER: Symbol = symbol_short!("SELLER");
const BUYER: Symbol = symbol_short!("BUYER");
const AMOUNT: Symbol = symbol_short!("AMOUNT");
const LOCKED: Symbol = symbol_short!("LOCKED");

// ---------------------------------------------------------------------------
// Contract impl
// ---------------------------------------------------------------------------

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
    // -----------------------------------------------------------------------
    // Initialization
    // -----------------------------------------------------------------------

    /// Initialize the escrow contract with global platform parameters.
    ///
    /// # Arguments
    /// * `admin`          — The administrator address that owns the contract.
    /// * `usdc_contract`  — The address of the USDC token contract.
    /// * `fee_bps`        — Platform fee in basis points (e.g. 100 = 1%).
    ///
    /// # Panics
    /// Panics with `Error::AlreadyInitialized` if called more than once.
    pub fn initialize(env: Env, admin: Address, usdc_contract: Address, fee_bps: u32) {
        // Idempotency guard: reject any second call.
        if env
            .storage()
            .instance()
            .get::<DataKey, bool>(&DataKey::Initialized)
            .unwrap_or(false)
        {
            panic!("AlreadyInitialized")
        }

        // The caller must authorise itself as the deployer/admin.
        admin.require_auth();

        // Persist the global configuration.
        env.storage()
            .instance()
            .set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::UsdcContract, &usdc_contract);
        env.storage()
            .instance()
            .set(&DataKey::FeeBps, &fee_bps);

        // Mark the contract as initialized so it cannot be called again.
        env.storage()
            .instance()
            .set(&DataKey::Initialized, &true);

        // Emit an Initialized event for indexers / front-ends.
        env.events()
            .publish(("amana", "initialized"), InitializedEvent { admin, fee_bps });
    }

    // -----------------------------------------------------------------------
    // Legacy escrow methods (unchanged)
    // -----------------------------------------------------------------------

    pub fn deposit(env: Env, buyer: Address, seller: Address, amount: i128) {
        buyer.require_auth();
        env.storage().instance().set(&BUYER, &buyer);
        env.storage().instance().set(&SELLER, &seller);
        env.storage().instance().set(&AMOUNT, &amount);
        env.storage().instance().set(&LOCKED, &true);
    }

    pub fn release(env: Env, buyer: Address) {
        buyer.require_auth();
        let stored_buyer: Address = env.storage().instance().get(&BUYER).unwrap();
        assert!(buyer == stored_buyer, "only buyer can release");
        env.storage().instance().set(&LOCKED, &false);
    }

    pub fn refund(env: Env, seller: Address) {
        seller.require_auth();
        let stored_seller: Address = env.storage().instance().get(&SELLER).unwrap();
        assert!(seller == stored_seller, "only seller can refund");
        env.storage().instance().set(&LOCKED, &false);
    }

    pub fn status(env: Env) -> bool {
        env.storage().instance().get(&LOCKED).unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    // Helper: deploy a fresh contract instance.
    fn setup() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(EscrowContract, ());
        (env, contract_id)
    }

    // -----------------------------------------------------------------------
    // Core data-structure test (from previous issue)
    // -----------------------------------------------------------------------

    #[test]
    fn test_storage_structs() {
        let (env, contract_id) = setup();

        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);

        let trade = Trade {
            trade_id: 1,
            buyer: buyer.clone(),
            seller: seller.clone(),
            amount_usdc: 1000,
            status: TradeStatus::Created,
            created_at: 1234567890,
            updated_at: 1234567890,
        };

        let key = DataKey::Trade(1);

        env.as_contract(&contract_id, || {
            env.storage().persistent().set(&key, &trade);

            let read_trade: Trade = env.storage().persistent().get(&key).unwrap();

            assert_eq!(read_trade.trade_id, 1);
            assert_eq!(read_trade.buyer, buyer);
            assert_eq!(read_trade.seller, seller);
            assert_eq!(read_trade.amount_usdc, 1000);
            assert_eq!(read_trade.status, TradeStatus::Created);
            assert_eq!(read_trade.created_at, 1234567890);
            assert_eq!(read_trade.updated_at, 1234567890);
        });
    }

    // -----------------------------------------------------------------------
    // Initialization tests
    // -----------------------------------------------------------------------

    /// initialize() should succeed on the first call and persist all parameters.
    #[test]
    fn test_initialize_succeeds() {
        let (env, contract_id) = setup();

        let client = EscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let usdc = Address::generate(&env);
        let fee_bps: u32 = 100; // 1 %

        client.initialize(&admin, &usdc, &fee_bps);

        // Verify stored values via as_contract.
        env.as_contract(&contract_id, || {
            let stored_admin: Address = env
                .storage()
                .instance()
                .get(&DataKey::Admin)
                .unwrap();
            let stored_usdc: Address = env
                .storage()
                .instance()
                .get(&DataKey::UsdcContract)
                .unwrap();
            let stored_fee: u32 = env
                .storage()
                .instance()
                .get(&DataKey::FeeBps)
                .unwrap();
            let initialized: bool = env
                .storage()
                .instance()
                .get(&DataKey::Initialized)
                .unwrap();

            assert_eq!(stored_admin, admin);
            assert_eq!(stored_usdc, usdc);
            assert_eq!(stored_fee, 100);
            assert!(initialized);
        });
    }

    /// initialize() must panic when called a second time.
    #[test]
    #[should_panic]
    fn test_initialize_fails_if_called_twice() {
        let (env, contract_id) = setup();

        let client = EscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let usdc = Address::generate(&env);

        // First call — must succeed.
        client.initialize(&admin, &usdc, &100u32);

        // Second call — must panic with AlreadyInitialized.
        client.initialize(&admin, &usdc, &100u32);
    }
}
