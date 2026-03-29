#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
};

#[contracttype]
pub enum DataKey {
    Admin,
    Initialized,
    TokenName,
    TokenSymbol,
    Balance(Address),
    TotalSupply,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MealCreditError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAmount = 3,
    InsufficientBalance = 4,
}

#[contract]
pub struct CanteenCreditContract;

fn require_initialized(env: &Env) -> Result<(), MealCreditError> {
    if env.storage().instance().has(&DataKey::Initialized) {
        Ok(())
    } else {
        Err(MealCreditError::NotInitialized)
    }
}

fn require_positive(amount: i128) -> Result<(), MealCreditError> {
    if amount > 0 {
        Ok(())
    } else {
        Err(MealCreditError::InvalidAmount)
    }
}

fn read_admin(env: &Env) -> Result<Address, MealCreditError> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(MealCreditError::NotInitialized)
}

fn read_balance(env: &Env, student: Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::Balance(student))
        .unwrap_or(0)
}

fn read_total_supply(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::TotalSupply)
        .unwrap_or(0)
}

#[contractimpl]
impl CanteenCreditContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        token_name: String,
        token_symbol: String,
    ) -> Result<(), MealCreditError> {
        if env.storage().instance().has(&DataKey::Initialized) {
            return Err(MealCreditError::AlreadyInitialized);
        }

        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::TokenName, &token_name);
        env.storage()
            .instance()
            .set(&DataKey::TokenSymbol, &token_symbol);
        env.storage().instance().set(&DataKey::TotalSupply, &0_i128);
        env.storage().instance().set(&DataKey::Initialized, &true);

        env.events()
            .publish((symbol_short!("init"), admin), token_symbol);
        Ok(())
    }

    pub fn top_up(env: Env, student: Address, amount: i128) -> Result<(), MealCreditError> {
        require_initialized(&env)?;
        require_positive(amount)?;

        let admin = read_admin(&env)?;
        admin.require_auth();

        let balance = read_balance(&env, student.clone());
        let next_balance = balance + amount;
        let next_supply = read_total_supply(&env) + amount;

        env.storage()
            .persistent()
            .set(&DataKey::Balance(student.clone()), &next_balance);
        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &next_supply);

        env.events()
            .publish((symbol_short!("topup"), student), amount);
        Ok(())
    }

    pub fn spend(env: Env, student: Address, amount: i128) -> Result<(), MealCreditError> {
        require_initialized(&env)?;
        require_positive(amount)?;
        student.require_auth();

        let balance = read_balance(&env, student.clone());
        if balance < amount {
            return Err(MealCreditError::InsufficientBalance);
        }

        let next_balance = balance - amount;
        let next_supply = read_total_supply(&env) - amount;

        env.storage()
            .persistent()
            .set(&DataKey::Balance(student.clone()), &next_balance);
        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &next_supply);

        env.events()
            .publish((symbol_short!("spend"), student), amount);
        Ok(())
    }

    pub fn transfer(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), MealCreditError> {
        require_initialized(&env)?;
        require_positive(amount)?;
        from.require_auth();

        let from_balance = read_balance(&env, from.clone());
        if from_balance < amount {
            return Err(MealCreditError::InsufficientBalance);
        }

        let to_balance = read_balance(&env, to.clone());

        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &(from_balance - amount));
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &(to_balance + amount));

        env.events()
            .publish((symbol_short!("transfer"), from, to), amount);
        Ok(())
    }

    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), MealCreditError> {
        require_initialized(&env)?;

        let admin = read_admin(&env)?;
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.events().publish((symbol_short!("admin"),), new_admin);
        Ok(())
    }

    pub fn balance(env: Env, student: Address) -> i128 {
        read_balance(&env, student)
    }

    pub fn total_supply(env: Env) -> i128 {
        read_total_supply(&env)
    }

    pub fn admin(env: Env) -> Address {
        read_admin(&env).unwrap()
    }

    pub fn token_name(env: Env) -> String {
        require_initialized(&env).unwrap();
        env.storage().instance().get(&DataKey::TokenName).unwrap()
    }

    pub fn token_symbol(env: Env) -> String {
        require_initialized(&env).unwrap();
        env.storage().instance().get(&DataKey::TokenSymbol).unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    fn setup() -> (Env, CanteenCreditContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CanteenCreditContract, ());
        let client = CanteenCreditContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client.initialize(
            &admin,
            &String::from_str(&env, "School Meal Credit"),
            &String::from_str(&env, "MEAL"),
        );

        (env, client, admin)
    }

    #[test]
    fn initialize_sets_metadata() {
        let (env, client, admin) = setup();

        assert_eq!(client.admin(), admin);
        assert_eq!(
            client.token_name(),
            String::from_str(&env, "School Meal Credit")
        );
        assert_eq!(client.token_symbol(), String::from_str(&env, "MEAL"));
        assert_eq!(client.total_supply(), 0);
    }

    #[test]
    fn admin_can_top_up_student() {
        let (env, client, _admin) = setup();
        let student = Address::generate(&env);

        client.top_up(&student, &500);

        assert_eq!(client.balance(&student), 500);
        assert_eq!(client.total_supply(), 500);
    }

    #[test]
    fn student_can_spend_credit() {
        let (env, client, _admin) = setup();
        let student = Address::generate(&env);

        client.top_up(&student, &500);
        client.spend(&student, &200);

        assert_eq!(client.balance(&student), 300);
        assert_eq!(client.total_supply(), 300);
    }

    #[test]
    fn student_can_transfer_credit() {
        let (env, client, _admin) = setup();
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        client.top_up(&alice, &700);
        client.transfer(&alice, &bob, &250);

        assert_eq!(client.balance(&alice), 450);
        assert_eq!(client.balance(&bob), 250);
        assert_eq!(client.total_supply(), 700);
    }

    #[test]
    fn spend_fails_when_balance_is_too_low() {
        let (env, client, _admin) = setup();
        let student = Address::generate(&env);

        client.top_up(&student, &100);

        assert!(client.try_spend(&student, &200).is_err());
    }
}
