pub mod msg;
pub mod state;

use anyhow::{anyhow, bail, ensure, Result};
use astroport::{
    asset::{Asset, AssetInfo},
    factory::{ExecuteMsg as PoolFactoryMsg, PairType},
    pair::ExecuteMsg as PairMsg,
    querier::query_pair_info,
};
use cosmwasm_std::{
    coin, coins, entry_point, to_json_binary, BankMsg, Binary, DenomUnit, Deps, DepsMut, Env,
    Event, MessageInfo, StdError, Uint128, WasmMsg,
};
use msg::{
    Config, DegenMetadata, ExecuteMsg, InstantiateMsg, QueryMsg, ShitcoinMetadata, ShitcoinPage,
};
use neutron_sdk::bindings::msg::NeutronMsg;

type Response = cosmwasm_std::Response<NeutronMsg>;

pub const HUNDRED_PERCENT_BPS: Uint128 = Uint128::new(10_000);
pub const ONE_PERCENT_BPS: u32 = 100;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response> {
    ensure!(
        msg.presale_length > 0,
        "presale length has to be greater than zero"
    );
    ensure!(
        msg.presale_fee_rate < ONE_PERCENT_BPS,
        "presale fee rate has to be less than {ONE_PERCENT_BPS} bps"
    );

    deps.api.addr_validate(&msg.fee_recipient)?;
    deps.api.addr_validate(&msg.pool_factory_address)?;

    state::set_pool_factory_address(deps.storage, &msg.pool_factory_address);
    state::set_platform_fee_recipient(deps.storage, &msg.fee_recipient);
    state::set_create_fee_denom(deps.storage, &msg.create_fee_denom);
    state::set_create_fee(deps.storage, msg.create_fee);
    state::set_presale_denom(deps.storage, &msg.presale_denom);
    state::set_presale_length(deps.storage, msg.presale_length);
    state::set_presale_fee_rate(deps.storage, msg.presale_fee_rate);

    Ok(Response::default())
}

fn denom(env: &Env, subdenom: &str) -> String {
    format!("factory/{}/{subdenom}", env.contract.address)
}

fn event(kind: &str, denom: &str, degen: Option<&str>) -> Event {
    let event = Event::new("shitcoin-garden")
        .add_attribute("kind", kind)
        .add_attribute("denom", denom);

    let Some(degen) = degen else {
        return event;
    };

    event.add_attribute("degen", degen)
}

pub fn create_shitcoin(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    ticker: String,
    name: String,
    supply: Uint128,
) -> Result<Response> {
    ensure!(supply.u128() > 0, "supply must be greater than zero c'mon");

    let create_fee_denom = state::create_fee_denom(deps.storage);

    let fee_payment = cw_utils::must_pay(&info, &create_fee_denom)
        .map_err(|_| anyhow!("you must also send {create_fee_denom} to create a shitcoin"))?;

    let create_fee_amount = state::create_fee(deps.storage);

    if fee_payment < create_fee_amount {
        bail!("you must pay {create_fee_amount} {create_fee_denom} to create a shitcoin");
    }

    let subdenom = ticker.to_lowercase();

    let denom = denom(&env, &subdenom);

    let creator = info.sender.into_string();

    let shitcoin_index = state::shitcoin_count(deps.storage);

    let shitcoin_count = shitcoin_index + 1;

    let presale_length = state::presale_length(deps.storage);

    let presale_end = env.block.time.seconds() + presale_length;

    let total_supply = supply * Uint128::new(10u128.pow(6));

    state::set_shitcoin_count(deps.storage, shitcoin_count);
    state::set_shitcoin_denom(deps.storage, shitcoin_index, &denom);
    state::set_shitcoin_creator(deps.storage, &denom, &creator);
    state::set_shitcoin_ticker(deps.storage, &denom, &ticker);
    state::set_shitcoin_name(deps.storage, &denom, &name);
    state::set_shitcoin_supply(deps.storage, &denom, total_supply);
    state::set_presale_end(deps.storage, &denom, presale_end);
    state::set_presale_raise(deps.storage, &denom, Uint128::zero());

    let create_msg = NeutronMsg::submit_create_denom(subdenom.to_lowercase());

    let pair_denom = state::presale_denom(deps.storage);

    let pool_factory = state::pool_factory_address(deps.storage);

    let pfee_recipient = state::platform_fee_recipient(deps.storage);

    let metadata_msg = NeutronMsg::SetDenomMetadata {
        description: "shitcoin".to_owned(),
        denom_units: vec![
            DenomUnit {
                denom: denom.clone(),
                exponent: 0,
                aliases: vec![],
            },
            DenomUnit {
                denom: ticker.clone(),
                exponent: 6,
                aliases: vec![],
            },
        ],
        base: denom.clone(),
        display: ticker.clone(),
        name,
        symbol: ticker.clone(),
        uri: String::new(),
        uri_hash: String::new(),
    };

    let mint_msg = NeutronMsg::submit_mint_tokens(&denom, total_supply, env.contract.address);

    let create_pair_msg = PoolFactoryMsg::CreatePair {
        pair_type: PairType::Xyk {},
        asset_infos: vec![AssetInfo::native(&denom), AssetInfo::native(pair_denom)],
        init_params: None,
    };

    let create_pool_msg = WasmMsg::Execute {
        contract_addr: pool_factory,
        msg: to_json_binary(&create_pair_msg)?,
        funds: vec![],
    };

    let send_create_fee = BankMsg::Send {
        to_address: pfee_recipient,
        amount: coins(fee_payment.u128(), create_fee_denom),
    };

    let event = event("shitcoin-created", &denom, None);

    Ok(Response::default()
        .add_messages([create_msg, metadata_msg, mint_msg])
        .add_message(create_pool_msg)
        .add_message(send_create_fee)
        .add_event(event))
}

pub fn enter_presale(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    denom: String,
) -> Result<Response> {
    let presale_end =
        state::presale_end(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    if presale_end.saturating_sub(env.block.time.seconds()) == 0 {
        bail!("you're too late to enter this shitcoin's presale");
    }

    let presale_denom = state::presale_denom(deps.storage);

    let amount = cw_utils::must_pay(&info, &presale_denom)
        .map_err(|_| anyhow!("you must send {presale_denom} to enter the presale"))?;

    let fee_rate = state::presale_fee_rate(deps.storage);

    let fee = (amount * Uint128::new(fee_rate as _)) / HUNDRED_PERCENT_BPS;

    if fee.is_zero() {
        bail!("bag too smol")
    }

    let submission = amount - fee;

    let current_raise =
        state::presale_raise(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    let current_submission =
        state::presale_submission(deps.storage, &denom, info.sender.as_str()).unwrap_or_default();

    state::set_presale_raise(deps.storage, &denom, current_raise + submission);
    state::set_presale_submission(
        deps.storage,
        &denom,
        info.sender.as_str(),
        current_submission + submission,
    );

    let creator =
        state::shitcoin_creator(deps.storage, &denom).expect("shitcoin creator must be set");

    let pfee_recipient = state::platform_fee_recipient(deps.storage);

    let creator_fee = fee.multiply_ratio(1u128, 2u128);

    let platform_fee = fee - creator_fee;

    let send_cfee_msg = BankMsg::Send {
        to_address: creator,
        amount: coins(creator_fee.u128(), &presale_denom),
    };

    let send_pfee_msg = BankMsg::Send {
        to_address: pfee_recipient,
        amount: coins(platform_fee.u128(), &presale_denom),
    };

    let event = event("presale-entered", &denom, Some(info.sender.as_str()));

    Ok(Response::default()
        .add_messages([send_cfee_msg, send_pfee_msg])
        .add_event(event))
}

pub fn extend_presale(deps: DepsMut, env: Env, denom: String) -> Result<Response> {
    let presale_end =
        state::presale_end(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    if presale_end.saturating_sub(env.block.time.seconds()) != 0 {
        bail!("patience young grasshopper the presale is not over");
    }

    let presale_raise =
        state::presale_raise(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    if !presale_raise.is_zero() {
        bail!("shitcoin is primed and ready for launch");
    }

    let presale_length = state::presale_length(deps.storage);

    state::set_presale_end(
        deps.storage,
        &denom,
        env.block.time.seconds() + presale_length,
    );

    let event = event("presale-extended", &denom, None);

    Ok(Response::default().add_event(event))
}

pub fn launch_shitcoin(deps: DepsMut, env: Env, denom: String) -> Result<Response> {
    let presale_end =
        state::presale_end(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    if presale_end.saturating_sub(env.block.time.seconds()) != 0 {
        bail!("patience young grasshopper the presale is not over");
    }

    let shitcoin_launched = state::shitcoin_launched(deps.storage, &denom).unwrap_or_default();

    if shitcoin_launched {
        bail!("shitcoin already launched");
    }

    let shitcoin_supply =
        state::shitcoin_supply(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    let presale_raise =
        state::presale_raise(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    let presale_denom = state::presale_denom(deps.storage);

    let pool_factory = state::pool_factory_address(deps.storage);

    let pair_info = query_pair_info(
        &deps.querier,
        pool_factory,
        &[AssetInfo::native(&denom), AssetInfo::native(&presale_denom)],
    )?;

    let lp_shitcoin_amount = shitcoin_supply.multiply_ratio(1u128, 2u128);

    state::set_shitcoin_launched(deps.storage, &denom, true);

    let provide_liquidity_msg = PairMsg::ProvideLiquidity {
        assets: vec![
            Asset {
                info: AssetInfo::native(&denom),
                amount: lp_shitcoin_amount,
            },
            Asset {
                info: AssetInfo::native(&presale_denom),
                amount: presale_raise,
            },
        ],
        slippage_tolerance: None,
        auto_stake: None,
        receiver: None,
    };

    let seed_pool_msg = WasmMsg::Execute {
        contract_addr: pair_info.contract_addr.into_string(),
        msg: to_json_binary(&provide_liquidity_msg)?,
        funds: vec![
            coin(lp_shitcoin_amount.u128(), &denom),
            coin(presale_raise.u128(), presale_denom),
        ],
    };

    let event = event("shitcoin-launched", &denom, None);

    Ok(Response::default()
        .add_message(seed_pool_msg)
        .add_event(event))
}

pub fn claim_shitcoin(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    denom: String,
) -> Result<Response> {
    let presale_end =
        state::presale_end(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    if presale_end.saturating_sub(env.block.time.seconds()) != 0 {
        bail!("patience young grasshopper the presale is not over");
    }

    let shitcoin_launched = state::shitcoin_launched(deps.storage, &denom).unwrap_or_default();

    if !shitcoin_launched {
        bail!("shitcoin needs to be launched before claiming");
    }

    let presale_claimed =
        state::presale_claimed(deps.storage, &denom, info.sender.as_str()).unwrap_or_default();

    if presale_claimed {
        bail!("shitcoins already claimed");
    }

    let presale_raise =
        state::presale_raise(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    let presale_submission =
        state::presale_submission(deps.storage, &denom, info.sender.as_str()).unwrap_or_default();

    if presale_submission.is_zero() {
        bail!("ser you did not enter this shitcoin presale");
    }

    let shitcoin_supply =
        state::shitcoin_supply(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    let total_claimable_amount = shitcoin_supply.multiply_ratio(1u128, 2u128);

    let claimable = total_claimable_amount.multiply_ratio(presale_submission, presale_raise);

    state::set_presale_claimed(deps.storage, &denom, info.sender.as_str(), true);

    let send_shitcoins = BankMsg::Send {
        to_address: info.sender.clone().into_string(),
        amount: coins(claimable.u128(), &denom),
    };

    let event = event("shitcoin-claimed", &denom, Some(info.sender.as_str()));

    Ok(Response::default()
        .add_message(send_shitcoins)
        .add_event(event))
}

pub fn set_shitcoin_url(
    deps: DepsMut,
    info: MessageInfo,
    denom: String,
    url: String,
) -> Result<Response> {
    let creator =
        state::shitcoin_creator(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    if creator.as_str() != info.sender.as_str() {
        bail!("you are not the creator of this shitcoin");
    }

    state::set_shitcoin_url(deps.storage, &denom, &url);

    let ticker =
        state::shitcoin_ticker(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    let name =
        state::shitcoin_name(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    let metadata_msg = NeutronMsg::SetDenomMetadata {
        description: "shitcoin".to_owned(),
        denom_units: vec![
            DenomUnit {
                denom: denom.clone(),
                exponent: 0,
                aliases: vec![],
            },
            DenomUnit {
                denom: ticker.clone(),
                exponent: 6,
                aliases: vec![],
            },
        ],
        base: denom.clone(),
        display: ticker.clone(),
        name,
        symbol: ticker.clone(),
        uri: url,
        uri_hash: String::new(),
    };

    let event = event("shitcoin-url-set", &denom, None);

    Ok(Response::default()
        .add_message(metadata_msg)
        .add_event(event))
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> Result<Response> {
    match msg {
        ExecuteMsg::CreateShitcoin {
            ticker,
            name,
            supply,
        } => create_shitcoin(deps, env, info, ticker, name, supply),

        ExecuteMsg::EnterPresale { denom } => enter_presale(deps, env, info, denom),

        ExecuteMsg::ExtendPresale { denom } => extend_presale(deps, env, denom),

        ExecuteMsg::LaunchShitcoin { denom } => launch_shitcoin(deps, env, denom),

        ExecuteMsg::ClaimShitcoin { denom } => claim_shitcoin(deps, env, info, denom),

        ExecuteMsg::SetUrl { denom, url } => set_shitcoin_url(deps, info, denom, url),
    }
}

pub fn config(deps: Deps) -> Result<Config> {
    Ok(Config {
        pool_factory_address: state::pool_factory_address(deps.storage),
        fee_recipient: state::platform_fee_recipient(deps.storage),
        create_fee_denom: state::create_fee_denom(deps.storage),
        create_fee: state::create_fee(deps.storage),
        presale_denom: state::presale_denom(deps.storage),
        presale_length: state::presale_length(deps.storage),
        presale_fee_rate: state::presale_fee_rate(deps.storage),
    })
}

pub fn shitcoin_metadata(deps: Deps, env: &Env, denom: String) -> Result<ShitcoinMetadata> {
    let creator =
        state::shitcoin_creator(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    let ticker =
        state::shitcoin_ticker(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    let name =
        state::shitcoin_name(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    let url = state::shitcoin_url(deps.storage, &denom).unwrap_or_default();

    let presale_end =
        state::presale_end(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    let presale_raise =
        state::presale_raise(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    let supply =
        state::shitcoin_supply(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    let launched = state::shitcoin_launched(deps.storage, &denom).unwrap_or_default();

    let ended = presale_end.saturating_sub(env.block.time.seconds()) == 0;

    Ok(ShitcoinMetadata {
        denom,
        creator,
        ticker,
        name,
        url,
        presale_end,
        presale_raise,
        supply,
        launched,
        ended,
    })
}

pub fn shitcoins(
    deps: Deps,
    env: &Env,
    page: Option<u64>,
    limit: Option<u64>,
) -> Result<ShitcoinPage> {
    let page = page.unwrap_or_default();

    let total = state::shitcoin_count(deps.storage);

    let limit = limit.unwrap_or(10).min(total);

    let start = page * limit;

    let end = total.max(start + limit);

    let mut shitcoins = Vec::with_capacity(((end + 1) - start) as _);

    for idx in start..end {
        dbg!(idx);

        let denom = state::shitcoin_denom(deps.storage, idx).expect("valid index");

        let shitcoin_meta = shitcoin_metadata(deps, env, denom)?;

        shitcoins.push(shitcoin_meta);
    }

    Ok(ShitcoinPage {
        page,
        limit,
        total,
        shitcoins,
    })
}

pub fn degen_metadata(deps: Deps, denom: String, degen: String) -> Result<DegenMetadata> {
    state::shitcoin_creator(deps.storage, &denom).ok_or_else(|| StdError::not_found(&denom))?;

    let presale_submission =
        state::presale_submission(deps.storage, &denom, &degen).unwrap_or_default();

    let shitcoins_claimed =
        state::presale_claimed(deps.storage, &denom, &degen).unwrap_or_default();

    Ok(DegenMetadata {
        presale_submission,
        shitcoins_claimed,
    })
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary> {
    let binary = match msg {
        QueryMsg::Config {} => {
            let response = config(deps)?;

            to_json_binary(&response)?
        }

        QueryMsg::ShitcoinMetadata { denom } => {
            let response = shitcoin_metadata(deps, &env, denom)?;

            to_json_binary(&response)?
        }

        QueryMsg::Shitcoins { page, limit } => {
            let response = shitcoins(deps, &env, page, limit)?;

            to_json_binary(&response)?
        }

        QueryMsg::DegenMetadata { denom, degen } => {
            let response = degen_metadata(deps, denom, degen)?;

            to_json_binary(&response)?
        }
    };

    Ok(binary)
}

#[cfg(test)]
mod test;
