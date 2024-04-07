use astroport::{asset::PairInfo, factory::PairType};
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError,
};
use serde_cw_value::Value;

#[entry_point]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: Value,
) -> Result<Response, StdError> {
    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: Value,
) -> Result<Response, StdError> {
    Ok(Response::default())
}

#[entry_point]
pub fn query(_deps: Deps, env: Env, _msg: Value) -> Result<Binary, StdError> {
    to_json_binary(&PairInfo {
        asset_infos: vec![],
        contract_addr: env.contract.address.clone(),
        liquidity_token: env.contract.address.clone(),
        pair_type: PairType::Xyk {},
    })
}
