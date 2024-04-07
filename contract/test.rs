use std::collections::HashMap;

use anyhow::{bail, Error, Result};
use astroport::{
    asset::{Asset, AssetInfo, PairInfo},
    factory::{ExecuteMsg as PoolFactoryMsg, PairType, QueryMsg as PoolFactoryQuery},
    pair::ExecuteMsg as PairMsg,
};
use cosmwasm_std::{
    coin, from_json,
    testing::{mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage},
    to_json_binary, Addr, BankMsg, Coin, ContractResult, CosmosMsg, DenomUnit, Empty, OwnedDeps,
    SystemResult, WasmMsg, WasmQuery,
};
use neutron_sdk::bindings::msg::NeutronMsg;

use crate::{
    msg::{Config, ExecuteMsg, InstantiateMsg, QueryMsg, ShitcoinPage},
    Response,
};

use super::{denom, execute, instantiate, query};

type MockDeps = OwnedDeps<MockStorage, MockApi, MockQuerier, Empty>;

#[derive(Debug, PartialEq)]
enum AstroportMsg {
    CreatePool {
        contract: String,
        asset_infos: Vec<AssetInfo>,
    },
    SeedPool {
        contract: String,
        assets: Vec<Asset>,
    },
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Token {
    supply: u128,
    description: String,
    denom_units: Vec<DenomUnit>,
    base: String,
    display: String,
    name: String,
    symbol: String,
}

#[derive(Debug, Default)]
struct External {
    balances: HashMap<(String, String), u128>,
    tokens: HashMap<String, Token>,
    astroport_msgs: Vec<AstroportMsg>,
}

struct Ctx {
    deps: MockDeps,
    config: Config,
    external: External,
}

impl std::fmt::Debug for Ctx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Ctx {{ config: {:?}, external: {:?} }}",
            self.config, self.external,
        )
    }
}

fn pool_address(shitcoin_denom: &str, presale_denom: &str) -> String {
    format!("{shitcoin_denom}-{presale_denom}-pool")
}

fn initialized_contract_ctx() -> Result<Ctx> {
    let external = External::default();

    let mut deps = mock_dependencies();

    deps.querier.update_wasm(|query| {
        let WasmQuery::Smart { msg, .. } = query else {
            panic!("unexpected wasm query: {query:?}");
        };

        let Ok(PoolFactoryQuery::Pair { asset_infos }) = from_json(msg) else {
            panic!("unexpected wasm smart query: {msg}");
        };

        let [AssetInfo::NativeToken {
            denom: shitcoin_denom,
        }, AssetInfo::NativeToken {
            denom: presale_denom,
        }] = &asset_infos[..]
        else {
            panic!("unexpected assets: {asset_infos:?}");
        };

        let pool_address = pool_address(shitcoin_denom, presale_denom);

        let pair_info = PairInfo {
            asset_infos,
            contract_addr: Addr::unchecked(pool_address),
            liquidity_token: Addr::unchecked("LP token"),
            pair_type: PairType::Xyk {},
        };

        let binary = to_json_binary(&pair_info).unwrap();

        SystemResult::Ok(ContractResult::Ok(binary))
    });

    let info = mock_info("contract_deployer", &[]);

    let config = Config {
        pool_factory_address: "pool_factory".to_owned(),
        fee_recipient: "fee_recipient".to_owned(),
        create_fee_denom: "untrn".to_owned(),
        create_fee: 1_000_000u128.into(),
        presale_denom: "uatom".to_owned(),
        presale_length: 60 * 60 * 24 * 7,
        presale_fee_rate: 50,
    };

    instantiate(
        deps.as_mut(),
        mock_env(),
        info,
        InstantiateMsg {
            pool_factory_address: config.pool_factory_address.clone(),
            fee_recipient: config.fee_recipient.clone(),
            create_fee_denom: config.create_fee_denom.clone(),
            create_fee: config.create_fee,
            presale_denom: config.presale_denom.clone(),
            presale_length: config.presale_length,
            presale_fee_rate: config.presale_fee_rate,
        },
    )?;

    Ok(Ctx {
        deps,
        config,
        external,
    })
}

impl TryFrom<WasmMsg> for AstroportMsg {
    type Error = Error;

    fn try_from(value: WasmMsg) -> Result<Self> {
        let WasmMsg::Execute {
            contract_addr,
            msg,
            funds,
        } = value
        else {
            bail!("invalid wasm msg type");
        };

        if let Ok(PoolFactoryMsg::CreatePair { asset_infos, .. }) = from_json(&msg) {
            return Ok(AstroportMsg::CreatePool {
                contract: contract_addr,
                asset_infos,
            });
        }

        if let Ok(PairMsg::ProvideLiquidity { assets, .. }) = from_json(&msg) {
            return Ok(AstroportMsg::SeedPool {
                contract: contract_addr,
                assets,
            });
        }

        bail!("unexpected msg: {contract_addr} - {msg} - {funds:?}")
    }
}

impl Ctx {
    fn handle_astroport_msg(&mut self, msg: WasmMsg) {
        let astroport_msg = AstroportMsg::try_from(msg).expect("valid astroport msg");
        self.external.astroport_msgs.push(astroport_msg);
    }

    fn handle_ntrn_msg(&mut self, msg: NeutronMsg) {
        match msg {
            NeutronMsg::CreateDenom { subdenom } => {
                let denom = denom(&mock_env(), &subdenom);

                assert!(self
                    .external
                    .tokens
                    .insert(denom, Token::default())
                    .is_none());
            }

            NeutronMsg::MintTokens {
                denom,
                amount,
                mint_to_address,
            } => {
                self.external
                    .tokens
                    .entry(denom.clone())
                    .and_modify(|token| token.supply += amount.u128());

                *self
                    .external
                    .balances
                    .entry((mint_to_address, denom))
                    .or_default() += amount.u128();
            }

            NeutronMsg::SetDenomMetadata {
                description,
                denom_units,
                base,
                display,
                name,
                symbol,
                ..
            } => {
                self.external
                    .tokens
                    .entry(base.clone())
                    .and_modify(|token| {
                        token.description = description;
                        token.denom_units = denom_units;
                        token.base = base;
                        token.display = display;
                        token.name = name;
                        token.symbol = symbol;
                    });
            }

            _ => panic!("unexpected msg: {msg:?}"),
        }
    }

    fn handle_cosmos_msg(&mut self, msg: CosmosMsg<NeutronMsg>) {
        match msg {
            CosmosMsg::Bank(BankMsg::Send { to_address, amount }) => {
                let Coin { denom, amount } = amount.into_iter().next().unwrap();

                *self
                    .external
                    .balances
                    .entry((to_address, denom))
                    .or_default() += amount.u128();
            }
            CosmosMsg::Custom(ntrn_msg) => self.handle_ntrn_msg(ntrn_msg),
            CosmosMsg::Wasm(wasm_msg) => self.handle_astroport_msg(wasm_msg),
            _ => panic!("unexpected msg: {msg:?}"),
        }
    }

    fn handle_response(&mut self, response: Response) {
        for msg in response.messages {
            self.handle_cosmos_msg(msg.msg);
        }
    }

    fn create_shitcoin(
        mut self,
        creator: &str,
        ticker: &str,
        name: &str,
        supply: u128,
    ) -> Result<Self> {
        let response = execute(
            self.deps.as_mut(),
            mock_env(),
            mock_info(
                creator,
                &[coin(
                    self.config.create_fee.u128(),
                    &self.config.create_fee_denom,
                )],
            ),
            ExecuteMsg::CreateShitcoin {
                ticker: ticker.to_owned(),
                name: name.to_owned(),
                supply: supply.into(),
            },
        )?;

        self.handle_response(response);

        Ok(self)
    }

    fn enter_presale(mut self, degen: &str, denom: &str, amount: u128) -> Result<Self> {
        let response = execute(
            self.deps.as_mut(),
            mock_env(),
            mock_info(degen, &[coin(amount, &self.config.presale_denom)]),
            ExecuteMsg::EnterPresale {
                denom: denom.to_owned(),
            },
        )?;

        self.handle_response(response);

        Ok(self)
    }

    fn extend_presale(mut self, denom: &str) -> Result<Self> {
        let mut env = mock_env();

        env.block.time = env.block.time.plus_seconds(self.config.presale_length + 1);

        let response = execute(
            self.deps.as_mut(),
            env,
            mock_info("extendoor", &[]),
            ExecuteMsg::ExtendPresale {
                denom: denom.to_owned(),
            },
        )?;

        self.handle_response(response);

        Ok(self)
    }

    fn launch_shitcoin(mut self, denom: &str) -> Result<Self> {
        let mut env = mock_env();

        env.block.time = env.block.time.plus_seconds(self.config.presale_length + 1);

        let response = execute(
            self.deps.as_mut(),
            env,
            mock_info("launcher", &[]),
            ExecuteMsg::LaunchShitcoin {
                denom: denom.to_owned(),
            },
        )?;

        self.handle_response(response);

        Ok(self)
    }

    fn claim_shitcoin(mut self, degen: &str, denom: &str) -> Result<Self> {
        let mut env = mock_env();

        env.block.time = env.block.time.plus_seconds(self.config.presale_length + 1);

        let response = execute(
            self.deps.as_mut(),
            env,
            mock_info(degen, &[]),
            ExecuteMsg::ClaimShitcoin {
                denom: denom.to_owned(),
            },
        )?;

        self.handle_response(response);

        Ok(self)
    }
}

#[test]
fn initialize() -> Result<()> {
    let ctx = initialized_contract_ctx()?;

    let query_response = query(ctx.deps.as_ref(), mock_env(), QueryMsg::Config {})?;

    let actual_config: Config = from_json(query_response)?;

    assert_eq!(actual_config, ctx.config);

    Ok(())
}

mod create_shitcoin {
    use crate::msg::ShitcoinPage;

    use super::*;

    #[test]
    fn happy_path() -> Result<()> {
        let ctx = initialized_contract_ctx()?
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)?;

        let query_response = query(
            ctx.deps.as_ref(),
            mock_env(),
            QueryMsg::Shitcoins {
                page: None,
                limit: None,
            },
        )?;

        let page: ShitcoinPage = from_json(query_response)?;

        assert_eq!(page.shitcoins.len(), 1);
        assert_eq!(page.total, 1);
        assert_eq!(page.page, 0);
        assert_eq!(page.limit, 1);

        let shitcoin = page.shitcoins.into_iter().next().unwrap();

        let denom = denom(&mock_env(), "meme");

        assert_eq!(&shitcoin.denom, &denom);
        assert_eq!(shitcoin.creator, "creator");
        assert_eq!(
            shitcoin.presale_end,
            mock_env().block.time.seconds() + ctx.config.presale_length
        );
        assert_eq!(shitcoin.presale_raise.u128(), 0);
        assert_eq!(shitcoin.supply.u128(), 1_000_000_000_000);
        assert!(!shitcoin.launched);

        let token = ctx.external.tokens.get(&denom).unwrap();

        assert_eq!(
            token,
            &Token {
                supply: 1_000_000_000_000,
                description: "shitcoin".to_owned(),
                denom_units: vec![
                    DenomUnit {
                        denom: denom.clone(),
                        exponent: 0,
                        aliases: vec![],
                    },
                    DenomUnit {
                        denom: "MEME".to_owned(),
                        exponent: 6,
                        aliases: vec![],
                    },
                ],
                base: denom.clone(),
                display: "MEME".to_owned(),
                name: "memecoin".to_owned(),
                symbol: "MEME".to_owned()
            }
        );

        let shitcoin_garden_balance = ctx
            .external
            .balances
            .get(&(mock_env().contract.address.to_string(), denom.clone()))
            .unwrap();

        assert_eq!(*shitcoin_garden_balance, 1_000_000_000_000);

        assert_eq!(ctx.external.astroport_msgs.len(), 1);

        assert_eq!(
            ctx.external.astroport_msgs[0],
            AstroportMsg::CreatePool {
                contract: ctx.config.pool_factory_address,
                asset_infos: vec![
                    AssetInfo::native(denom),
                    AssetInfo::native(ctx.config.presale_denom)
                ]
            }
        );

        Ok(())
    }

    #[test]
    fn without_paying_correct_fee_amount_fails() {
        let mut ctx = initialized_contract_ctx().unwrap();

        let err = execute(
            ctx.deps.as_mut(),
            mock_env(),
            mock_info(
                "creator",
                &[coin(
                    ctx.config.create_fee.u128() - 1,
                    &ctx.config.create_fee_denom,
                )],
            ),
            ExecuteMsg::CreateShitcoin {
                ticker: "MEME".to_owned(),
                name: "memecoin".to_owned(),
                supply: 1_000u128.into(),
            },
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "you must pay 1000000 untrn to create a shitcoin"
        );
    }

    #[test]
    fn without_paying_correct_fee_denom_fails() {
        let mut ctx = initialized_contract_ctx().unwrap();

        let err = execute(
            ctx.deps.as_mut(),
            mock_env(),
            mock_info("creator", &[coin(ctx.config.create_fee.u128(), "uatom")]),
            ExecuteMsg::CreateShitcoin {
                ticker: "MEME".to_owned(),
                name: "memecoin".to_owned(),
                supply: 1_000u128.into(),
            },
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "you must also send untrn to create a shitcoin"
        );
    }
}

mod enter_presale {
    use crate::msg::{DegenMetadata, ShitcoinMetadata};
    use crate::HUNDRED_PERCENT_BPS;

    use super::*;

    #[test]
    fn happy_path() -> Result<()> {
        const PRESALE_BUY_AMOUNT: u128 = 1_000_000_000;
        let denom = denom(&mock_env(), "meme");

        let ctx = initialized_contract_ctx()?
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)?
            .enter_presale("degen1", &denom, PRESALE_BUY_AMOUNT)?
            .enter_presale("degen1", &denom, PRESALE_BUY_AMOUNT)?
            .enter_presale("degen2", &denom, PRESALE_BUY_AMOUNT)?;

        let single_fee =
            (PRESALE_BUY_AMOUNT * ctx.config.presale_fee_rate as u128) / HUNDRED_PERCENT_BPS.u128();

        let query_response = query(
            ctx.deps.as_ref(),
            mock_env(),
            QueryMsg::ShitcoinMetadata {
                denom: denom.clone(),
            },
        )?;

        let shitcoin: ShitcoinMetadata = from_json(query_response)?;

        assert_eq!(
            shitcoin.presale_raise.u128(),
            (3 * PRESALE_BUY_AMOUNT) - (3 * single_fee)
        );

        let query_response = query(
            ctx.deps.as_ref(),
            mock_env(),
            QueryMsg::DegenMetadata {
                denom: denom.clone(),
                degen: "degen1".to_owned(),
            },
        )?;

        let degen1: DegenMetadata = from_json(query_response)?;

        let query_response = query(
            ctx.deps.as_ref(),
            mock_env(),
            QueryMsg::DegenMetadata {
                denom: denom.clone(),
                degen: "degen2".to_owned(),
            },
        )?;

        let degen2: DegenMetadata = from_json(query_response)?;

        assert_eq!(
            degen1.presale_submission.u128(),
            (2 * PRESALE_BUY_AMOUNT) - (2 * single_fee)
        );
        assert_eq!(
            degen2.presale_submission.u128(),
            PRESALE_BUY_AMOUNT - single_fee
        );
        assert!(!degen1.shitcoins_claimed);
        assert!(!degen2.shitcoins_claimed);

        let fee_recipient_balance = ctx
            .external
            .balances
            .get(&(
                ctx.config.fee_recipient.clone(),
                ctx.config.presale_denom.clone(),
            ))
            .unwrap();

        assert_eq!(*fee_recipient_balance, (single_fee * 3) / 2);

        let creator_balance = ctx
            .external
            .balances
            .get(&("creator".to_owned(), ctx.config.presale_denom.clone()))
            .unwrap();

        assert_eq!(*creator_balance, (single_fee * 3) / 2);

        Ok(())
    }

    #[test]
    fn without_paying_correct_fee_denom_fails() {
        let denom = denom(&mock_env(), "meme");

        let mut ctx = initialized_contract_ctx()
            .unwrap()
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)
            .unwrap();

        let err = execute(
            ctx.deps.as_mut(),
            mock_env(),
            mock_info("degen", &[coin(1_000_000, "untrn")]),
            ExecuteMsg::EnterPresale {
                denom: denom.to_owned(),
            },
        )
        .unwrap_err();

        assert_eq!(err.to_string(), "you must send uatom to enter the presale");
    }

    #[test]
    fn presale_ended_fails() {
        let denom = denom(&mock_env(), "meme");

        let mut ctx = initialized_contract_ctx()
            .unwrap()
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)
            .unwrap();

        let mut env = mock_env();

        env.block.time = env.block.time.plus_seconds(ctx.config.presale_length + 1);

        let err = execute(
            ctx.deps.as_mut(),
            env,
            mock_info("degen", &[coin(1_000_000, &ctx.config.presale_denom)]),
            ExecuteMsg::EnterPresale {
                denom: denom.to_owned(),
            },
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "you're too late to enter this shitcoin's presale"
        );
    }

    #[test]
    fn amount_too_small_for_fee_fails() {
        let denom = denom(&mock_env(), "meme");

        let mut ctx = initialized_contract_ctx()
            .unwrap()
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)
            .unwrap();

        let err = execute(
            ctx.deps.as_mut(),
            mock_env(),
            mock_info("degen", &[coin(99, &ctx.config.presale_denom)]),
            ExecuteMsg::EnterPresale {
                denom: denom.to_owned(),
            },
        )
        .unwrap_err();

        assert_eq!(err.to_string(), "bag too smol");
    }
}

mod extend_presale {
    use crate::msg::ShitcoinMetadata;

    use super::*;

    #[test]
    fn happy_path() -> Result<()> {
        let denom = denom(&mock_env(), "meme");

        let ctx = initialized_contract_ctx()?
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)?
            .extend_presale(&denom)?;

        let query_response = query(
            ctx.deps.as_ref(),
            mock_env(),
            QueryMsg::ShitcoinMetadata {
                denom: denom.clone(),
            },
        )?;

        let shitcoin: ShitcoinMetadata = from_json(query_response)?;

        assert_eq!(
            shitcoin.presale_end,
            mock_env().block.time.seconds() + (2 * ctx.config.presale_length) + 1
        );

        Ok(())
    }

    #[test]
    fn presale_ongoing_fails() {
        let denom = denom(&mock_env(), "meme");

        let mut ctx = initialized_contract_ctx()
            .unwrap()
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)
            .unwrap();

        let mut env = mock_env();

        env.block.time = env.block.time.plus_seconds(ctx.config.presale_length - 1);

        let err = execute(
            ctx.deps.as_mut(),
            env,
            mock_info("extendoor", &[]),
            ExecuteMsg::ExtendPresale {
                denom: denom.to_owned(),
            },
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "patience young grasshopper the presale is not over"
        );
    }

    #[test]
    fn non_zero_raise_fails() {
        let denom = denom(&mock_env(), "meme");

        let err = initialized_contract_ctx()
            .unwrap()
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)
            .unwrap()
            .enter_presale("degen1", &denom, 1_000_000)
            .unwrap()
            .extend_presale(&denom)
            .unwrap_err();

        assert_eq!(err.to_string(), "shitcoin is primed and ready for launch");
    }
}

mod launch_shitcoin {
    use cosmwasm_std::Uint128;

    use crate::msg::ShitcoinMetadata;

    use super::*;

    #[test]
    fn happy_path() -> Result<()> {
        let denom = denom(&mock_env(), "meme");

        let ctx = initialized_contract_ctx()?
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)?
            .enter_presale("degen", &denom, 1_000_000_000)?
            .launch_shitcoin(&denom)?;

        let query_response = query(
            ctx.deps.as_ref(),
            mock_env(),
            QueryMsg::ShitcoinMetadata {
                denom: denom.clone(),
            },
        )?;

        let shitcoin: ShitcoinMetadata = from_json(query_response)?;

        assert!(shitcoin.launched);

        assert_eq!(ctx.external.astroport_msgs.len(), 2);

        assert_eq!(
            ctx.external.astroport_msgs[1],
            AstroportMsg::SeedPool {
                contract: pool_address(&denom, &ctx.config.presale_denom),
                assets: vec![
                    Asset {
                        info: AssetInfo::native(&denom),
                        amount: shitcoin.supply / Uint128::new(2)
                    },
                    Asset {
                        info: AssetInfo::native(ctx.config.presale_denom),
                        amount: shitcoin.presale_raise
                    }
                ]
            }
        );

        Ok(())
    }

    #[test]
    fn presale_not_ended_fails() {
        let denom = denom(&mock_env(), "meme");

        let mut ctx = initialized_contract_ctx()
            .unwrap()
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)
            .unwrap()
            .enter_presale("degen", &denom, 1_000_000_000)
            .unwrap();

        let err = execute(
            ctx.deps.as_mut(),
            mock_env(),
            mock_info("launcher", &[]),
            ExecuteMsg::LaunchShitcoin {
                denom: denom.to_owned(),
            },
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "patience young grasshopper the presale is not over"
        );
    }

    #[test]
    fn shitcoin_already_launched_fails() {
        let denom = denom(&mock_env(), "meme");

        let mut ctx = initialized_contract_ctx()
            .unwrap()
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)
            .unwrap()
            .enter_presale("degen", &denom, 1_000_000_000)
            .unwrap()
            .launch_shitcoin(&denom)
            .unwrap();

        let mut env = mock_env();

        env.block.time = env.block.time.plus_seconds(ctx.config.presale_length + 1);

        let err = execute(
            ctx.deps.as_mut(),
            env,
            mock_info("launcher", &[]),
            ExecuteMsg::LaunchShitcoin {
                denom: denom.to_owned(),
            },
        )
        .unwrap_err();

        assert_eq!(err.to_string(), "shitcoin already launched");
    }
}

mod claim_shitcoin {
    use crate::msg::{DegenMetadata, ShitcoinMetadata};

    use super::*;

    #[test]
    fn happy_path() -> Result<()> {
        const PRESALE_BUY_AMOUNT: u128 = 1_000_000_000;
        let denom = denom(&mock_env(), "meme");

        let ctx = initialized_contract_ctx()?
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)?
            .enter_presale("degen1", &denom, PRESALE_BUY_AMOUNT)?
            .enter_presale("degen1", &denom, PRESALE_BUY_AMOUNT)?
            .enter_presale("degen2", &denom, PRESALE_BUY_AMOUNT)?
            .launch_shitcoin(&denom)?
            .claim_shitcoin("degen1", &denom)?
            .claim_shitcoin("degen2", &denom)?;

        let query_response = query(
            ctx.deps.as_ref(),
            mock_env(),
            QueryMsg::ShitcoinMetadata {
                denom: denom.clone(),
            },
        )?;

        let shitcoin: ShitcoinMetadata = from_json(query_response)?;

        let claimable_supply = shitcoin.supply.u128() / 2;

        let query_response = query(
            ctx.deps.as_ref(),
            mock_env(),
            QueryMsg::DegenMetadata {
                denom: denom.clone(),
                degen: "degen1".to_owned(),
            },
        )?;

        let degen1: DegenMetadata = from_json(query_response)?;

        assert!(degen1.shitcoins_claimed);

        assert_eq!(
            *ctx.external
                .balances
                .get(&("degen1".to_owned(), denom.clone()))
                .unwrap(),
            (claimable_supply * degen1.presale_submission.u128()) / shitcoin.presale_raise.u128()
        );

        let query_response = query(
            ctx.deps.as_ref(),
            mock_env(),
            QueryMsg::DegenMetadata {
                denom: denom.clone(),
                degen: "degen2".to_owned(),
            },
        )?;

        let degen2: DegenMetadata = from_json(query_response)?;

        assert!(degen2.shitcoins_claimed);

        assert_eq!(
            *ctx.external
                .balances
                .get(&("degen2".to_owned(), denom.clone()))
                .unwrap(),
            (claimable_supply * degen2.presale_submission.u128()) / shitcoin.presale_raise.u128()
        );

        Ok(())
    }

    #[test]
    fn presale_ongoing_fails() {
        let denom = denom(&mock_env(), "meme");

        let mut ctx = initialized_contract_ctx()
            .unwrap()
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)
            .unwrap()
            .enter_presale("degen", &denom, 1_000_000)
            .unwrap();

        let mut env = mock_env();

        env.block.time = env.block.time.plus_seconds(ctx.config.presale_length - 1);

        let err = execute(
            ctx.deps.as_mut(),
            env,
            mock_info("degen", &[]),
            ExecuteMsg::ClaimShitcoin {
                denom: denom.to_owned(),
            },
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "patience young grasshopper the presale is not over"
        );
    }

    #[test]
    fn shitcoin_not_launched_fails() {
        let denom = denom(&mock_env(), "meme");

        let err = initialized_contract_ctx()
            .unwrap()
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)
            .unwrap()
            .enter_presale("degen", &denom, 1_000_000)
            .unwrap()
            .claim_shitcoin("degen", &denom)
            .unwrap_err();

        assert_eq!(
            err.to_string(),
            "shitcoin needs to be launched before claiming"
        );
    }

    #[test]
    fn shitcoins_already_claimed_fails() {
        let denom = denom(&mock_env(), "meme");

        let err = initialized_contract_ctx()
            .unwrap()
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)
            .unwrap()
            .enter_presale("degen", &denom, 1_000_000)
            .unwrap()
            .launch_shitcoin(&denom)
            .unwrap()
            .claim_shitcoin("degen", &denom)
            .unwrap()
            .claim_shitcoin("degen", &denom)
            .unwrap_err();

        assert_eq!(err.to_string(), "shitcoins already claimed");
    }

    #[test]
    fn did_not_enter_presale_fails() {
        let denom = denom(&mock_env(), "meme");

        let err = initialized_contract_ctx()
            .unwrap()
            .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)
            .unwrap()
            .enter_presale("degen", &denom, 1_000_000)
            .unwrap()
            .launch_shitcoin(&denom)
            .unwrap()
            .claim_shitcoin("griff", &denom)
            .unwrap_err();

        assert_eq!(
            err.to_string(),
            "ser you did not enter this shitcoin presale"
        );
    }
}

#[test]
fn shitcoins_query() -> Result<()> {
    let ctx = initialized_contract_ctx()?
        .create_shitcoin("creator", "MEME", "memecoin", 1_000_000)?
        .create_shitcoin("creator", "EMERALD", "emerald", 1_000)?;

    let binary = query(
        ctx.deps.as_ref(),
        mock_env(),
        QueryMsg::Shitcoins {
            page: None,
            limit: Some(100),
        },
    )?;

    let _shitcoin_page: ShitcoinPage = cosmwasm_std::from_json(binary)?;

    Ok(())
}
