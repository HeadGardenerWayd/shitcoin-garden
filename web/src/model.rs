use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use cosmos_sdk_proto::cosmwasm::wasm::v1::QueryRawContractStateRequest;
use cosmos_sdk_proto::{
    cosmos::base::query::v1beta1::PageRequest,
    cosmwasm::wasm::v1::{Model, QueryAllContractStateRequest},
};
use futures::future::{try_join, try_join3, try_join5};

use crate::{CwClient, SHITCOIN_GARDEN_CONTRACT};

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ShitcoinMeta {
    pub creator: String,
    pub ticker: String,
    pub name: String,
    pub url: String,
    pub presale_end: u64,
    pub presale_raise: u128,
    pub supply: u128,
    pub launched: bool,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct DegenMeta {
    pub submission: u128,
    pub claimed: bool,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ShitcoinGardenState {
    pub indexes: BTreeMap<u64, String>,
    pub shitcoins: HashMap<String, ShitcoinMeta>,
    pub degens: HashMap<(String, String), DegenMeta>,
}

const PRESALE_END: &[u8] = b"PRESALE_END";
const PRESALE_RAISE: &[u8] = b"PRESALE_RAISE";
const PRESALE_SUBMISSION: &[u8] = b"PRESALE_SUBMISSION";
const PRESALE_CLAIMED: &[u8] = b"PRESALE_CLAIMED";
const SHITCOIN_CREATOR: &[u8] = b"SHITCOIN_CREATOR";
const SHITCOIN_TICKER: &[u8] = b"SHITCOIN_TICKER";
const SHITCOIN_NAME: &[u8] = b"SHITCOIN_NAME";
const SHITCOIN_URL: &[u8] = b"SHITCOIN_URL";
const SHITCOIN_SUPPLY: &[u8] = b"SHITCOIN_SUPPLY";
const SHITCOIN_LAUNCHED: &[u8] = b"SHITCOIN_LAUNCHED";
const SHITCOIN_DENOM: &[u8] = b"SHITCOIN_DENOM";

fn add_model_to_state(model: Model, state: &mut ShitcoinGardenState) {
    let mut parts = model.key.split(|b| *b == b':');

    let Some(prefix) = parts.next() else {
        return;
    };

    match prefix {
        PRESALE_END => {
            let denom_bytes = parts.next().unwrap();

            let denom = std::str::from_utf8(denom_bytes).unwrap();

            state
                .shitcoins
                .entry(denom.to_owned())
                .or_default()
                .presale_end = u64::from_le_bytes(model.value.try_into().unwrap());
        }

        PRESALE_RAISE => {
            let denom_bytes = parts.next().unwrap();

            let denom = std::str::from_utf8(denom_bytes).unwrap();

            state
                .shitcoins
                .entry(denom.to_owned())
                .or_default()
                .presale_raise = u128::from_le_bytes(model.value.try_into().unwrap());
        }

        PRESALE_SUBMISSION => {
            let denom_bytes = parts.next().unwrap();

            let denom = std::str::from_utf8(denom_bytes).unwrap();

            let degen_bytes = parts.next().unwrap();

            let degen = std::str::from_utf8(degen_bytes).unwrap();

            state
                .degens
                .entry((denom.to_owned(), degen.to_owned()))
                .or_default()
                .submission = u128::from_le_bytes(model.value.try_into().unwrap());
        }

        PRESALE_CLAIMED => {
            let denom_bytes = parts.next().unwrap();

            let denom = std::str::from_utf8(denom_bytes).unwrap();

            let degen_bytes = parts.next().unwrap();

            let degen = std::str::from_utf8(degen_bytes).unwrap();

            state
                .degens
                .entry((denom.to_owned(), degen.to_owned()))
                .or_default()
                .claimed = matches!(model.value.as_slice(), &[1]);
        }

        SHITCOIN_CREATOR => {
            let denom_bytes = parts.next().unwrap();

            let denom = std::str::from_utf8(denom_bytes).unwrap();

            state.shitcoins.entry(denom.to_owned()).or_default().creator =
                String::from_utf8(model.value).unwrap();
        }

        SHITCOIN_TICKER => {
            let denom_bytes = parts.next().unwrap();

            let denom = std::str::from_utf8(denom_bytes).unwrap();

            state.shitcoins.entry(denom.to_owned()).or_default().ticker =
                String::from_utf8(model.value).unwrap();
        }

        SHITCOIN_NAME => {
            let denom_bytes = parts.next().unwrap();

            let denom = std::str::from_utf8(denom_bytes).unwrap();

            state.shitcoins.entry(denom.to_owned()).or_default().name =
                String::from_utf8(model.value).unwrap();
        }

        SHITCOIN_URL => {
            let denom_bytes = parts.next().unwrap();

            let denom = std::str::from_utf8(denom_bytes).unwrap();

            state.shitcoins.entry(denom.to_owned()).or_default().url =
                String::from_utf8(model.value).unwrap();
        }

        SHITCOIN_SUPPLY => {
            let denom_bytes = parts.next().unwrap();

            let denom = std::str::from_utf8(denom_bytes).unwrap();

            state.shitcoins.entry(denom.to_owned()).or_default().supply =
                u128::from_le_bytes(model.value.try_into().unwrap());
        }

        SHITCOIN_LAUNCHED => {
            let denom_bytes = parts.next().unwrap();

            let denom = std::str::from_utf8(denom_bytes).unwrap();

            state
                .shitcoins
                .entry(denom.to_owned())
                .or_default()
                .launched = matches!(model.value.as_slice(), &[1]);
        }

        SHITCOIN_DENOM => {
            let index_str_bytes = parts.next().unwrap();

            let idx: u64 = std::str::from_utf8(index_str_bytes)
                .unwrap()
                .parse()
                .unwrap();

            let denom = String::from_utf8(model.value).unwrap();

            *state.indexes.entry(idx).or_default() = denom;
        }
        _ => {}
    }
}

fn decode_contract_state(models: Vec<Model>) -> ShitcoinGardenState {
    let mut state = ShitcoinGardenState::default();

    for model in models {
        add_model_to_state(model, &mut state);
    }

    state
}

pub async fn query_entire_contract_state(cw: &mut CwClient) -> Result<ShitcoinGardenState> {
    let initial_response = cw
        .all_contract_state(QueryAllContractStateRequest {
            address: crate::SHITCOIN_GARDEN_CONTRACT.to_owned(),
            pagination: None,
        })
        .await?
        .into_inner();

    let mut models = initial_response.models;
    let mut pagination = initial_response.pagination;

    while let Some(page) = pagination.take() {
        if page.next_key.is_empty() {
            break;
        }

        let response = cw
            .all_contract_state(QueryAllContractStateRequest {
                address: SHITCOIN_GARDEN_CONTRACT.to_owned(),
                pagination: Some(PageRequest {
                    key: page.next_key,
                    ..Default::default()
                }),
            })
            .await?
            .into_inner();

        models.extend(response.models);
        pagination = response.pagination;
    }

    let state = decode_contract_state(models);

    Ok(state)
}

fn raw_shitcoin_meta_key(prefix: &[u8], denom: &str) -> Vec<u8> {
    [prefix, &[b':'], denom.as_bytes()].concat()
}

fn raw_shitcoin_meta_request(prefix: &[u8], denom: &str) -> QueryRawContractStateRequest {
    let query_data = raw_shitcoin_meta_key(prefix, denom);

    QueryRawContractStateRequest {
        address: SHITCOIN_GARDEN_CONTRACT.to_owned(),
        query_data,
    }
}

fn raw_degen_meta_key(prefix: &[u8], denom: &str, degen: &str) -> Vec<u8> {
    [prefix, &[b':'], denom.as_bytes(), &[b':'], degen.as_bytes()].concat()
}

fn raw_degen_meta_request(prefix: &[u8], denom: &str, degen: &str) -> QueryRawContractStateRequest {
    let query_data = raw_degen_meta_key(prefix, denom, degen);

    QueryRawContractStateRequest {
        address: SHITCOIN_GARDEN_CONTRACT.to_owned(),
        query_data,
    }
}

async fn query_shitcoin_meta_raw(mut cw: CwClient, prefix: &[u8], denom: &str) -> Result<Vec<u8>> {
    let response = cw
        .raw_contract_state(raw_shitcoin_meta_request(prefix, denom))
        .await?;

    Ok(response.into_inner().data)
}

async fn query_degen_meta_raw(
    mut cw: CwClient,
    prefix: &[u8],
    denom: &str,
    degen: &str,
) -> Result<Vec<u8>> {
    let response = cw
        .raw_contract_state(raw_degen_meta_request(prefix, denom, degen))
        .await?;

    Ok(response.into_inner().data)
}

pub async fn query_shitcoin_creator(cw: CwClient, denom: &str) -> Result<String> {
    let raw = query_shitcoin_meta_raw(cw, SHITCOIN_CREATOR, denom).await?;
    let creator = String::from_utf8(raw)?;

    Ok(creator)
}

pub async fn query_shitcoin_ticker(cw: CwClient, denom: &str) -> Result<String> {
    let raw = query_shitcoin_meta_raw(cw, SHITCOIN_TICKER, denom).await?;
    let ticker = String::from_utf8(raw)?;

    Ok(ticker)
}

pub async fn query_shitcoin_name(cw: CwClient, denom: &str) -> Result<String> {
    let raw = query_shitcoin_meta_raw(cw, SHITCOIN_NAME, denom).await?;
    let name = String::from_utf8(raw)?;

    Ok(name)
}

pub async fn query_shitcoin_url(cw: CwClient, denom: &str) -> Result<String> {
    let raw = query_shitcoin_meta_raw(cw, SHITCOIN_URL, denom).await?;
    let url = String::from_utf8(raw)?;

    Ok(url)
}

pub async fn query_shitcoin_presale_end(cw: CwClient, denom: &str) -> Result<u64> {
    let raw = query_shitcoin_meta_raw(cw, PRESALE_END, denom).await?;
    let le_bytes_arr = raw.try_into().unwrap();

    Ok(u64::from_le_bytes(le_bytes_arr))
}

pub async fn query_shitcoin_presale_raise(cw: CwClient, denom: &str) -> Result<u128> {
    let raw = query_shitcoin_meta_raw(cw, PRESALE_RAISE, denom).await?;
    let le_bytes_arr = raw.try_into().unwrap();

    Ok(u128::from_le_bytes(le_bytes_arr))
}

pub async fn query_shitcoin_supply(cw: CwClient, denom: &str) -> Result<u128> {
    let raw = query_shitcoin_meta_raw(cw, SHITCOIN_SUPPLY, denom).await?;
    let le_bytes_arr = raw.try_into().unwrap();

    Ok(u128::from_le_bytes(le_bytes_arr))
}

pub async fn query_shitcoin_launched(cw: CwClient, denom: &str) -> Result<bool> {
    let raw = query_shitcoin_meta_raw(cw, SHITCOIN_LAUNCHED, denom).await?;
    let launched = matches!(raw.as_slice(), [1]);

    Ok(launched)
}

pub async fn query_shitcoin_metadata(cw: &mut CwClient, denom: &str) -> Result<ShitcoinMeta> {
    let (creator, presale_end, presale_raise, supply, launched) = try_join5(
        query_shitcoin_creator(cw.clone(), denom),
        query_shitcoin_presale_end(cw.clone(), denom),
        query_shitcoin_presale_raise(cw.clone(), denom),
        query_shitcoin_supply(cw.clone(), denom),
        query_shitcoin_launched(cw.clone(), denom),
    )
    .await?;

    let (ticker, name, url) = try_join3(
        query_shitcoin_ticker(cw.clone(), denom),
        query_shitcoin_name(cw.clone(), denom),
        query_shitcoin_url(cw.clone(), denom),
    )
    .await?;

    let shitcoin = ShitcoinMeta {
        creator,
        ticker,
        name,
        url,
        presale_end,
        presale_raise,
        supply,
        launched,
    };

    Ok(shitcoin)
}

pub async fn query_degen_submission(cw: CwClient, denom: &str, degen: &str) -> Result<u128> {
    let raw = query_degen_meta_raw(cw, PRESALE_SUBMISSION, denom, degen).await?;

    if raw.is_empty() {
        return Ok(0);
    }

    let le_bytes_arr = raw.try_into().unwrap();

    Ok(u128::from_le_bytes(le_bytes_arr))
}

pub async fn query_degen_claimed(cw: CwClient, denom: &str, degen: &str) -> Result<bool> {
    let raw = query_degen_meta_raw(cw, PRESALE_CLAIMED, denom, degen).await?;
    let claimed = matches!(raw.as_slice(), [1]);

    Ok(claimed)
}

pub async fn query_degen_metadata(
    cw: &mut CwClient,
    denom: &str,
    degen: &str,
) -> Result<DegenMeta> {
    let (submission, claimed) = try_join(
        query_degen_submission(cw.clone(), denom, degen),
        query_degen_claimed(cw.clone(), denom, degen),
    )
    .await?;

    let degen = DegenMeta {
        submission,
        claimed,
    };

    Ok(degen)
}
