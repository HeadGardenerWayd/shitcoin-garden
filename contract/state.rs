use cosmwasm_std::{Storage, Uint128};

pub const POOL_FACTORY: &str = "POOL_FACTORY";
pub const PLATFORM_FEE_RECIPIENT: &str = "PLATFORM_FEE_RECIPIENT";

pub const CREATE_FEE_DENOM: &str = "CREATE_FEE_DENOM";
pub const CREATE_FEE: &str = "CREATE_FEE";

pub const PRESALE_DENOM: &str = "PRESALE_DENOM";
pub const PRESALE_LENGTH: &str = "PRESALE_LENGTH";
pub const PRESALE_FEE_RATE: &str = "PRESALE_FEE_RATE";
pub const PRESALE_END: &str = "PRESALE_END";
pub const PRESALE_RAISE: &str = "PRESALE_RAISE";
pub const PRESALE_SUBMISSION: &str = "PRESALE_SUBMISSION";
pub const PRESALE_CLAIMED: &str = "PRESALE_CLAIMED";

pub const SHITCOIN_COUNT: &str = "SHITCOIN_COUNT";
pub const SHITCOIN_DENOM: &str = "SHITCOIN_DENOM";
pub const SHITCOIN_CREATOR: &str = "SHITCOIN_CREATOR";
pub const SHITCOIN_TICKER: &str = "SHITCOIN_TICKER";
pub const SHITCOIN_NAME: &str = "SHITCOIN_NAME";
pub const SHITCOIN_URL: &str = "SHITCOIN_URL";
pub const SHITCOIN_SUPPLY: &str = "SHITCOIN_SUPPLY";
pub const SHITCOIN_LAUNCHED: &str = "SHITCOIN_LAUNCHED";

pub fn compose_key(parts: &[&dyn ToString]) -> String {
    let mut key: String = parts
        .iter()
        .map(|p| p.to_string())
        .map(|mut s| {
            s.push(':');
            s
        })
        .collect();

    key.pop();

    key
}

macro_rules! key {
    ($($p:ident),+) => {
        &compose_key(&[$(&$p,)+])
    };
}

pub fn set_string(storage: &mut dyn Storage, key: &str, value: &str) {
    storage.set(key.as_bytes(), value.as_bytes());
}

pub fn get_string(storage: &dyn Storage, key: &str) -> Option<String> {
    storage
        .get(key.as_bytes())
        .map(String::from_utf8)
        .transpose()
        .expect("valid utf-8")
}

pub fn set_u128(storage: &mut dyn Storage, key: &str, value: u128) {
    storage.set(key.as_bytes(), &value.to_le_bytes());
}

pub fn get_u128(storage: &dyn Storage, key: &str) -> Option<u128> {
    storage
        .get(key.as_bytes())
        .map(TryFrom::try_from)
        .transpose()
        .expect("valid little endian byte array")
        .map(u128::from_le_bytes)
}

pub fn set_u64(storage: &mut dyn Storage, key: &str, value: u64) {
    storage.set(key.as_bytes(), &value.to_le_bytes());
}

pub fn get_u64(storage: &dyn Storage, key: &str) -> Option<u64> {
    storage
        .get(key.as_bytes())
        .map(TryFrom::try_from)
        .transpose()
        .expect("valid little endian byte array")
        .map(u64::from_le_bytes)
}

pub fn set_u32(storage: &mut dyn Storage, key: &str, value: u32) {
    storage.set(key.as_bytes(), &value.to_le_bytes());
}

pub fn get_u32(storage: &dyn Storage, key: &str) -> Option<u32> {
    storage
        .get(key.as_bytes())
        .map(TryFrom::try_from)
        .transpose()
        .expect("valid little endian byte array")
        .map(u32::from_le_bytes)
}

pub fn set_bool(storage: &mut dyn Storage, key: &str, value: bool) {
    storage.set(key.as_bytes(), &[value as u8]);
}

pub fn get_bool(storage: &dyn Storage, key: &str) -> Option<bool> {
    storage.get(key.as_bytes()).map(|bytes| match bytes[..] {
        [b] => b != 0,
        _ => panic!("expected single byte"),
    })
}

pub fn set_pool_factory_address(storage: &mut dyn Storage, daddress: &str) {
    set_string(storage, POOL_FACTORY, daddress)
}

pub fn pool_factory_address(storage: &dyn Storage) -> String {
    get_string(storage, POOL_FACTORY).expect("set during init")
}

pub fn set_platform_fee_recipient(storage: &mut dyn Storage, create_fee_recipient: &str) {
    set_string(storage, PLATFORM_FEE_RECIPIENT, create_fee_recipient)
}

pub fn platform_fee_recipient(storage: &dyn Storage) -> String {
    get_string(storage, PLATFORM_FEE_RECIPIENT).expect("set during init")
}

pub fn set_create_fee_denom(storage: &mut dyn Storage, create_fee_denom: &str) {
    set_string(storage, CREATE_FEE_DENOM, create_fee_denom)
}

pub fn create_fee_denom(storage: &dyn Storage) -> String {
    get_string(storage, CREATE_FEE_DENOM).expect("set during init")
}

pub fn set_create_fee(storage: &mut dyn Storage, create_fee: Uint128) {
    set_u128(storage, CREATE_FEE, create_fee.u128())
}

pub fn create_fee(storage: &dyn Storage) -> Uint128 {
    get_u128(storage, CREATE_FEE)
        .expect("set during init")
        .into()
}

pub fn set_presale_denom(storage: &mut dyn Storage, presale_denom: &str) {
    set_string(storage, PRESALE_DENOM, presale_denom)
}

pub fn presale_denom(storage: &dyn Storage) -> String {
    get_string(storage, PRESALE_DENOM).expect("set during init")
}

pub fn set_presale_length(storage: &mut dyn Storage, presale_length: u64) {
    set_u64(storage, PRESALE_LENGTH, presale_length)
}

pub fn presale_length(storage: &dyn Storage) -> u64 {
    get_u64(storage, PRESALE_LENGTH).expect("set during init")
}

pub fn set_presale_fee_rate(storage: &mut dyn Storage, presale_fee_rate: u32) {
    set_u32(storage, PRESALE_FEE_RATE, presale_fee_rate)
}

pub fn presale_fee_rate(storage: &dyn Storage) -> u32 {
    get_u32(storage, PRESALE_FEE_RATE).expect("set during init")
}

pub fn set_presale_end(storage: &mut dyn Storage, denom: &str, presale_end: u64) {
    set_u64(storage, key![PRESALE_END, denom], presale_end)
}

pub fn presale_end(storage: &dyn Storage, denom: &str) -> Option<u64> {
    get_u64(storage, key![PRESALE_END, denom])
}

pub fn set_presale_raise(storage: &mut dyn Storage, denom: &str, presale_raise: Uint128) {
    set_u128(storage, key![PRESALE_RAISE, denom], presale_raise.u128())
}

pub fn presale_raise(storage: &dyn Storage, denom: &str) -> Option<Uint128> {
    get_u128(storage, key![PRESALE_RAISE, denom]).map(Uint128::new)
}

pub fn set_presale_submission(
    storage: &mut dyn Storage,
    denom: &str,
    degen: &str,
    presale_submission: Uint128,
) {
    set_u128(
        storage,
        key![PRESALE_SUBMISSION, denom, degen],
        presale_submission.u128(),
    )
}

pub fn presale_submission(storage: &dyn Storage, denom: &str, degen: &str) -> Option<Uint128> {
    get_u128(storage, key![PRESALE_SUBMISSION, denom, degen]).map(Uint128::new)
}

pub fn set_presale_claimed(storage: &mut dyn Storage, denom: &str, degen: &str, claimed: bool) {
    set_bool(storage, key![PRESALE_CLAIMED, denom, degen], claimed)
}

pub fn presale_claimed(storage: &dyn Storage, denom: &str, degen: &str) -> Option<bool> {
    get_bool(storage, key![PRESALE_CLAIMED, denom, degen])
}

pub fn set_shitcoin_count(storage: &mut dyn Storage, count: u64) {
    set_u64(storage, SHITCOIN_COUNT, count)
}

pub fn shitcoin_count(storage: &dyn Storage) -> u64 {
    get_u64(storage, SHITCOIN_COUNT).unwrap_or_default()
}

pub fn set_shitcoin_denom(storage: &mut dyn Storage, index: u64, denom: &str) {
    set_string(storage, key![SHITCOIN_DENOM, index], denom)
}

pub fn shitcoin_denom(storage: &dyn Storage, index: u64) -> Option<String> {
    get_string(storage, key![SHITCOIN_DENOM, index])
}

pub fn set_shitcoin_creator(storage: &mut dyn Storage, denom: &str, creator: &str) {
    set_string(storage, key![SHITCOIN_CREATOR, denom], creator)
}

pub fn shitcoin_creator(storage: &dyn Storage, denom: &str) -> Option<String> {
    get_string(storage, key![SHITCOIN_CREATOR, denom])
}

pub fn set_shitcoin_ticker(storage: &mut dyn Storage, denom: &str, ticker: &str) {
    set_string(storage, key![SHITCOIN_TICKER, denom], ticker)
}

pub fn shitcoin_ticker(storage: &dyn Storage, denom: &str) -> Option<String> {
    get_string(storage, key![SHITCOIN_TICKER, denom])
}

pub fn set_shitcoin_name(storage: &mut dyn Storage, denom: &str, name: &str) {
    set_string(storage, key![SHITCOIN_NAME, denom], name)
}

pub fn shitcoin_name(storage: &dyn Storage, denom: &str) -> Option<String> {
    get_string(storage, key![SHITCOIN_NAME, denom])
}

pub fn set_shitcoin_url(storage: &mut dyn Storage, denom: &str, url: &str) {
    set_string(storage, key![SHITCOIN_URL, denom], url)
}

pub fn shitcoin_url(storage: &dyn Storage, denom: &str) -> Option<String> {
    get_string(storage, key![SHITCOIN_URL, denom])
}

pub fn set_shitcoin_supply(storage: &mut dyn Storage, denom: &str, shitcoin_supply: Uint128) {
    set_u128(
        storage,
        key![SHITCOIN_SUPPLY, denom],
        shitcoin_supply.u128(),
    )
}

pub fn shitcoin_supply(storage: &dyn Storage, denom: &str) -> Option<Uint128> {
    get_u128(storage, key![SHITCOIN_SUPPLY, denom]).map(Uint128::new)
}

pub fn set_shitcoin_launched(storage: &mut dyn Storage, denom: &str, launched: bool) {
    set_bool(storage, key![SHITCOIN_LAUNCHED, denom], launched)
}

pub fn shitcoin_launched(storage: &dyn Storage, denom: &str) -> Option<bool> {
    get_bool(storage, key![SHITCOIN_LAUNCHED, denom])
}
