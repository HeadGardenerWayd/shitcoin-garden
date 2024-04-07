import { SigningCosmWasmClient }   from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet, coin } from "@cosmjs/proto-signing";
import { calculateFee, GasPrice }  from "@cosmjs/stargate";
import * as fs from "fs";

const WASM_FILE_PATH = `${__dirname}/../artifacts/shitcoin-garden.wasm`;

const walletMnemonic = process.env.DEPLOY_WALLET_MNEMONIC;
const walletAddress  = process.env.DEPLOY_WALLET_ADDRESS;
const chainPrefix    = process.env.DEPLOY_CHAIN_PREFIX;
const chainDenom     = process.env.DEPLOY_CHAIN_DENOM;
const chainRpc       = process.env.DEPLOY_CHAIN_RPC;
const chainGasPrice  = process.env.DEPLOY_CHAIN_GAS_PRICE;
const label          = process.env.DEPLOY_LABEL;
const poolFactory    = process.env.DEPLOY_POOL_FACTORY;
const createFeeDenom = process.env.DEPLOY_CREATE_FEE_DENOM;
const createFee      = process.env.DEPLOY_CREATE_FEE;
const feeRecipient   = process.env.DEPLOY_FEE_RECIPIENT;
const presaleDenom   = process.env.DEPLOY_PRESALE_DENOM;
const presaleLength  = process.env.DEPLOY_PRESALE_LENGTH;
const presaleFeeRate = process.env.DEPLOY_PRESALE_FEE_RATE;

if (!walletMnemonic) throw new Error("DEPLOY_WALLET_MNEMONIC env var not set");
if (!walletAddress)  throw new Error("DEPLOY_WALLET_ADDRESS env var not set");
if (!chainPrefix)    throw new Error("DEPLOY_CHAIN_PREFIX env var not set");
if (!chainDenom)     throw new Error("DEPLOY_CHAIN_DENOM env var not set");
if (!chainRpc)       throw new Error("DEPLOY_CHAIN_RPC env var not set");
if (!chainGasPrice)  throw new Error("DEPLOY_CHAIN_GAS_PRICE env var not set");
if (!label)          throw new Error("DEPLOY_LABEL env var not set");
if (!poolFactory)    throw new Error("DEPLOY_POOL_FACTORY env var not set");
if (!createFeeDenom) throw new Error("DEPLOY_CREATE_FEE_DENOM env var not set");
if (!createFee)      throw new Error("DEPLOY_CREATE_FEE env var not set");
if (!feeRecipient)   throw new Error("DEPLOY_FEE_RECIPIENT env var not set");
if (!presaleDenom)   throw new Error("DEPLOY_PRESALE_DENOM env var not set");
if (!presaleLength)  throw new Error("DEPLOY_PRESALE_LENGTH env var not set");
if (!presaleFeeRate) throw new Error("DEPLOY_PRESALE_FEE_RATE env var not set");

const gasPrice = GasPrice.fromString(chainGasPrice);
const wallet = await DirectSecp256k1HdWallet.fromMnemonic(walletMnemonic, { prefix: chainPrefix });
const client = await SigningCosmWasmClient.connectWithSigner(chainRpc, wallet);

const wasm = fs.readFileSync(WASM_FILE_PATH);
const uploadFee = calculateFee(2_500_000, gasPrice);
const uploadReceipt = await client.upload(walletAddress, wasm, uploadFee, label);

console.log(`uploaded contract: ${uploadReceipt.codeId}`);

const initMsg = {
    pool_factory_address: poolFactory,
    fee_recipient: feeRecipient,
    create_fee_denom: createFeeDenom,
    create_fee: createFee,
    presale_denom: presaleDenom,
    presale_length: +presaleLength,
    presale_fee_rate: +presaleFeeRate,
};
const instantiateFee = calculateFee(500_000, gasPrice);
const { contractAddress } = await client.instantiate(
  walletAddress,
  uploadReceipt.codeId,
  initMsg,
  label,
  instantiateFee,
);

console.log(`instantiated contract: ${contractAddress}`);
