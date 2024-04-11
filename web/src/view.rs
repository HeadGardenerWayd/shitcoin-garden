use std::collections::VecDeque;

use askama::Template;
use bigdecimal::{BigDecimal, Zero};

use crate::{
    events::{ContractEventKind, ShitcoinEvent},
    model::{DegenMeta, ShitcoinGardenState, ShitcoinMeta},
};

#[derive(Default, Debug, Clone)]
struct Amount(BigDecimal);

impl Amount {
    pub fn mm(&self) -> Amount {
        Amount(self.0.clone() / 10u32.pow(6))
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl From<u128> for Amount {
    fn from(value: u128) -> Self {
        Self(value.into())
    }
}

fn pretty_amount(amount: &Amount) -> String {
    let s = amount.0.round(6).to_string();

    let (int, dec) = s.rsplit_once('.').unwrap();

    let trimmed_dec = dec.trim_end_matches('0');

    let pretty_int_vec_chars = int.chars().rev().enumerate().fold(
        VecDeque::with_capacity(s.len() * 2),
        |mut acc, (n, c)| {
            if n % 3 == 0 && n != 0 {
                acc.push_front(b',');
            }
            acc.push_front(c as u8);
            acc
        },
    );

    let pretty_int = String::from_utf8(pretty_int_vec_chars.into()).unwrap();

    if trimmed_dec.is_empty() {
        return pretty_int;
    }

    format!("{pretty_int}.{trimmed_dec}")
}

impl std::fmt::Display for Amount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", pretty_amount(self))
    }
}

#[derive(Debug, Clone)]
struct Percent(BigDecimal);

impl std::fmt::Display for Percent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.round(2))
    }
}

#[derive(Debug, Clone, Default)]
struct Degen {
    pub presale_submission: Amount,
    pub shitcoins_claimed: bool,
}

#[derive(Debug, Clone)]
struct Shitcoin {
    denom: String,
    creator: String,
    ticker: String,
    name: String,
    url: String,
    presale_end: u64,
    presale_raise: Amount,
    supply: Amount,
    ended: bool,
    launched: bool,
    degen: Option<Degen>,
}

impl Shitcoin {
    fn percent_of_presale(&self) -> Percent {
        let Some(degen) = self.degen.as_ref() else {
            return Percent(BigDecimal::zero());
        };

        if self.presale_raise.is_zero() {
            return Percent(BigDecimal::zero());
        }

        Percent((&degen.presale_submission.0 / &self.presale_raise.0) * 100)
    }

    fn percent_of_supply(&self) -> Percent {
        Percent(self.percent_of_presale().0 / 2)
    }

    fn claimable_amount(&self) -> Amount {
        let pos = self.percent_of_supply();

        Amount((pos.0 * self.supply.mm().0) / 100)
    }

    fn icon_url(&self) -> String {
        if self.url.is_empty() {
            return "/static/shitcoin.png".to_owned();
        }

        self.url.clone()
    }

    fn seconds_remaining(&self, last_block_time: &u64) -> u64 {
        self.presale_end.saturating_sub(*last_block_time)
    }

    fn reload_trigger(&self, last_block_time: &u64) -> String {
        if self.ended {
            return "reload".to_owned();
        }

        let refresh_after = self.seconds_remaining(last_block_time);

        format!("load delay:{refresh_after}s, reload")
    }

    fn shortened_creator(&self) -> &str {
        let (_, last_3) = self.creator.split_at(self.creator.len() - 3);
        last_3
    }
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate;

#[derive(Template, Default)]
#[template(path = "presales.html")]
pub struct PresalesTemplate {
    balance: Option<Amount>,
    degen: Option<String>,
    shitcoins: Vec<Shitcoin>,
    last_block_time: u64,
}

fn get_shitcoin(
    denom: String,
    state: &ShitcoinGardenState,
    chain_timestamp: u64,
    degen: Option<&str>,
) -> Shitcoin {
    let ShitcoinMeta {
        creator,
        ticker,
        name,
        url,
        presale_end,
        presale_raise,
        supply,
        launched,
    } = state.shitcoins.get(&denom).cloned().unwrap();

    let ended = presale_end.saturating_sub(chain_timestamp) == 0;

    let degen = degen.map(|degen| {
        let Some(degen_meta) = state.degens.get(&(denom.clone(), degen.to_owned())) else {
            return Degen::default();
        };

        Degen {
            presale_submission: degen_meta.submission.into(),
            shitcoins_claimed: degen_meta.claimed,
        }
    });

    Shitcoin {
        denom,
        creator,
        ticker,
        name,
        url,
        presale_end,
        presale_raise: presale_raise.into(),
        supply: supply.into(),
        ended,
        launched,
        degen,
    }
}

fn get_shitcoins(
    state: &ShitcoinGardenState,
    chain_timestamp: u64,
    degen: Option<&str>,
) -> Vec<Shitcoin> {
    let mut shitcoins = Vec::with_capacity(state.indexes.len());

    for denom in state.indexes.values().cloned().rev() {
        let shitcoin = get_shitcoin(denom, state, chain_timestamp, degen);
        shitcoins.push(shitcoin);
    }

    shitcoins
}

impl PresalesTemplate {
    pub fn new(state: &ShitcoinGardenState, chain_timestamp: u64) -> Self {
        let shitcoins = get_shitcoins(state, chain_timestamp, None);

        Self {
            shitcoins,
            ..Default::default()
        }
    }

    pub fn new_with_degen(
        state: &ShitcoinGardenState,
        last_block_time: u64,
        degen: String,
        balance: u128,
    ) -> Self {
        let shitcoins = get_shitcoins(state, last_block_time, Some(&degen));

        let balance = Some(balance.into());

        let degen = Some(degen);

        Self {
            balance,
            degen,
            shitcoins,
            last_block_time,
        }
    }

    fn sse_path(&self) -> String {
        match self.degen.as_ref() {
            Some(degen) => format!("/sse/{degen}"),
            None => "/sse".to_owned(),
        }
    }

    fn is_update(&self) -> bool {
        false
    }
}

#[derive(Template)]
#[template(path = "presale.html")]
pub struct UpdatedPresaleTemplate {
    kind: ContractEventKind,
    shitcoin: Shitcoin,
    last_block_time: u64,
}

impl UpdatedPresaleTemplate {
    pub fn new(
        ShitcoinEvent {
            kind,
            denom,
            shitcoin,
            last_block_time,
            ..
        }: ShitcoinEvent,
    ) -> Self {
        let ShitcoinMeta {
            creator,
            ticker,
            name,
            url,
            presale_end,
            presale_raise,
            supply,
            launched,
        } = shitcoin;

        let ended = presale_end.saturating_sub(last_block_time) == 0;

        let shitcoin = Shitcoin {
            denom,
            creator,
            name,
            ticker,
            url,
            presale_end,
            presale_raise: presale_raise.into(),
            supply: supply.into(),
            ended,
            launched,
            degen: None,
        };

        Self {
            kind,
            shitcoin,
            last_block_time,
        }
    }

    pub fn new_with_degen(shitcoin_event: ShitcoinEvent, degen: DegenMeta) -> Self {
        let mut view = Self::new(shitcoin_event);

        view.shitcoin.degen = Some(Degen {
            presale_submission: degen.submission.into(),
            shitcoins_claimed: degen.claimed,
        });

        view
    }

    pub fn is_update(&self) -> bool {
        !self.kind.is_shitcoin_created()
    }
}

#[derive(Template)]
#[template(path = "presale.html")]
pub struct PresaleTemplate {
    shitcoin: Shitcoin,
    last_block_time: u64,
}

impl PresaleTemplate {
    pub fn new(state: &ShitcoinGardenState, last_block_time: u64, denom: String) -> Self {
        let shitcoin = get_shitcoin(denom, state, last_block_time, None);

        Self {
            shitcoin,
            last_block_time,
        }
    }

    pub fn new_with_degen(
        state: &ShitcoinGardenState,
        last_block_time: u64,
        denom: String,
        degen: String,
    ) -> Self {
        let shitcoin = get_shitcoin(denom, state, last_block_time, Some(&degen));

        Self {
            shitcoin,
            last_block_time,
        }
    }

    fn is_update(&self) -> bool {
        false
    }
}

pub fn balance(amount: u128) -> String {
    Amount(BigDecimal::from(amount)).mm().to_string()
}
