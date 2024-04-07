use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

#[cw_serde]
pub struct InstantiateMsg {
    pub pool_factory_address: String,
    pub fee_recipient: String,
    pub create_fee_denom: String,
    pub create_fee: Uint128, // fixed
    pub presale_denom: String,
    pub presale_length: u64,
    pub presale_fee_rate: u32, // bps
}

#[cw_serde]
pub enum ExecuteMsg {
    CreateShitcoin {
        ticker: String,
        name: String,
        supply: Uint128,
    },
    EnterPresale {
        denom: String,
    },
    ExtendPresale {
        denom: String,
    },
    LaunchShitcoin {
        denom: String,
    },
    ClaimShitcoin {
        denom: String,
    },
    SetUrl {
        denom: String,
        url: String,
    },
}

#[cw_serde]
pub struct Config {
    pub pool_factory_address: String,
    pub fee_recipient: String,
    pub create_fee_denom: String,
    pub create_fee: Uint128,
    pub presale_denom: String,
    pub presale_length: u64,
    pub presale_fee_rate: u32,
}

#[cw_serde]
pub struct ShitcoinMetadata {
    pub denom: String,
    pub creator: String,
    pub ticker: String,
    pub name: String,
    pub url: String,
    pub presale_end: u64,
    pub presale_raise: Uint128,
    pub supply: Uint128,
    pub ended: bool,
    pub launched: bool,
}

#[cw_serde]
pub struct DegenMetadata {
    pub presale_submission: Uint128,
    pub shitcoins_claimed: bool,
}

#[cw_serde]
pub struct ShitcoinPage {
    pub page: u64,
    pub limit: u64,
    pub total: u64,
    pub shitcoins: Vec<ShitcoinMetadata>,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Config)]
    Config {},
    #[returns(ShitcoinMetadata)]
    ShitcoinMetadata { denom: String },
    #[returns(ShitcoinPage)]
    Shitcoins {
        page: Option<u64>,
        limit: Option<u64>,
    },
    #[returns(DegenMetadata)]
    DegenMetadata { denom: String, degen: String },
}
