use anyhow::{bail, Result};
use cosmos_sdk_proto::cosmos::{
    bank::v1beta1::{QueryBalanceRequest, QueryDenomMetadataRequest},
    base::tendermint::v1beta1::GetLatestBlockRequest,
};

use crate::{BankClient, TmClient, PRESALE_DENOM};

#[tracing::instrument]
pub async fn latest_block_timestamp(tm: &mut TmClient) -> Result<u64> {
    let response = tm.get_latest_block(GetLatestBlockRequest {}).await?;

    let seconds = response
        .into_inner()
        .block
        .unwrap()
        .header
        .unwrap()
        .time
        .unwrap()
        .seconds as u64;

    Ok(seconds)
}

#[tracing::instrument]
pub async fn query_balance(bank: &mut BankClient, degen: &str) -> Result<u128> {
    let balance_query_req = QueryBalanceRequest {
        address: degen.to_owned(),
        denom: PRESALE_DENOM.to_owned(),
    };

    let query_response = bank.balance(balance_query_req).await?;

    let Some(coin) = query_response.into_inner().balance else {
        return Ok(0);
    };

    let balance = coin.amount.parse()?;

    Ok(balance)
}

#[tracing::instrument]
pub async fn query_denom_meta(mut bank: BankClient, denom: &str) -> Result<(String, String)> {
    let denom_query_req = QueryDenomMetadataRequest {
        denom: denom.to_owned(),
    };

    let Some(metadata) = bank
        .denom_metadata(denom_query_req)
        .await?
        .into_inner()
        .metadata
    else {
        bail!("{denom} metadata not found");
    };

    Ok((metadata.symbol, metadata.name))
}
