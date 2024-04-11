mod chain;
mod events;
mod model;
mod view;

use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, Sse},
        IntoResponse, Response,
    },
    routing::get,
    Extension, Json, Router,
};
use cosmos_sdk_proto::{
    cosmos::{
        bank::v1beta1::query_client::QueryClient as BankQueryClient,
        base::tendermint::v1beta1::service_client::ServiceClient as TmQueryClient,
    },
    cosmwasm::wasm::v1::query_client::QueryClient as CwQueryClient,
};
use futures::{Stream, TryStreamExt};
use shuttle_axum::AxumService;
use shuttle_runtime::SecretStore;
use tendermint_rpc::{client::CompatMode, WebSocketClient, WebSocketClientUrl};
use tokio::sync::{broadcast::channel, RwLock};
use tokio_stream::wrappers::BroadcastStream;
use tonic::transport::Channel as GrpcChannel;
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::{level_filters::LevelFilter, Level};
use tracing_subscriber::EnvFilter;

use crate::model::query_entire_contract_state;

use self::chain::{latest_block_timestamp, query_balance};
use self::events::{monitor_contract_events, ContractEventKind, ShitcoinEvent, ShitcoinStream};
use self::model::ShitcoinGardenState;
use self::view::{IndexTemplate, PresaleTemplate, PresalesTemplate, UpdatedPresaleTemplate};

pub type CwClient = CwQueryClient<GrpcChannel>;
pub type BankClient = BankQueryClient<GrpcChannel>;
pub type TmClient = TmQueryClient<GrpcChannel>;

pub const SHITCOIN_GARDEN_CONTRACT: &str =
    "neutron14yg2vvwt39fan0a5gc380e65ftydlxrpaax2l72ecxfy0l7xttts0szf73";

pub const PRESALE_DENOM: &str =
    "ibc/C4CFF46FD6DE35CA4CF4CE031E643C8FDC9BA4B99AE598E9B0ED98FE3A2319F9";

struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {:#}", self.0),
        )
            .into_response()
    }
}

#[derive(Debug, Clone)]
struct Client {
    bank: BankClient,
    tm: TmClient,
}

type SharedState = Arc<RwLock<ShitcoinGardenState>>;

#[derive(Debug, Clone)]
struct AppState {
    client: Client,
    state: SharedState,
}

async fn index() -> impl IntoResponse {
    IndexTemplate
}

#[tracing::instrument]
async fn dump(State(AppState { state, .. }): State<AppState>) -> Json<ShitcoinGardenState> {
    let state = state.read_owned().await;
    Json(state.to_owned())
}

#[tracing::instrument]
async fn presales(
    State(AppState { state, mut client }): State<AppState>,
) -> Result<PresalesTemplate, AppError> {
    let state = state.read().await;

    let chain_timestamp = latest_block_timestamp(&mut client.tm).await?;

    Ok(PresalesTemplate::new(&state, chain_timestamp))
}

#[tracing::instrument]
async fn degen_presales(
    Path(degen): Path<String>,
    State(AppState { state, mut client }): State<AppState>,
) -> Result<PresalesTemplate, AppError> {
    let state = state.read().await;

    let balance = query_balance(&mut client.bank, &degen)
        .await
        .context("querying balance")?;

    let chain_timestamp = latest_block_timestamp(&mut client.tm)
        .await
        .context("querying latest block time")?;

    let view = PresalesTemplate::new_with_degen(&state, chain_timestamp, degen, balance);

    Ok(view)
}

fn full_denom(ticker: &str) -> String {
    format!("factory/{SHITCOIN_GARDEN_CONTRACT}/{ticker}")
}

#[tracing::instrument]
async fn presale(
    Path(ticker): Path<String>,
    State(AppState { state, mut client }): State<AppState>,
) -> Result<PresaleTemplate, AppError> {
    let state = state.read().await;

    let denom = full_denom(&ticker);

    let chain_timestamp = latest_block_timestamp(&mut client.tm).await?;

    Ok(PresaleTemplate::new(&state, chain_timestamp, denom))
}

#[tracing::instrument]
async fn degen_presale(
    Path((ticker, degen)): Path<(String, String)>,
    State(AppState { state, mut client }): State<AppState>,
) -> Result<PresaleTemplate, AppError> {
    let state = state.read().await;

    let denom = full_denom(&ticker);

    let chain_timestamp = latest_block_timestamp(&mut client.tm).await?;

    Ok(PresaleTemplate::new_with_degen(
        &state,
        chain_timestamp,
        denom,
        degen,
    ))
}

#[tracing::instrument]
async fn degen_balance(
    Path(degen): Path<String>,
    State(AppState { mut client, .. }): State<AppState>,
) -> Result<String, AppError> {
    let balance = query_balance(&mut client.bank, &degen).await?;

    Ok(view::balance(balance))
}

async fn handle_updated_shitcoin(event: ShitcoinEvent) -> Result<SseEvent> {
    let event_type = event.kind.sse_event_type();

    if let ContractEventKind::ShitcoinClaimed = event.kind {
        return Ok(SseEvent::default());
    }

    let template = UpdatedPresaleTemplate::new(event).render()?;

    Ok(SseEvent::default().event(event_type).data(template))
}

async fn sse_handler(
    Extension(tx): Extension<ShitcoinStream>,
) -> Sse<impl Stream<Item = Result<SseEvent>>> {
    tracing::info!("non-degen subscribing");

    let rx = tx.subscribe();

    let stream = BroadcastStream::new(rx)
        .map_err(anyhow::Error::from)
        .and_then(handle_updated_shitcoin);

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(600))
            .text("keep-alive-text"),
    )
}

async fn handle_shitcoin_event_with_degen(
    state: SharedState,
    degen: String,
    event: ShitcoinEvent,
) -> Result<SseEvent> {
    let state = state.read().await;

    let degen_meta = state
        .degens
        .get(&(event.denom.clone(), degen.clone()))
        .cloned()
        .unwrap_or_default();

    let event_type = event.kind.sse_event_type();

    if let (ContractEventKind::ShitcoinClaimed, Some(claimant)) = (event.kind, event.degen.as_ref())
    {
        if claimant != degen.as_str() {
            return Ok(SseEvent::default());
        }
    }

    let template = UpdatedPresaleTemplate::new_with_degen(event, degen_meta).render()?;

    Ok(SseEvent::default().event(event_type).data(template))
}

async fn sse_degen_handler(
    Path(degen): Path<String>,
    Extension(tx): Extension<ShitcoinStream>,
    State(AppState { state, .. }): State<AppState>,
) -> Sse<impl Stream<Item = Result<SseEvent>>> {
    tracing::info!("degen subscribing: {degen}");

    let rx = tx.subscribe();

    let stream = BroadcastStream::new(rx)
        .map_err(anyhow::Error::from)
        .and_then({
            move |shitcoin| handle_shitcoin_event_with_degen(state.clone(), degen.clone(), shitcoin)
        });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(600))
            .text("keep-alive-text"),
    )
}

async fn monitor_contract_state(
    state: SharedState,
    mut cw: CwClient,
    tm: TmClient,
    ws: WebSocketClient,
    tx: ShitcoinStream,
) {
    loop {
        if let Err(err) = monitor_contract_events(
            state.clone(),
            cw.clone(),
            tm.clone(),
            ws.clone(),
            tx.clone(),
        )
        .await
        {
            tracing::error!("monitor contract events task failed: {err}");
        }

        tracing::info!("re-fetching entire contract state");

        match query_entire_contract_state(&mut cw).await {
            Ok(entire_state) => {
                *state.write().await = entire_state;
            }
            Err(err) => {
                tracing::error!("fetching entire contract state failed: {err}");
                std::process::exit(1);
            }
        }
    }
}

async fn server(grpc_address: &str, ws_address: &str) -> Result<AxumService> {
    let ws_url: WebSocketClientUrl = ws_address.parse()?;

    let builder = WebSocketClient::builder(ws_url).compat_mode(CompatMode::V0_37);

    let (ws_client, ws_driver) = builder.build().await?;

    tokio::spawn(async move { ws_driver.run().await });

    let (tx, _rx) = channel(20);

    let grpc_endpoint = GrpcChannel::from_shared(grpc_address.to_owned())?
        .rate_limit(5, Duration::from_secs(1))
        .timeout(Duration::from_secs(5))
        .connect()
        .await?;

    let mut cw = CwQueryClient::new(grpc_endpoint.clone());
    let tm = TmClient::new(grpc_endpoint.clone());
    let bank = BankClient::new(grpc_endpoint.clone());

    let initial_state = model::query_entire_contract_state(&mut cw).await?;

    let state = Arc::new(RwLock::new(initial_state));

    tokio::spawn(monitor_contract_state(
        state.clone(),
        cw.clone(),
        tm.clone(),
        ws_client,
        tx.clone(),
    ));

    let state = AppState {
        client: Client { bank, tm },
        state,
    };

    let router = Router::new()
        .route("/", get(index))
        .route("/dump", get(dump))
        .route("/presales", get(presales))
        .route("/presales/:degen", get(degen_presales))
        .route("/presale/:denom", get(presale))
        .route("/presale/:denom/:degen", get(degen_presale))
        .route("/balance/:degen", get(degen_balance))
        .route("/sse", get(sse_handler))
        .route("/sse/:degen", get(sse_degen_handler))
        .nest_service("/static", ServeDir::new("web/static"))
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(Extension(tx));

    Ok(router.into())
}

#[shuttle_runtime::main]
async fn main(#[shuttle_runtime::Secrets] secrets: SecretStore) -> shuttle_axum::ShuttleAxum {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    tracing::info!("tracing is initialized");

    let grpc_endpoint = secrets
        .get("GRPC_ENDPOINT")
        .expect("GRPC_ENDPOINT set in Secrets.toml");

    let ws_endpoint = secrets
        .get("WS_ENDPOINT")
        .expect("WS_ENDPOINT set in Secrets.toml");

    server(&grpc_endpoint, &ws_endpoint)
        .await
        .map_err(shuttle_runtime::Error::Custom)
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
