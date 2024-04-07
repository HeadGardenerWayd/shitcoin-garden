use anyhow::Result;
use futures::{Stream, TryStreamExt};
use tendermint_rpc::{
    event::Event,
    query::{EventType as TendermintEventType, Query},
    Error as RpcError, SubscriptionClient, WebSocketClient,
};
use tokio::sync::broadcast::Sender;
// use tokio_stream::{StreamExt as _, StreamMap};

use crate::{
    chain::latest_block_timestamp,
    model::{query_degen_metadata, query_shitcoin_metadata, ShitcoinMeta},
    CwClient, SharedState, TmClient, SHITCOIN_GARDEN_CONTRACT,
};

#[derive(Debug, Clone, Copy)]
pub enum ContractEventKind {
    ShitcoinCreated,
    PresaleEntered,
    PresaleExtended,
    ShitcoinLaunched,
    ShitcoinClaimed,
    ShitcoinUrlSet,
}

impl ContractEventKind {
    pub const fn sse_event_type(&self) -> &'static str {
        match self {
            ContractEventKind::ShitcoinCreated => "ShitcoinCreated",
            ContractEventKind::PresaleEntered => "PresaleEntered",
            ContractEventKind::PresaleExtended => "PresaleExtended",
            ContractEventKind::ShitcoinLaunched => "ShitcoinLaunched",
            ContractEventKind::ShitcoinClaimed => "ShitcoinClaimed",
            ContractEventKind::ShitcoinUrlSet => "ShitcoinUrlSet",
        }
    }

    /// Returns `true` if the contract event kind is [`ShitcoinCreated`].
    ///
    /// [`ShitcoinCreated`]: ContractEventKind::ShitcoinCreated
    #[must_use]
    pub fn is_shitcoin_created(&self) -> bool {
        matches!(self, Self::ShitcoinCreated)
    }
}

#[derive(Debug, Clone)]
pub struct ContractEvent {
    pub denom: String,
    pub kind: ContractEventKind,
    pub degen: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ShitcoinEvent {
    pub kind: ContractEventKind,
    pub denom: String,
    pub degen: Option<String>,
    pub shitcoin: ShitcoinMeta,
    pub last_block_time: u64,
}

type ContractEventStream = Box<dyn Stream<Item = Result<ContractEvent, RpcError>> + Unpin + Send>;
pub type ShitcoinStream = Sender<ShitcoinEvent>;

fn parse_shitcoin_garden_event(event: Event) -> ContractEvent {
    let events = event.events.unwrap();

    let kind_str = events
        .get("wasm-shitcoin-garden.kind")
        .unwrap()
        .first()
        .unwrap();

    let kind = match kind_str.as_str() {
        "shitcoin-created" => ContractEventKind::ShitcoinCreated,
        "presale-entered" => ContractEventKind::PresaleEntered,
        "presale-extended" => ContractEventKind::PresaleExtended,
        "shitcoin-launched" => ContractEventKind::ShitcoinLaunched,
        "shitcoin-claimed" => ContractEventKind::ShitcoinClaimed,
        "shitcoin-url-set" => ContractEventKind::ShitcoinUrlSet,
        _ => panic!("unexpected event kind: {kind_str}"),
    };

    let denom = events
        .get("wasm-shitcoin-garden.denom")
        .unwrap()
        .first()
        .unwrap()
        .to_owned();

    let degen = events
        .get("wasm-shitcoin-garden.degen")
        .and_then(|v| v.first())
        .map(ToOwned::to_owned);

    ContractEvent { denom, kind, degen }
}

async fn subscribe_to_events(client: &WebSocketClient) -> Result<ContractEventStream> {
    let stream = client
        .subscribe(
            Query::from(TendermintEventType::Tx)
                .and_eq("execute._contract_address", SHITCOIN_GARDEN_CONTRACT)
                .and_exists("wasm-shitcoin-garden"),
        )
        .await?
        .map_ok(parse_shitcoin_garden_event);

    Ok(Box::new(stream))
}

pub async fn monitor_contract_events(
    shared_state: SharedState,
    mut cw: CwClient,
    mut tm: TmClient,
    ws: WebSocketClient,
    tx: ShitcoinStream,
) -> Result<()> {
    let mut event_stream = subscribe_to_events(&ws).await?;

    tracing::info!("listening for contract events");

    while let Some(ContractEvent { denom, kind, degen }) = event_stream.try_next().await? {
        tracing::info!("{kind:?}: {denom}");

        // block readers as soon as event received
        let mut state = shared_state.write().await;

        let shitcoin = query_shitcoin_metadata(&mut cw, &denom).await?;

        state.shitcoins.insert(denom.clone(), shitcoin.clone());

        match kind {
            ContractEventKind::ShitcoinCreated => {
                let index = state.indexes.len() as u64;
                state.indexes.insert(index, denom.clone());
            }

            ContractEventKind::PresaleEntered | ContractEventKind::ShitcoinClaimed => {
                let degen = degen.as_ref().unwrap();
                let degen_meta = query_degen_metadata(&mut cw, &denom, degen).await?;

                state
                    .degens
                    .insert((denom.clone(), degen.clone()), degen_meta);
            }

            _ => {}
        }

        // release lock now shared state is updated
        drop(state);

        let chain_timestamp = latest_block_timestamp(&mut tm).await?;

        let event = ShitcoinEvent {
            kind,
            denom,
            degen,
            shitcoin,
            last_block_time: chain_timestamp,
        };

        if tx.send(event).is_err() {
            tracing::info!("nobody cares");
        }
    }

    Ok(())
}
